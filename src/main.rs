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
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tower::Layer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

fn default_listen() -> SocketAddr {
    "127.0.0.1:3000".parse().unwrap()
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

#[derive(Deserialize, Clone)]
#[serde(tag = "method", rename_all = "snake_case")]
enum AuthConfig {
    Header {
        header_name: Option<String>,
    },
    #[serde(alias = "cms", alias = "cms-cookie")]
    CmsCookie {
        cookie_name: String,
        secret: String,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::Header { header_name: None }
    }
}

#[derive(Deserialize)]
struct Config {
    #[serde(default = "default_listen")]
    listen: SocketAddr,
    #[serde(default = "default_data_dir")]
    data_dir: PathBuf,
    #[serde(default)]
    auth: AuthConfig,
}

#[derive(Clone)]
struct AppState {
    data_dir: PathBuf,
    auth: AuthConfig,
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

struct Auth(String);

impl FromRequestParts<AppState> for Auth {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        match &state.auth {
            AuthConfig::Header { header_name } => {
                let name = header_name.as_deref().unwrap_or("X-OII-AUTH");
                let token = parts
                    .headers
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .ok_or(StatusCode::UNAUTHORIZED)?;
                if token.is_empty() || token.contains('/') || token.contains('\\') || token.contains("..") {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                Ok(Self(token.to_owned()))
            }
            AuthConfig::CmsCookie { cookie_name, secret } => {
                let cookie_header = parts
                    .headers
                    .get(http::header::COOKIE)
                    .and_then(|v| v.to_str().ok())
                    .ok_or(StatusCode::UNAUTHORIZED)?;
                let username = parse_cms_cookie(cookie_header, cookie_name, secret)
                    .ok_or(StatusCode::UNAUTHORIZED)?;
                if username.is_empty() || username.contains('/') || username.contains('\\') || username.contains("..") {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                Ok(Self(username))
            }
        }
    }
}

/// Parses and validates a Tornado 4 signed CMS cookie.
fn parse_cms_cookie(cookie_header: &str, target_name: &str, secret: &str) -> Option<String> {
    let secret_bytes = hex::decode(secret).unwrap_or_else(|_| secret.as_bytes().to_vec());
    let raw = cookie_header.split(';').find_map(|s| {
        let (k, v) = s.trim().split_once('=')?;
        (k.trim() == target_name).then_some(v.trim().trim_matches('"'))
    })?;

    if !raw.starts_with("2|") {
        return None;
    }

    fn consume(s: &str) -> Option<(&str, &str)> {
        let (len, rest) = s.split_once(':')?;
        let n: usize = len.parse().ok()?;
        if rest.len() < n || rest.as_bytes().get(n) != Some(&b'|') {
            return None;
        }
        Some((&rest[..n], &rest[n + 1..]))
    }

    let rest = &raw[2..];
    let (_key_ver, rest) = consume(rest)?;
    let (_ts, rest) = consume(rest)?;
    let (name, rest) = consume(rest)?;
    let (val_b64, passed_sig) = consume(rest)?;

    if name != target_name {
        return None;
    }

    let to_sign = &raw[..raw.len() - passed_sig.len()];
    let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).ok()?;
    mac.update(to_sign.as_bytes());
    let sig_bytes = hex::decode(passed_sig).ok()?;
    mac.verify_slice(&sig_bytes).ok()?;

    let decoded = STANDARD.decode(val_b64).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    if let Some(arr) = json.as_array() {
        arr.first().and_then(|v| v.as_str()).map(String::from)
    } else {
        json.as_str().map(String::from)
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
    fn test_parse_cms_cookie_valid() {
        let secret = "my_secret_key";
        let cookie_name = "practice_login";
        let username = "tonno";

        let payload = format!(r#"["{username}", "bcrypt:hash", 1790238485, false]"#);
        let b64_val = STANDARD.encode(payload.as_bytes());
        let to_sign = format!("2|1:0|10:1790238485|{}:{}|{}:{}|", cookie_name.len(), cookie_name, b64_val.len(), b64_val);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        let cookie = format!("{to_sign}{sig}");

        let header = format!("other=123; {cookie_name}=\"{cookie}\"; extra=abc");
        let parsed = parse_cms_cookie(&header, cookie_name, secret).unwrap();
        assert_eq!(parsed, username);
    }

    #[test]
    fn test_user_cms_cookie_with_hex_secret() {
        let secret = "82a0cc1c6e0bd7e5f9ba63045b0b5cb3";
        let cookie_name = "practice_login";
        let cookie = "2|1:0|10:1790243501|14:practice_login|144:WyJ0b25ubyIsICJiY3J5cHQ6JDJiJDEyJDhuRlhvb2Y3YU5Ic1NDWmxBanJ1VU9OaUwwYVlTSm9NSHR3WUVYME9QeTZYcXZ1OGg1Mk4yIiwgMTc5MDI0MzUwMS40ODcxODQsIGZhbHNlXQ==|90b4b85c453917d63c006255b36cbf5bdf3202bb2cf0fd58a43aa89cbedd20be";
        let header = format!("{cookie_name}={cookie}");
        let username = parse_cms_cookie(&header, cookie_name, secret).unwrap();
        assert_eq!(username, "tonno");
    }

    #[test]
    fn test_parse_cms_cookie_invalid_sig() {
        let secret = "my_secret_key";
        let header = "practice_login=2|1:0|10:1790238485|14:practice_login|144:WyJ0b25ubyIsICJiY3J5cHQ6JDJiJDEyJDhuRlhvb2Y3YU5Ic1NDWmxBanJ1VU9OaUwwYVlTSm9NSHR3WUVYME9QeTZYcXZ1OGg1Mk4yIiwgMTc5MDIzODQ4NS40MDQ1NTksIGZhbHNlXQ==|invalid_sig";
        assert!(parse_cms_cookie(header, "practice_login", secret).is_none());
    }

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
