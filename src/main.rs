use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::{FromRequestParts, Json, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::routing::post;
use axum::{Router, serve};
use axum_extra::TypedHeader;
use axum_extra::headers::ContentType;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

/// Command-line arguments for the OII check backend.
#[derive(Parser, Debug)]
struct Cli {
    /// Socket address to bind the HTTP server to.
    #[arg(short, long, default_value = "127.0.0.1:3000")]
    listen: SocketAddr,

    /// Directory where uploaded metadata and screenshots are stored.
    #[arg(short, long, default_value = "data")]
    data_dir: PathBuf,
}

#[derive(Clone)]
struct AppState {
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let state = AppState {
        data_dir: cli.data_dir,
    };
    tokio::fs::create_dir_all(&state.data_dir).await?;

    let app = Router::new()
        .route("/internet", post(internet))
        .route("/screen", post(screen))
        .fallback_service(ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = TcpListener::bind(cli.listen).await?;
    serve(listener, app).await?;
    Ok(())
}

fn unix_timestamp_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs_f64()
}

struct ClientTs(f64);

impl<S> FromRequestParts<S> for ClientTs
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let client_ts = parts
            .headers
            .get("X-OII-CLIENT-TS")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .ok_or(StatusCode::BAD_REQUEST)?;
        Ok(Self(client_ts))
    }
}

struct Fingerprint(String);

impl<S> FromRequestParts<S> for Fingerprint
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let fp = parts
            .headers
            .get("X-OII-FP")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::BAD_REQUEST)?;
        let bytes = fp.as_bytes();
        if bytes.len() != 32 || !bytes.iter().all(u8::is_ascii_hexdigit) {
            return Err(StatusCode::BAD_REQUEST);
        }
        Ok(Self(fp.to_owned()))
    }
}

struct Auth(String);

impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("X-OII-AUTH")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        Ok(Self(token.to_owned()))
    }
}

#[derive(Deserialize)]
struct InternetPayload {
    ic: Vec<bool>,
}

#[derive(Serialize)]
struct InternetRecord {
    client_ts: f64,
    fp: String,
    ic: Vec<bool>,
    server_ts: f64,
}

#[derive(Serialize)]
struct ScreenRecord {
    client_ts: f64,
    fp: String,
    filename: String,
    server_ts: f64,
}

async fn internet(
    State(state): State<AppState>,
    Auth(token): Auth,
    ClientTs(client_ts): ClientTs,
    Fingerprint(fp): Fingerprint,
    Json(data): Json<InternetPayload>,
) -> Result<(), StatusCode> {
    let token_dir = state.data_dir.join(&token);
    tokio::fs::create_dir_all(&token_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let record = InternetRecord {
        client_ts,
        fp,
        ic: data.ic,
        server_ts: unix_timestamp_seconds(),
    };

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(token_dir.join("logs.json"))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut line = serde_json::to_vec(&record).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    line.push(b'\n');
    file.write_all(&line)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

async fn screen(
    State(state): State<AppState>,
    Auth(token): Auth,
    ClientTs(client_ts): ClientTs,
    Fingerprint(fp): Fingerprint,
    TypedHeader(content_type): TypedHeader<ContentType>,
    body: Bytes,
) -> Result<(), StatusCode> {
    let server_ts = unix_timestamp_seconds();
    let token_dir = state.data_dir.join(&token);
    let fp_dir = token_dir.join(&fp);
    tokio::fs::create_dir_all(&fp_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let extension = if content_type == ContentType::png() {
        "png"
    } else if content_type == ContentType::jpeg() {
        "jpg"
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let filename = format!("{client_ts}.{extension}");
    tokio::fs::write(fp_dir.join(&filename), body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let record = ScreenRecord {
        client_ts,
        fp,
        filename,
        server_ts,
    };
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(token_dir.join("logs.json"))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut line = serde_json::to_vec(&record).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    line.push(b'\n');
    file.write_all(&line)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}
