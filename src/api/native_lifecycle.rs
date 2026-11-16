//! Versioned route adapters. The shared public version gate remains closed.
use super::state::AppState;
use crate::serialization::{
    contract::{ContractVersion, InputError, KEY_HEADER},
    native_lifecycle::{Operation, Reply},
};
use axum::{
    Json,
    extract::{FromRequest, Path, Query, Request, State},
    response::{IntoResponse, Response},
};

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    dispatch(state, id, request, Operation::Forget).await
}
pub(crate) async fn feedback(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    dispatch(state, id, request, Operation::Feedback).await
}
async fn dispatch(state: AppState, id: String, request: Request, operation: Operation) -> Response {
    let version = match super::middleware::request_contract_version(request.headers())
        .and_then(|v| v.require_supported().map(|_| v).map_err(|e| e.to_string()))
    {
        Ok(version) => version,
        Err(message) => {
            return super::errors::AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if version == ContractVersion::V1 {
        return match operation {
            Operation::Forget => {
                match Query::<super::models::MemoryScopeQuery>::try_from_uri(request.uri()) {
                    Ok(scope) => super::handlers::delete_memory(State(state), Path(id), scope)
                        .await
                        .into_response(),
                    Err(e) => e.into_response(),
                }
            }
            Operation::Feedback => {
                match Json::<serde_json::Value>::from_request(request, &state).await {
                    Ok(body) => super::handlers::reinforce_memory(State(state), Path(id), body)
                        .await
                        .into_response(),
                    Err(e) => e.into_response(),
                }
            }
        };
    }
    prepared(state, id, request, operation).await
}
/// Actual route body after version negotiation, exercised without opening public2.
pub(crate) async fn prepared(
    state: AppState,
    id: String,
    request: Request,
    operation: Operation,
) -> Response {
    let invalid = |field: &str, message: &'static str| {
        Reply::input(InputError {
            code: "INVALID_TYPE",
            field: field.into(),
            message,
        })
        .http()
    };
    let mut keys = request.headers().get_all(KEY_HEADER).iter();
    let key = keys.next();
    if keys.next().is_some() {
        return invalid("/idempotencyKey", "only one key header is allowed");
    }
    let key = match key.map(|v| v.to_str()).transpose() {
        Ok(v) => v.map(str::to_owned),
        Err(_) => return invalid("/idempotencyKey", "key header must be ASCII"),
    };
    let query =
        match Query::<std::collections::BTreeMap<String, String>>::try_from_uri(request.uri()) {
            Ok(v) => v.0,
            Err(_) => return invalid("", "invalid query parameters"),
        };
    if query.keys().any(|k| k != "namespace") {
        return invalid("", "unsupported query parameter");
    }
    let bytes = match axum::body::to_bytes(request.into_body(), 2 * 1024 * 1024).await {
        Ok(v) => v,
        Err(_) => return invalid("", "invalid lifecycle body"),
    };
    let mut value: serde_json::Value = if bytes.is_empty() && operation == Operation::Forget {
        serde_json::json!({})
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return invalid("", "expected a JSON object"),
        }
    };
    let Some(object) = value.as_object_mut() else {
        return invalid("", "expected a JSON object");
    };
    if object.contains_key("id") {
        return invalid("/id", "HTTP target comes from path only");
    }
    object.insert("id".into(), serde_json::json!(id));
    if let Some(namespace) = query.get("namespace") {
        if object
            .get("namespace")
            .is_some_and(|v| v.as_str() != Some(namespace.as_str()))
        {
            return invalid("/namespace", "body and query namespace must agree");
        }
        object.insert("namespace".into(), serde_json::json!(namespace));
    }
    let parsed = match crate::serialization::native_lifecycle::parse(
        &value,
        operation,
        false,
        key.as_deref(),
    ) {
        Ok(v) => v,
        Err(e) => return Reply::input(e).http(),
    };
    match operation {
        Operation::Forget => state.storage.forget_native(parsed).await.http(),
        Operation::Feedback => state.decay.feedback_native(parsed).await.http(),
    }
}
