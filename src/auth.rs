use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{StatusCode, header::HeaderMap};

#[derive(Clone, Copy, Debug)]
pub struct UserId(pub i64);

#[derive(Clone, Copy, Debug)]
pub struct HxRequest(pub bool);

/// Identity resolved from proxy headers, or the dev fallback.
#[derive(Debug, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

/// Pure resolution: proxy headers win; else dev_user; else None (=> 401).
pub fn resolve_identity(headers: &HeaderMap, dev_user: Option<&str>) -> Option<Identity> {
    let get = |k: &str| {
        headers
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    if let Some(subject) = get("Remote-User").filter(|s| !s.is_empty()) {
        return Some(Identity {
            subject,
            email: get("Remote-Email"),
            name: get("Remote-Name"),
        });
    }
    dev_user.filter(|s| !s.is_empty()).map(|d| Identity {
        subject: d.to_string(),
        email: None,
        name: Some(d.to_string()),
    })
}

pub fn is_hx(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

impl<S: Send + Sync> FromRequestParts<S> for HxRequest {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(HxRequest(is_hx(&parts.headers)))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for UserId {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<UserId>()
            .copied()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue};

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn proxy_headers_win() {
        let id = resolve_identity(
            &hm(&[("Remote-User", "sub"), ("Remote-Email", "e@x")]),
            Some("dev"),
        );
        assert_eq!(id.unwrap().subject, "sub");
    }

    #[test]
    fn dev_fallback_when_no_headers() {
        let id = resolve_identity(&hm(&[]), Some("dev")).unwrap();
        assert_eq!(id.subject, "dev");
    }

    #[test]
    fn none_when_unauthenticated() {
        assert!(resolve_identity(&hm(&[]), None).is_none());
    }

    #[test]
    fn hx_header_detection() {
        assert!(is_hx(&hm(&[("HX-Request", "true")])));
        assert!(!is_hx(&hm(&[])));
    }
}
