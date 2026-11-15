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
        cookie_name: Option<String>,
        secret: String,
    },
}

fn is_valid_identity(s: &str) -> bool {
    !s.is_empty() && !s.contains('/') && !s.contains('\\') && !s.contains("..")
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
                if !is_valid_identity(token) {
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
                let username = parse_cms_cookies(cookie_header, cookie_name.as_deref(), secret)
                    .ok_or(StatusCode::UNAUTHORIZED)?;
                Ok(Self(username))
            }
        }
    }
}

/// Parses and validates matching Tornado 4 signed CMS cookies.
///
/// If `target_name` is `Some(name)`, only that cookie is checked; otherwise any `*_login`
/// cookie is considered. When multiple valid cookies match, the most recently issued one
/// (highest Tornado `_ts`) is selected.
fn parse_cms_cookies(cookie_header: &str, target_name: Option<&str>, secret: &str) -> Option<String> {
    let secret_bytes = hex::decode(secret).unwrap_or_else(|_| secret.as_bytes().to_vec());
    cookie_header
        .split(';')
        .filter_map(|s| {
            let (k, v) = s.trim().split_once('=')?;
            let k = k.trim();
            let matches = match target_name {
                Some(name) => k == name,
                None => k.ends_with("_login"),
            };
            if !matches {
                return None;
            }
            let raw = v.trim().trim_matches('"');
            let (ts, username) = parse_cms_cookie_value(raw, k, &secret_bytes)?;
            is_valid_identity(&username).then_some((ts, username))
        })
        .max_by_key(|(ts, _)| *ts)
        .map(|(_, username)| username)
}

/// Parses and validates a single Tornado 4 signed CMS cookie value, returning `(ts, username)`.
fn parse_cms_cookie_value(raw: &str, target_name: &str, secret_bytes: &[u8]) -> Option<(u64, String)> {

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
    let (ts_str, rest) = consume(rest)?;
    let ts: u64 = ts_str.parse().ok()?;
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
    let username = if let Some(arr) = json.as_array() {
        arr.first().and_then(|v| v.as_str()).map(String::from)?
    } else {
        json.as_str().map(String::from)?
    };
    Some((ts, username))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cms_cookie(cookie_name: &str, username: &str, ts: u64, secret: &str) -> String {
        let payload = format!(r#"["{username}", "bcrypt:hash", {ts}, false]"#);
        let b64_val = STANDARD.encode(payload.as_bytes());
        let ts_str = ts.to_string();
        let to_sign = format!(
            "2|1:0|{}:{}|{}:{}|{}:{}|",
            ts_str.len(),
            ts_str,
            cookie_name.len(),
            cookie_name,
            b64_val.len(),
            b64_val
        );
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        format!("{to_sign}{sig}")
    }

    #[test]
    fn test_parse_cms_cookie_valid() {
        let secret = "my_secret_key";
        let cookie_name = "practice_login";
        let username = "tonno";
        let cookie = make_cms_cookie(cookie_name, username, 1790238485, secret);

        let header = format!("other=123; {cookie_name}=\"{cookie}\"; extra=abc");
        let parsed = parse_cms_cookies(&header, Some(cookie_name), secret).unwrap();
        assert_eq!(parsed, username);
    }

    #[test]
    fn test_user_cms_cookie_with_hex_secret() {
        let secret = "82a0cc1c6e0bd7e5f9ba63045b0b5cb3";
        let cookie_name = "practice_login";
        let cookie = "2|1:0|10:1790243501|14:practice_login|144:WyJ0b25ubyIsICJiY3J5cHQ6JDJiJDEyJDhuRlhvb2Y3YU5Ic1NDWmxBanJ1VU9OaUwwYVlTSm9NSHR3WUVYME9QeTZYcXZ1OGg1Mk4yIiwgMTc5MDI0MzUwMS40ODcxODQsIGZhbHNlXQ==|90b4b85c453917d63c006255b36cbf5bdf3202bb2cf0fd58a43aa89cbedd20be";
        let header = format!("{cookie_name}={cookie}");
        let username = parse_cms_cookies(&header, Some(cookie_name), secret).unwrap();
        assert_eq!(username, "tonno");
    }

    #[test]
    fn test_parse_cms_cookie_invalid_sig() {
        let secret = "my_secret_key";
        let header = "practice_login=2|1:0|10:1790238485|14:practice_login|144:WyJ0b25ubyIsICJiY3J5cHQ6JDJiJDEyJDhuRlhvb2Y3YU5Ic1NDWmxBanJ1VU9OaUwwYVlTSm9NSHR3WUVYME9QeTZYcXZ1OGg1Mk4yIiwgMTc5MDIzODQ4NS40MDQ1NTksIGZhbHNlXQ==|invalid_sig";
        assert!(parse_cms_cookies(header, Some("practice_login"), secret).is_none());
    }

    #[test]
    fn test_autodetect_picks_newest_timestamp() {
        let secret = "my_secret_key";
        let older = make_cms_cookie("contest1_login", "alice", 1790238000, secret);
        let newer = make_cms_cookie("contest2_login", "bob", 1790239000, secret);

        // Newer cookie first
        let header1 = format!("contest2_login={newer}; contest1_login={older}");
        assert_eq!(parse_cms_cookies(&header1, None, secret).as_deref(), Some("bob"));

        // Older cookie first
        let header2 = format!("contest1_login={older}; contest2_login={newer}");
        assert_eq!(parse_cms_cookies(&header2, None, secret).as_deref(), Some("bob"));
    }

    #[test]
    fn test_autodetect_ignores_non_login_and_invalid_cookies() {
        let secret = "my_secret_key";
        let valid = make_cms_cookie("oii_login", "carol", 1790238500, secret);
        // A non-*_login cookie even if validly signed should be ignored during autodetect
        let non_login = make_cms_cookie("other_cookie", "mallory", 1790239999, secret);
        // A newer *_login cookie with an invalid signature should be ignored
        let header = format!(
            "other_cookie={non_login}; bad_login=2|1:0|10:1790999999|9:bad_login|4:e30=|deadbeef; oii_login={valid}"
        );
        assert_eq!(parse_cms_cookies(&header, None, secret).as_deref(), Some("carol"));
    }

    #[test]
    fn test_explicit_cookie_name_ignores_other_login_cookies() {
        let secret = "my_secret_key";
        let c1 = make_cms_cookie("contest1_login", "alice", 1790238000, secret);
        let c2 = make_cms_cookie("contest2_login", "bob", 1790239000, secret);
        let header = format!("contest1_login={c1}; contest2_login={c2}");
        assert_eq!(
            parse_cms_cookies(&header, Some("contest1_login"), secret).as_deref(),
            Some("alice")
        );
    }
}
