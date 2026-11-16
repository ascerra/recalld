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
    failure_response(store_api1::failure(reply))
}
fn failure_response((status, body): (u16, store_api1::Failure)) -> Response {
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
    let mut reply = match producer.store_api1(request.clone()).await {
        store_api1::Reply::Owned(reply) => reply,
        store_api1::Reply::Uncertain(receipt) => {
            return failure_response(store_api1::uncertain_evidence(receipt));
        }
        store_api1::Reply::Legacy(_) => return error(native_single::Reply::uncertain()),
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

#[derive(Deserialize)]
pub(super) struct BatchRequest {
    memories: Vec<Request>,
}

pub(super) async fn batch(state: AppState, body: BatchRequest) -> Response {
    use crate::serialization::batch_api1::{self, Surface};
    if body.memories.is_empty() || body.memories.len() > crate::model::constants::MAX_BATCH_MEMORIES
    {
        return AppError::BadRequest {
            message: "memories must contain 1–100 items".into(),
            field: Some("memories".into()),
        }
        .into_response();
    }
    let native = match state.storage.native_store_format().await {
        Ok(v) => v,
        Err(e) => return AppError::from(e).into_response(),
    };
    if !native {
        if body.memories.iter().any(|m| m.profile.is_some()) {
            return error(native_single::Reply::invalid(InputError {
                code: "UNSUPPORTED_CONSTRAINT",
                field: "/memories/embeddingProfileId".into(),
                message: "legacy storage cannot verify an explicit embedding profile ID",
            }));
        }
        return super::handlers::batch_store(
            State(state),
            Json(super::models::BatchStoreRequest {
                memories: body.memories.into_iter().map(|m| m.memory).collect(),
            }),
        )
        .await
        .into_response();
    }
    let Some(producer) = state.native_batches.as_ref() else {
        return batch_reply(batch_api1::Reply::Owned(Err(
            crate::serialization::native_batch::BatchFailure::unavailable(),
        )));
    };
    let items=body.memories.into_iter().map(|m|{
        let r=m.memory;
        let mut value=serde_json::json!({"summary":r.summary,"namespace":r.namespace,"tags":r.tags,"entities":r.entities,"topics":r.topics,"emotions":r.emotions});
        // Match the typed REST extractor's omission/null semantics. These are
        // caller values, not sampled defaults. Never derive a profile descriptor.
        if let Some(v)=r.full_text {value["fullText"]=serde_json::json!(v);}
        if let Some(v)=r.embedding {value["embedding"]=serde_json::json!(v);}
        if let Some(v)=r.initial_stability {value["initialStability"]=serde_json::json!(v);}
        if let Some(v)=r.created_at {value["createdAt"]=serde_json::json!(v);}
        if let Some(v)=r.parent_id {value["parentId"]=serde_json::json!(v);}
        if let Some(v)=r.supersedes {value["supersedes"]=serde_json::json!(v);}
        if let Some(v)=r.check_duplicates {value["checkDuplicates"]=serde_json::json!(v);}
        if let Some(v)=m.profile {value["embeddingProfileId"]=v;}
        value
    }).collect();
    let input = crate::storage::batch::BatchInput {
        request_context: None,
        expected_generation: None,
        result_version: 1,
        mutation_version: 1,
        kind: crate::storage::batch::BatchKind::Insert,
        items,
        default_namespace: state.default_namespace.clone(),
        store_id: None,
        idempotency_key: None,
        batch_operation_id: None,
    };
    batch_reply(
        producer
            .store_batch_api1(batch_api1::Request {
                input,
                surface: Surface::Rest,
            })
            .await,
    )
}
fn batch_reply(reply: crate::serialization::batch_api1::Reply) -> Response {
    let (status, value) = reply.response(crate::serialization::batch_api1::Surface::Rest);
    (
        axum::http::StatusCode::from_u16(status).expect("fixed status"),
        Json(value),
    )
        .into_response()
}
