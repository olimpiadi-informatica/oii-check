use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Router, serve};
use base64::Engine;
use chrono::Local;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

/// Command-line arguments for the OII check backend.
#[derive(Parser, Debug)]
struct Cli {
    /// Socket address to bind the HTTP server to.
    #[arg(long, default_value = "127.0.0.1:3000")]
    listen: SocketAddr,

    /// Directory where uploaded metadata and screenshots are stored.
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
}

#[derive(Clone)]
struct AppState {
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let state = AppState {
        data_dir: cli.data_dir,
    };
    tokio::fs::create_dir_all(&state.data_dir).await?;

    let app = Router::new()
        .route("/internet", post(internet))
        .route("/screen", post(screen))
        .fallback_service(ServeDir::new("static"))
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

fn validate_token(token: &str) -> Result<(), StatusCode> {
    let bytes = token.as_bytes();
    if bytes.len() != 9 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let is_valid = matches!(bytes[0], b't' | b'b')
        && bytes[1] == b'-'
        && bytes[2..5].iter().all(u8::is_ascii_lowercase)
        && bytes[5] == b'-'
        && bytes[6..9].iter().all(u8::is_ascii_digit);

    if is_valid {
        Ok(())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn token_dir(state: &AppState, token: &str) -> Result<PathBuf, StatusCode> {
    validate_token(token)?;
    let directory = state.data_dir.join(token);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(directory)
}

#[derive(Deserialize)]
struct InternetPayload {
    client_ts: f64,
    fp: String,
    ic: Vec<bool>,
    token: String,
}

#[derive(Serialize)]
struct InternetRecord {
    client_ts: f64,
    fp: String,
    ic: Vec<bool>,
    server_ts: f64,
    token: String,
}

async fn internet(
    State(state): State<AppState>,
    Json(data): Json<InternetPayload>,
) -> Result<(), StatusCode> {
    let token_dir = token_dir(&state, &data.token).await?;
    let record = InternetRecord {
        client_ts: data.client_ts,
        fp: data.fp,
        ic: data.ic,
        server_ts: unix_timestamp_seconds(),
        token: data.token,
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
    token: String,
}

async fn screen(
    State(state): State<AppState>,
    Json(data): Json<ScreenPayload>,
) -> Result<(), StatusCode> {
    let token_dir = token_dir(&state, &data.token).await?;
    let fp_dir = token_dir.join(&data.fp);
    tokio::fs::create_dir_all(&fp_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let img = base64::engine::general_purpose::STANDARD
        .decode(&data.img)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    tokio::fs::write(
        fp_dir.join({
            format!(
                "{}_{}.webp",
                data.client_ts,
                Local::now().format("%Y-%m-%d %H:%M:%S"),
            )
        }),
        img,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}
