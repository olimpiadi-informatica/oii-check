use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::AppState;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AuthConfig {
    Header {
        header_name: String,
    },
    #[serde(alias = "cms", alias = "cms-cookie")]
    CmsCookie {
        cookie_name: String,
        secret: String,
    },
}

pub struct Auth(pub String);

impl FromRequestParts<AppState> for Auth {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        match &state.auth {
            AuthConfig::Header { header_name } => {
                let token = parts
                    .headers
                    .get(header_name.as_str())
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
}
