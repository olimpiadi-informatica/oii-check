mod auth;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use auth::{Auth, AuthConfig};
use axum::body::Bytes;
use axum::extract::{FromRequestParts, Json, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::routing::post;
use axum::{Router, serve};
use axum_extra::TypedHeader;
use axum_extra::headers::ContentType;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tower::Layer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

#[derive(Deserialize)]
struct Config {
    listen: SocketAddr,
    data_dir: PathBuf,
    auth: AuthConfig,
}

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub auth: AuthConfig,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config_path = std::env::args().nth(1).unwrap_or_else(|| "config.toml".into());
    let config_str = tokio::fs::read_to_string(&config_path).await.map_err(|e| {
        format!("Failed to read config file '{config_path}': {e}\nSee config.example.toml")
    })?;
    let config: Config = toml::from_str(&config_str)?;

    let state = AppState {
        data_dir: config.data_dir,
        auth: config.auth,
    };
    tokio::fs::create_dir_all(&state.data_dir).await?;

    let app = Router::new()
        .route("/internet", post(internet))
        .route("/screen", post(screen))
        .fallback_service(
            SetResponseHeaderLayer::if_not_present(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("public, no-cache"),
            )
            .layer(ServeDir::new("static")),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = TcpListener::bind(config.listen).await?;
    tracing::info!("Server listening on {}", config.listen);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let toml_str = r#"
            listen = "0.0.0.0:8000"
            data_dir = "contest_data"
            [auth]
            method = "cms_cookie"
            cookie_name = "practice_login"
            secret = "cms_secret"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.listen, "0.0.0.0:8000".parse().unwrap());
        assert_eq!(config.data_dir, PathBuf::from("contest_data"));
        match config.auth {
            AuthConfig::CmsCookie { cookie_name, secret } => {
                assert_eq!(cookie_name, "practice_login");
                assert_eq!(secret, "cms_secret");
            }
            _ => panic!("Expected CmsCookie"),
        }
    }
}
