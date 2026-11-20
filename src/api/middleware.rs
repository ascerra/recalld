//! Custom middleware for the API server.
//!
//! Provides request ID injection for tracing correlation. Every request
//! gets a unique ID that is attached to the tracing span, inserted
//! into request extensions (for handler access), and included in the
//! `X-Request-ID` response header.

use axum::{
    body::Body,
    http::{HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;
use uuid::Uuid;

/// Generates a UUID v4 request ID, attaches it to the tracing span,
/// and includes it in the `X-Request-ID` response header.
///
/// If the client sends an `X-Request-ID` header, it is preserved
/// rather than overwritten -- this supports distributed tracing.
pub async fn request_id_middleware(req: Request<Body>, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(128).collect::<String>())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Attach to tracing span
    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %req.method(),
        path = %req.uri().path(),
    );

    let mut response = next.run(req).instrument(span).await;

    if let Ok(val) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", val);
    }

    response
}

pub(crate) fn request_contract_version(
    headers: &axum::http::HeaderMap,
) -> Result<crate::serialization::contract::ContractVersion, String> {
    use crate::serialization::contract::{ContractVersion, VERSION_HEADER};
    let mut values = headers.get_all(VERSION_HEADER).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err("only one Recalld-Contract-Version header is allowed".into());
    }
    first
        .map(|value| value.to_str())
        .transpose()
        .map_err(|_| "contract version header must be ASCII".to_owned())
        .and_then(|value| ContractVersion::from_http(value).map_err(|e| e.to_string()))
}

/// The one operation published at contract 2 (#237): numeric namespace policy
/// replacement, `PUT /namespaces/{id}/policy`. Contract 2 stays refused elsewhere.
pub(crate) fn is_policy_replacement(method: &axum::http::Method, path: &str) -> bool {
    method == axum::http::Method::PUT
        && path
            .strip_prefix("/namespaces/")
            .and_then(|rest| rest.strip_suffix("/policy"))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

/// Admit application contract selection before any REST handler/backend work.
pub async fn contract_version_middleware(req: Request<Body>, next: Next) -> Response {
    use crate::serialization::contract::ContractVersion;
    use axum::response::IntoResponse;
    let policy = is_policy_replacement(req.method(), req.uri().path());
    match request_contract_version(req.headers()).and_then(|version| match version {
        ContractVersion::V2 if policy => Ok(()),
        _ => version.require_supported().map_err(|e| e.to_string()),
    }) {
        Ok(()) => next.run(req).await,
        Err(message) => super::AppError::BadRequest {
            message,
            field: Some("contractVersion".into()),
        }
        .into_response(),
    }
}
