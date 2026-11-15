//! Physical-format-aware API1 POST /memories; successful old shape is retained.
use super::{AppState, errors::AppError, models::CreateMemoryApiRequest};
use crate::serialization::{contract::InputError, native_single, store_api1};
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub(super) struct Request {
    #[serde(flatten)]
    memory: CreateMemoryApiRequest,
    #[serde(rename = "embeddingProfileId", default, deserialize_with = "present")]
    profile: Option<Value>,
}
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
fn error(reply: native_single::Reply) -> Response {
    let (status, body) = store_api1::failure(reply);
    (
        axum::http::StatusCode::from_u16(status).unwrap(),
        Json(body),
    )
        .into_response()
}
pub(super) async fn create(state: AppState, body: Request) -> Response {
    let started = std::time::Instant::now();
    let profile_id = match body.profile {
        None => None,
        Some(value) => match value
            .as_str()
            .filter(|s| crate::embedding::profile::valid_profile_id(s))
        {
            Some(id) => Some(id.to_owned()),
            None => {
                return error(native_single::Reply::invalid(InputError {
                    code: "INVALID_INPUT",
                    field: "/embeddingProfileId".into(),
                    message: "embeddingProfileId must be an ep1 profile ID",
                }));
            }
        },
    };
    let native = match state.storage.native_store_format().await {
        Ok(native) => native,
        Err(e) => return AppError::from(e).into_response(),
    };
    if !native {
        if profile_id.is_some() {
            return error(native_single::Reply::invalid(InputError {
                code: "UNSUPPORTED_CONSTRAINT",
                field: "/embeddingProfileId".into(),
                message: "legacy storage cannot verify an explicit embedding profile ID",
            }));
        }
        return super::handlers::create_memory(State(state), Json(body.memory))
            .await
            .into_response();
    }
    let Some(producer) = state.native_batches.as_ref() else {
        return error(native_single::Reply::failure(
            crate::storage::StorageError::RecoveryRequired(
                "native API1 store producer unavailable".into(),
            ),
        ));
    };
    let r = body.memory;
    let request = store_api1::Request {
        input: crate::mcp::bridge::StoreInput {
            summary: r.summary,
            full_text: r.full_text,
            tags: r.tags,
            entities: r.entities,
            topics: r.topics,
            emotions: r.emotions,
            namespace: r.namespace,
            embedding: r.embedding,
            initial_stability: r.initial_stability,
            parent_id: r.parent_id,
            supersedes: r.supersedes,
            check_duplicates: r.check_duplicates,
        },
        profile_id,
        created_at: r.created_at,
        require_native: true,
    };
    let store_api1::Reply::Owned(mut reply) = producer.store_api1(request.clone()).await else {
        return error(native_single::Reply::uncertain());
    };
    if reply.error.is_some() {
        return error(reply);
    }
    match store_api1::memory_response(&request, &reply) {
        Ok(data) => (
            axum::http::StatusCode::CREATED,
            Json(crate::serialization::ApiResponse {
                data,
                took_us: Some(started.elapsed().as_micros() as u64),
            }),
        )
            .into_response(),
        Err(source) => {
            let source = match reply.receipt.clone() {
                Some(receipt) => crate::storage::StorageError::CommittedMutation {
                    receipt: Box::new(receipt),
                    source: Box::new(source),
                },
                None => source,
            };
            reply.error = native_single::Reply::failure(source).error;
            reply.data = None;
            error(reply)
        }
    }
}
