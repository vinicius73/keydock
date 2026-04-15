//! Credential extraction from HTTP headers and query string (parsing only; no resolution).

use axum::http::{HeaderMap, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use percent_encoding::percent_decode_str;
use tracing::instrument;

/// Raw credential material extracted from the wire (never logged).
#[derive(Clone)]
pub enum RawCredential {
    Bearer(String),
    Basic(String),
    QueryParam(String),
}

impl RawCredential {
    pub fn as_str(&self) -> &str {
        match self {
            RawCredential::Bearer(s) | RawCredential::Basic(s) | RawCredential::QueryParam(s) => {
                s.as_str()
            }
        }
    }
}

/// Extracts a credential from `Authorization` (preferred) or query parameters.
///
/// Priority: `Authorization` (Bearer, then Basic) wins over the query string.
/// Query parameters: `access_token` is preferred over `key` when both are present.
#[instrument(skip_all)]
pub fn extract(headers: &HeaderMap, query: Option<&str>) -> Option<RawCredential> {
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(rest) = auth.strip_prefix("Bearer ") {
            let token = rest.trim();
            if !token.is_empty() {
                return Some(RawCredential::Bearer(token.to_string()));
            }
        }
        if let Some(rest) = auth.strip_prefix("Basic ")
            && let Ok(bytes) = BASE64_STD.decode(rest.trim())
            && let Ok(decoded) = String::from_utf8(bytes)
            && let Some(cred) = basic_username_credential(&decoded)
        {
            return Some(RawCredential::Basic(cred));
        }
    }

    extract_from_query(query?)
}

fn basic_username_credential(decoded: &str) -> Option<String> {
    let user = match decoded.split_once(':') {
        Some((u, _)) => u,
        None => decoded,
    };
    if user.is_empty() {
        None
    } else {
        Some(user.to_string())
    }
}

fn extract_from_query(query: &str) -> Option<RawCredential> {
    let mut access_token: Option<String> = None;
    let mut key_param: Option<String> = None;

    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let name = name.trim();
        if name == "access_token" && access_token.is_none() {
            access_token = decode_query_value(value);
        } else if name == "key" && key_param.is_none() {
            key_param = decode_query_value(value);
        }
    }

    let value = access_token.or(key_param)?;
    if value.is_empty() {
        return None;
    }
    Some(RawCredential::QueryParam(value))
}

fn decode_query_value(value: &str) -> Option<String> {
    let bytes = percent_decode_str(value).collect::<Vec<u8>>();
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    fn headers_with_auth(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(value).expect("header value"),
        );
        h
    }

    #[test]
    fn extract_bearer() {
        let h = headers_with_auth("Bearer secret-token");
        let c = extract(&h, Some("access_token=ignored")).expect("credential");
        assert_eq!(c.as_str(), "secret-token");
    }

    #[test]
    fn extract_bearer_priority_over_query() {
        let h = headers_with_auth("Bearer from-header");
        let c = extract(&h, Some("access_token=from-query")).expect("credential");
        assert_eq!(c.as_str(), "from-header");
    }

    #[test]
    fn extract_basic_username() {
        // "user:pass" -> credential is username per contract.
        let encoded = BASE64_STD.encode(b"alice:secret");
        let h = headers_with_auth(&format!("Basic {encoded}"));
        let c = extract(&h, None).expect("credential");
        assert_eq!(c.as_str(), "alice");
    }

    #[rstest]
    #[case::access_token("access_token=qp-token", "qp-token")]
    #[case::key_alias("key=key-alias", "key-alias")]
    #[case::access_token_wins_over_key("key=first&access_token=second", "second")]
    #[case::percent_encoded("access_token=hello%20world", "hello world")]
    fn extract_query_cases(#[case] query: &str, #[case] expected: &str) {
        let h = HeaderMap::new();
        let c = extract(&h, Some(query)).expect("credential");
        assert_eq!(c.as_str(), expected);
    }

    #[test]
    fn extract_missing_returns_none() {
        let h = HeaderMap::new();
        assert_eq!(extract(&h, None).is_none(), true);
        assert_eq!(extract(&h, Some("other=1")).is_none(), true);
    }
}
