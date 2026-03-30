use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{FromRequestParts, Json, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::routing::post;
use axum::{Router, serve};
use base64::Engine;
use chrono::Local;
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

fn validate_fingerprint(fp: &str) -> Result<(), StatusCode> {
    let bytes = fp.as_bytes();

    let is_hex_32 = bytes.len() == 32 && bytes.iter().all(u8::is_ascii_hexdigit);
    let is_uuid = bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 8 | 13 | 18 | 23) || byte.is_ascii_hexdigit());

    if is_hex_32 || is_uuid {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
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
    client_ts: f64,
    fp: String,
    ic: Vec<bool>,
}

#[derive(Serialize)]
struct InternetRecord {
    client_ts: f64,
    fp: String,
    ic: Vec<bool>,
    server_ts: f64,
}

async fn internet(
    State(state): State<AppState>,
    Auth(token): Auth,
    Json(data): Json<InternetPayload>,
) -> Result<(), StatusCode> {
    validate_fingerprint(&data.fp)?;
    let token_dir = state.data_dir.join(&token);
    tokio::fs::create_dir_all(&token_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let record = InternetRecord {
        client_ts: data.client_ts,
        fp: data.fp,
        ic: data.ic,
        server_ts: unix_timestamp_seconds(),
    };

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(token_dir.join("internet.json"))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut line = serde_json::to_vec(&record).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    line.push(b'\n');
    file.write_all(&line)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

#[derive(Deserialize)]
struct ScreenPayload {
    client_ts: f64,
    img: String,
    fp: String,
}

async fn screen(
    State(state): State<AppState>,
    Auth(token): Auth,
    Json(data): Json<ScreenPayload>,
) -> Result<(), StatusCode> {
    validate_fingerprint(&data.fp)?;
    let server_ts = unix_timestamp_seconds();
    let fp_dir = state.data_dir.join(&token).join(&data.fp);
    tokio::fs::create_dir_all(&fp_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let img = base64::engine::general_purpose::STANDARD
        .decode(&data.img)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let filename = format!(
        "{}_{}_{}.webp",
        data.client_ts,
        server_ts,
        Local::now().format("%Y-%m-%d %H:%M:%S"),
    );
    tokio::fs::write(fp_dir.join(filename), img)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}
