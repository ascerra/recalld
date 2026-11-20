//! Localhost-only access control for `recalld ui` (SPEC §1.1).
//!
//! - Every request must carry `Host: 127.0.0.1:<port>`, `localhost:<port>` or
//!   `recalld.localhost:<port>` (blocks DNS rebinding; `*.localhost` always
//!   resolves to loopback in browsers, RFC 6761).
//! - A per-launch token, passed once in the opened URL (`?t=…`), is traded
//!   for an `HttpOnly; SameSite=Strict` cookie; every `/api/ui/*` call needs it.
//! - Mutations additionally need `X-Recalld-UI: 1`.
//! - No CORS: no `Access-Control-*` header is ever sent and preflights are
//!   refused.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::http::UiState;

/// Header every mutating `/api/ui/*` request must carry.
pub const MUTATION_HEADER: &str = "x-recalld-ui";

/// A fresh random launch token (256 bits, hex).
pub fn new_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Cookie name, per port: cookies are shared across ports on one host.
pub fn cookie_name(port: u16) -> String {
    format!("recalld_ui_{port}")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn host_allowed(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    ["127.0.0.1", "localhost", "recalld.localhost"]
        .iter()
        .any(|name| host == format!("{name}:{port}"))
}

fn cookie_token<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(k, v)| (k == name).then_some(v))
}

fn query_token(query: Option<&str>) -> Option<&str> {
    query?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(k, v)| (k == "t").then_some(v))
}

pub(crate) fn forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(super::wire::UiError {
            error: "forbidden".into(),
            message: message.into(),
        }),
    )
        .into_response()
}

/// The whole policy, applied to every request before routing.
pub async fn guard(State(state): State<UiState>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    if !host_allowed(headers, state.port) {
        return forbidden("unexpected Host header");
    }
    if request.method() == Method::OPTIONS {
        return forbidden("cross-origin requests are not allowed");
    }
    let path = request.uri().path();
    let name = cookie_name(state.port);
    let authorized = cookie_token(headers, &name)
        .is_some_and(|t| constant_time_eq(t.as_bytes(), state.token.as_bytes()));

    if path.starts_with("/api/") {
        if !authorized {
            return forbidden("open the link printed by `recalld ui`");
        }
        let mutating = !matches!(*request.method(), Method::GET | Method::HEAD);
        if mutating && headers.get(MUTATION_HEADER).and_then(|v| v.to_str().ok()) != Some("1") {
            return forbidden("missing X-Recalld-UI header");
        }
        return with_security_headers(next.run(request).await);
    }

    // Page loads: trade a valid `?t=` for the cookie, then drop it from the URL.
    if let Some(token) = query_token(request.uri().query()) {
        if !constant_time_eq(token.as_bytes(), state.token.as_bytes()) {
            return forbidden("this link has expired; run `recalld ui` again");
        }
        let cookie = format!("{name}={}; HttpOnly; SameSite=Strict; Path=/", state.token);
        let mut response = Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(header::LOCATION, path)
            .header(header::SET_COOKIE, cookie)
            .body(Body::empty())
            .expect("static response");
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return with_security_headers(response);
    }
    with_security_headers(next.run(request).await)
}

fn with_security_headers(mut response: Response) -> Response {
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: blob:; font-src 'self'; worker-src 'self' blob:; connect-src 'self'; \
             frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_must_be_loopback_with_our_port() {
        let mut h = HeaderMap::new();
        assert!(!host_allowed(&h, 7690));
        for (host, ok) in [
            ("127.0.0.1:7690", true),
            ("localhost:7690", true),
            ("recalld.localhost:7690", true),
            ("evil.localhost:7690", false),
            ("127.0.0.1:7691", false),
            ("evil.example:7690", false),
            ("127.0.0.1", false),
            ("[::1]:7690", false),
        ] {
            h.insert(header::HOST, HeaderValue::from_str(host).unwrap());
            assert_eq!(host_allowed(&h, 7690), ok, "{host}");
        }
    }

    #[test]
    fn tokens_are_random_and_compared_exactly() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(constant_time_eq(a.as_bytes(), a.as_bytes()));
        assert!(!constant_time_eq(a.as_bytes(), b.as_bytes()));
        assert!(!constant_time_eq(a.as_bytes(), &a.as_bytes()[1..]));
    }

    #[test]
    fn reads_tokens_from_cookie_and_query() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("other=1; recalld_ui_7690=abc; x=y"),
        );
        assert_eq!(cookie_token(&h, "recalld_ui_7690"), Some("abc"));
        assert_eq!(cookie_token(&h, "recalld_ui_7691"), None);
        assert_eq!(query_token(Some("a=1&t=xyz")), Some("xyz"));
        assert_eq!(query_token(Some("tt=1")), None);
    }
}
