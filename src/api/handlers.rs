//! Request handler functions for all API endpoints.
//!
//! Each handler is an `async fn` that receives axum extractors and
//! returns `Result<impl IntoResponse, AppError>`. All handlers follow
//! the same pattern: extract, validate, delegate to subsystem, convert
//! result, respond.

use crate::decay::observation::Observed;
use std::time::Instant;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::errors::AppError;
use super::models::*;
use super::state::{AppState, NamespaceRegistry, QueryInput, SearchQuery, StorageEngine};
use crate::health::report as health_report_compute;
use crate::model::DecayPhase;
use crate::model::constants::{
    EMOTION_TAG_PREFIX, ENTITY_TAG_PREFIX, MAX_BATCH_MEMORIES, TOPIC_TAG_PREFIX,
};
use crate::model::id::{MemoryId, NamespaceId};
use crate::model::memory::AccessKind;
use crate::model::validation::{MemoryInputRef, validate_legacy_memory_input};
use crate::serialization::{
    ApiResponse, MemoryResponse, NamespaceResponse, SearchHit, SearchRequest, SearchResponse,
};

/// Keep shared parser field names in HTTP validation errors.
fn argument_error(error: crate::mcp::args::ArgError) -> AppError {
    AppError::BadRequest {
        message: error.message,
        field: (!error.field.is_empty()).then_some(error.field),
    }
}

/// Duration for which a cached health report is considered fresh.
const HEALTH_REPORT_CACHE_TTL_SECS: u64 = 60;

/// Per-scope cached health reports.
///
/// Uses a lazily-initialized global to avoid threading the cache through
/// `AppState` (which would require a schema change beyond the scope of
/// this fix). The `RwLock` ensures concurrent readers don't block each
/// other and only one writer recomputes at a time.
fn health_report_cache()
-> &'static RwLock<std::collections::HashMap<String, (u64, Instant, HealthReport)>> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<
        RwLock<std::collections::HashMap<String, (u64, Instant, HealthReport)>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()))
}

/// Build the near-duplicate advisory for a memory about to be created.
///
/// Shared by the single and batch create endpoints so the two HTTP doors
/// cannot drift, and mirroring `McpStorageAdapter::store_memory` so the
/// HTTP and MCP doors cannot either. `MemoryResponse.supersedes` exists
/// for exactly that reason.
///
/// **Call this BEFORE the memory is indexed.** `state.search.index_memory`
/// puts the new vector in the index; run after that and the memory matches
/// itself at score 1.0.
///
/// Never fails a create: every error path inside
/// [`find_near_duplicates`](crate::search::find_near_duplicates) yields no
/// matches.
///
/// The Full/Summary guarantee is enforced HERE, not by the
/// `decay_phases` filter `find_near_duplicates` hands the vector index:
/// `VectorIndexOwner` writes each entry's `decay_phase` once at add time
/// (always Full) and `VectorIndex::update_metadata` has no callers, so
/// that filter is inert in production. A candidate whose record is
/// missing or past Summary is dropped here instead.
async fn near_duplicate_report(
    state: &AppState,
    embedding: &[f32],
    namespace_id: NamespaceId,
    check_duplicates: Option<bool>,
) -> Result<Option<crate::model::NearDuplicateReport>, AppError> {
    let stamp = state.storage.effects_stamp().await?;
    use crate::model::constants::{
        DUPLICATE_SIMILARITY_THRESHOLD, MAX_NEAR_DUPLICATE_MATCHES,
        NEAR_DUPLICATE_SUMMARY_MAX_BYTES,
    };
    use crate::model::{DecayPhase, NearDuplicateMatch, NearDuplicateReport, truncate_summary};

    if !check_duplicates.unwrap_or(true) {
        return Ok(None);
    }

    let candidates = state
        .search
        .near_duplicates(
            embedding,
            namespace_id,
            DUPLICATE_SIMILARITY_THRESHOLD as f32,
            MAX_NEAR_DUPLICATE_MATCHES,
        )
        .await?;

    let mut matches = Vec::with_capacity(candidates.len());
    for (id, score) in candidates {
        // `get_or_load` reads through the cache without recording an
        // access — the endpoints that count a read call `record_access`
        // separately afterwards. Loading a summary to warn about a
        // duplicate is not a retrieval of that memory.
        let Some(record) = state.cache.get_or_load(id, state.storage.as_ref()).await else {
            // Hard-deleted, and the vector index has not caught up.
            continue;
        };
        // Ghost has no summary left to show and Tombstone is deleted;
        // neither is something to be told it duplicates.
        if !matches!(record.phase, DecayPhase::Full | DecayPhase::Summary) {
            continue;
        }
        matches.push(NearDuplicateMatch {
            id,
            score,
            summary: truncate_summary(&record.summary, NEAR_DUPLICATE_SUMMARY_MAX_BYTES),
        });
    }

    if state.storage.effects_stamp().await? != stamp {
        return Err(crate::storage::StorageError::RecoveryRequired(
            "effects changed during advisory hydration".into(),
        )
        .into());
    }
    Ok(NearDuplicateReport::from_matches(
        DUPLICATE_SIMILARITY_THRESHOLD,
        matches,
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Memory CRUD
// ═══════════════════════════════════════════════════════════════════════

/// POST /memories -- create a new memory.
///
/// Steps:
/// 1. Validate request content limits (see [`validate_legacy_memory_input`]);
///    all length limits are UTF-8 bytes, not characters.
/// 2. Resolve namespace by name -> NamespaceId.
/// 3. Validate `parent_id` / `supersedes` targets, BEFORE any write, so a
///    bad link returns 422 without leaving an orphaned memory behind.
/// 4. Generate embedding if not provided (calls embedding provider).
/// 5. Validate embedding dimensionality against namespace config.
/// 6. Persist to storage (meta.db, fulltext.dat, vectors.dat).
/// 7. Insert into RAM cache and vector index.
/// 8. Create the `parent_id` and `supersedes` edges in the graph.
/// 9. Return 201 with the created memory.
pub async fn single_store_versioned(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use crate::serialization::{
        contract::{ContractVersion, InputError, KEY_HEADER},
        native_single::Reply,
    };
    use axum::{extract::FromRequest, response::IntoResponse};
    let version = match super::middleware::request_contract_version(request.headers()) {
        Ok(version) => version,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(error) = version.require_supported() {
        return single_reply_response(Reply::invalid(error));
    }
    if version == ContractVersion::V1 {
        return match Json::<super::store_api1::Request>::from_request(request, &state).await {
            Ok(Json(body)) => super::store_api1::create(state, body).await,
            Err(error) => error.into_response(),
        };
    }
    let mut keys = request.headers().get_all(KEY_HEADER).iter();
    let key = keys
        .next()
        .map(|v| v.to_str().map(str::to_owned))
        .transpose();
    if keys.next().is_some() || key.is_err() {
        return single_reply_response(Reply::invalid(InputError {
            code: "INVALID_TYPE",
            field: "/idempotencyKey".into(),
            message: "at most one ASCII idempotency key header is allowed",
        }));
    }
    let body = match Json::<serde_json::Value>::from_request(request, &state).await {
        Ok(Json(body)) => body,
        Err(_) => {
            return single_reply_response(Reply::invalid(InputError {
                code: "INVALID_TYPE",
                field: String::new(),
                message: "body must be a JSON object",
            }));
        }
    };
    let Some(producer) = state.native_batches.as_deref() else {
        return single_reply_response(Reply::failure(
            crate::storage::StorageError::RecoveryRequired(
                "native single consumer unavailable".into(),
            ),
        ));
    };
    single_store_v2(
        producer,
        &state.default_namespace,
        &body,
        key.unwrap().as_deref(),
    )
    .await
}
fn single_reply_response(
    reply: crate::serialization::native_single::Reply,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (status, envelope) = reply.response();
    (
        StatusCode::from_u16(status).expect("fixed single status"),
        Json(envelope),
    )
        .into_response()
}
pub(crate) async fn single_store_v2(
    producer: &dyn crate::mcp::bridge::StorageEngine,
    default_namespace: &str,
    body: &serde_json::Value,
    key: Option<&str>,
) -> axum::response::Response {
    use crate::serialization::native_single::{Reply, Request};
    let reply = match Request::parse(body, true, key, default_namespace) {
        Ok(request) => producer.store_native_wire(request).await,
        Err(error) => Reply::invalid(error),
    };
    single_reply_response(reply)
}

pub async fn create_memory(
    State(state): State<AppState>,
    Json(req): Json<CreateMemoryApiRequest>,
) -> Result<(StatusCode, Json<ApiResponse<MemoryResponse>>), AppError> {
    let work = state.search.accept_work()?;
    let result = async {
        state.storage.effects_stamp().await?;

        let start = Instant::now();

        // --- Validation ---
        validate_legacy_memory_input(MemoryInputRef {
            summary: &req.summary,
            full_text: req.full_text.as_deref(),
            tags: &req.tags,
            entities: &req.entities,
            topics: &req.topics,
            emotions: &req.emotions,
        })?;

        // --- Resolve namespace ---
        let ns = state
            .namespaces
            .resolve(&req.namespace)
            .ok_or_else(|| AppError::NotFound {
                resource: "namespace",
                id: req.namespace.clone(),
            })?;

        // --- Validate caller-specified links BEFORE anything is written ---
        //
        // The memory is persisted further down; a link failure discovered after
        // that would leave an orphaned record behind a 4xx (which is exactly
        // what the parent edge used to do). Checking here also spares an
        // embedding round-trip on a request that cannot succeed.
        //
        // `None` for the new ID: it does not exist until `create_memory`
        // returns, so the self-reference check has nothing to compare against.
        for (field, target) in [("parentId", req.parent_id), ("supersedes", req.supersedes)] {
            if let Some(target) = target {
                state
                    .graph
                    .check_link_target(field, None, target, ns.id)
                    .await?;
            }
        }

        // --- Merge entities/topics/emotions into tags (Issue 8) ---
        // The prefixes come from `constants` because the label length limits
        // reserve room for exactly these bytes.
        let mut merged_tags = req.tags.clone();
        for entity in &req.entities {
            let tag = format!("{ENTITY_TAG_PREFIX}{}", entity.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
        }
        for topic in &req.topics {
            let tag = format!("{TOPIC_TAG_PREFIX}{}", topic.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
        }
        for emotion in &req.emotions {
            let tag = format!("{EMOTION_TAG_PREFIX}{}", emotion.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
        }

        // --- Embedding (Issue 9: embed summary + full_text + tags) ---
        let raw_embedding = match req.embedding {
            Some(ref vec) => {
                if vec.len() != ns.embedding_dim as usize {
                    return Err(AppError::UnprocessableEntity {
                        message: format!(
                            "embedding has {} dimensions, namespace '{}' requires {}",
                            vec.len(),
                            ns.name,
                            ns.embedding_dim
                        ),
                        field: Some("embedding".into()),
                    });
                }
                vec.clone().into()
            }
            None => {
                // Build embedding text from summary + full_text + tags (matches MCP)
                let mut embed_text = match &req.full_text {
                    Some(ft) => format!("{}\n\n{}", req.summary, ft),
                    None => req.summary.clone(),
                };
                if !merged_tags.is_empty() {
                    embed_text = format!("{} {}", embed_text, merged_tags.join(" "));
                }
                state.search.embed_text(&embed_text, &ns).await?
            }
        };

        crate::embedding::runtime::check_namespace(&ns, raw_embedding.profile.as_ref())
            .map_err(|e| crate::embedding::EmbeddingError::Unavailable(e.to_string()))?;
        let mut embedding = raw_embedding.values.clone();
        crate::storage::insertion::normalize_embedding(
            &mut embedding,
            ns.embedding_dim,
            req.embedding.is_some(),
        )?;

        // --- Near-duplicate advisory ---
        // Before the write, so the new memory is not yet in the vector index
        // and cannot match itself. See `near_duplicate_report`.
        let near_duplicates =
            near_duplicate_report(&state, &embedding, ns.id, req.check_duplicates).await?;

        // Canonical record + explicit links share one receipt/effects transaction.
        // Completion publishes all indexes before this returns.
        let inserted = state
            .storage
            .insert_memory_with_work(
                ns.id,
                crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: req.summary.clone(),
                    full_text: req.full_text.clone(),
                    tags: crate::model::parse_tags_lossy("create_memory.tags", &merged_tags),
                    embedding: req.embedding.clone(),
                    initial_stability: req.initial_stability,
                    created_at: req.created_at,
                    parent_id: req.parent_id,
                    supersedes: req.supersedes,
                },
                raw_embedding,
                work.as_ref(),
            )
            .await?;
        let receipt = inserted.receipt;
        let memory = inserted.record.ok_or_else(|| {
            AppError::from(crate::storage::StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(crate::storage::StorageError::RecoveryRequired(
                    "inserted record unavailable".into(),
                )),
            })
        })?;
        let supersedes_outcome = req.supersedes.map(|id| {
            crate::model::SupersedesOutcome::new(id, crate::model::SupersedesStatus::Applied)
        });

        // --- Autolink, entity-link, temporal-link (Issue 12) ---
        let created_at = memory.created_at;
        state
            .graph
            .post_creation_links_with_work(
                memory.id,
                ns.id,
                &embedding,
                &merged_tags,
                &req.entities,
                created_at,
                work.as_ref(),
            )
            .await
            .map_err(|e| crate::storage::StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(crate::storage::StorageError::RecoveryRequired(
                    e.to_string(),
                )),
            })?;

        state
            .storage
            .complete_insertion_with_work(&receipt, work.as_ref())
            .await?;
        state.cache.insert(&memory).await;

        let mut response = MemoryResponse::from_cached(&memory, ns.name.clone());
        response.full_text = req.full_text;
        response.supersedes = supersedes_outcome;
        response.near_duplicates = near_duplicates;
        let took = start.elapsed().as_micros() as u64;

        Ok((
            StatusCode::CREATED,
            Json(ApiResponse {
                data: response,
                took_us: Some(took),
            }),
        ))
    }
    .await;
    state.search.finish_work(work);
    result
}

/// GET /memories/:id -- retrieve a single memory.
///
/// Steps:
/// 1. Parse UUID from path.
/// 2. Look up in cache (hit) or load from storage (miss).
/// 3. Record an access event (`DirectRetrieval`).
/// 4. Return 200 with memory, or 404 if not found.
/// Prepared inner consumer; the public version gate remains unchanged.
pub(crate) async fn prepared_get(
    search: &dyn crate::api::state::SearchPipeline,
    id: crate::model::MemoryId,
    arguments: &serde_json::Value,
) -> (StatusCode, Json<crate::serialization::get::GetReply>) {
    let reply = match crate::serialization::get::GetRequest::from_http(arguments, id) {
        Ok(request) => {
            search
                .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
                .await
        }
        Err(error) => crate::serialization::get::GetReply::input(error),
    };
    (
        StatusCode::from_u16(reply.http_status).expect("get HTTP status"),
        Json(reply),
    )
}

pub async fn get_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(options): Query<GetMemoryQuery>,
) -> Result<Json<ApiResponse<MemoryResponse>>, AppError> {
    let work = state.search.accept_work()?;
    let result = async {
        let stamp = state.storage.effects_stamp().await?;
        let observed = get_memory_admitted(
            State(state.clone()),
            Path(id),
            Query(options),
            stamp,
            work.as_ref(),
        )
        .await;
        observed?.finish(check_read_admission(&state, stamp).await)
    }
    .await;
    state.search.finish_work(work);
    result
}

async fn get_memory_admitted(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(options): Query<GetMemoryQuery>,
    admitted_stamp: u64,
    parent: Option<&crate::storage::work::WorkTicket>,
) -> Result<Observed<Json<ApiResponse<MemoryResponse>>>, AppError> {
    let start = Instant::now();

    let memory_id = parse_memory_id(&id)?;
    if let Some(prepared) = state
        .search
        .prepare_coherent_get_with_work(
            memory_id,
            options.namespace.as_deref(),
            options.record_access,
            parent,
        )
        .await?
    {
        let (namespace, row) = prepared.payload.ok_or_else(|| AppError::NotFound {
            resource: "memory",
            id: id.clone(),
        })?;
        let mut response = MemoryResponse::from_cached(
            &crate::model::CachedRecord::from(&row.record),
            namespace.name,
        );
        if row.record.schema.as_ref().is_some_and(|s| {
            s.lifecycle.lifecycle_version
                == crate::model::schema::LifecycleVersion::ExplicitFeedbackV2
        }) {
            if let Some(projected) = prepared.response {
                response.strength = projected.strength;
                response.decay_strength = projected.decay_strength;
            }
        }
        response.full_text = row.full_text;
        check_read_admission(&state, admitted_stamp).await?;
        let receipts = if prepared.requested_access && row.record.phase != DecayPhase::Tombstone {
            state
                .search
                .record_coherent_exposures_with_work(
                    &[(memory_id, AccessKind::DirectRetrieval)],
                    crate::storage::control::ExposureScope {
                        namespace: response.namespace.clone(),
                        source: Some(memory_id),
                    },
                    prepared.evaluated_at,
                    parent,
                )
                .await?
        } else {
            Vec::new()
        };
        return Ok(Observed {
            receipts,
            value: Json(ApiResponse {
                data: response,
                took_us: Some(start.elapsed().as_micros() as u64),
            }),
        });
    }
    if let Some(namespace) = options.namespace.as_deref() {
        state
            .storage
            .assert_memory_namespace(memory_id, namespace)
            .await?;
    }

    let record = if options.record_access {
        state
            .cache
            .get_or_load(memory_id, state.storage.as_ref())
            .await
    } else {
        state
            .storage
            .get_record(memory_id)
            .await
            .map(|r| std::sync::Arc::new(crate::model::CachedRecord::from(&r)))
    }
    .ok_or_else(|| AppError::NotFound {
        resource: "memory",
        id: id.clone(),
    })?;

    let ns_name = state
        .namespaces
        .name_for(record.namespace_id)
        .unwrap_or_else(|| "unknown".to_string());

    let mut response = MemoryResponse::from_cached(&record, ns_name);
    response.full_text = state.storage.get_full_text(memory_id).await?;
    check_read_admission(&state, admitted_stamp).await?;
    let receipts = if options.record_access && record.phase != DecayPhase::Tombstone {
        state
            .search
            .record_exposures_with_work(
                &[(memory_id, AccessKind::DirectRetrieval)],
                crate::storage::control::ExposureScope {
                    namespace: response.namespace.clone(),
                    source: Some(memory_id),
                },
                parent,
            )
            .await?
    } else {
        Vec::new()
    };
    let took = start.elapsed().as_micros() as u64;

    Ok(Observed {
        receipts,
        value: Json(ApiResponse {
            data: response,
            took_us: Some(took),
        }),
    })
}

/// DELETE /memories/:id -- delete a memory.
///
/// Steps:
/// 1. Parse UUID from path.
/// 2. Remove from storage (meta.db, fulltext.dat pointer, vectors.dat slot).
/// 3. Remove from cache and vector index.
/// 4. Clean up graph edges (both directions).
/// 5. Return 200 with deletion confirmation.
pub async fn delete_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(scope): Query<MemoryScopeQuery>,
) -> Result<Json<ApiResponse<DeleteResponse>>, AppError> {
    let start = Instant::now();
    let memory_id = parse_memory_id(&id)?;
    let mut reply = state
        .storage
        .forget_api1(memory_id, scope.namespace.as_deref())
        .await;
    let Some(crate::serialization::lifecycle_api1::Value::Forget { deleted }) = reply.value.take()
    else {
        return Err(reply.http_error(&id));
    };
    Ok(Json(ApiResponse {
        data: DeleteResponse {
            id: memory_id,
            deleted,
        },
        took_us: Some(start.elapsed().as_micros() as u64),
    }))
}

/// POST /memories/:id/reinforce -- manual reinforcement.
///
/// Steps:
/// 1. Parse UUID from path.
/// 2. Verify memory exists.
/// 3. Apply FSRS reinforcement with the given quality rating.
/// 4. Update storage and cache with new strength/stability.
/// 5. Return updated decay parameters.
pub async fn reinforce_memory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<ReinforceResponse>>, AppError> {
    let start = Instant::now();
    let memory_id = parse_memory_id(&id)?;
    let namespace = crate::mcp::args::parse_namespace_assertion(&req).map_err(argument_error)?;
    let quality = crate::mcp::args::parse_quality(&req).map_err(argument_error)?;
    let mut reply = state
        .decay
        .feedback_api1(memory_id, quality, namespace)
        .await;
    let Some(crate::serialization::lifecycle_api1::Value::Feedback { result }) = reply.value.take()
    else {
        return Err(reply.http_error(&id));
    };
    Ok(Json(ApiResponse {
        data: ReinforceResponse {
            id: memory_id,
            strength: result.strength,
            stability: result.stability,
            phase: result.phase,
            is_permastore: result.is_permastore,
        },
        took_us: Some(start.elapsed().as_micros() as u64),
    }))
}

/// Preserve typed lifecycle outcomes instead of classifying display strings.
fn feedback_error(error: Box<dyn std::error::Error + Send + Sync>, id: &str) -> AppError {
    use crate::decay::lifecycle::LifecycleError;
    match error.downcast_ref::<LifecycleError>() {
        Some(LifecycleError::Observation(error)) => AppError::Observation(error.clone()),
        Some(LifecycleError::NamespaceMismatch(_)) => AppError::NamespaceMismatch {
            message: error.to_string(),
        },
        Some(LifecycleError::ContentUnavailable) => AppError::Conflict {
            message: error.to_string(),
        },
        Some(LifecycleError::NotFound) => AppError::NotFound {
            resource: "memory",
            id: id.into(),
        },
        Some(LifecycleError::InvalidInput(_)) => AppError::BadRequest {
            message: error.to_string(),
            field: None,
        },
        _ => AppError::Internal { source: error },
    }
}

/// GET /memories -- list memories with filtering and pagination.
///
/// Steps:
/// 1. Parse and validate query parameters.
/// 2. Resolve namespace if specified.
/// 3. Request filtered records from storage adapter.
/// 4. Sort results in memory.
/// 5. Apply pagination (offset + limit).
/// 6. Convert to MemoryResponse objects.
/// 7. Return ListMemoriesResponse with pagination metadata.
pub async fn list_memories(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use crate::serialization::contract::ContractVersion;
    use axum::{extract::FromRequestParts, response::IntoResponse};
    let version = match super::middleware::request_contract_version(request.headers()) {
        Ok(version) => version,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(error) = version.require_supported() {
        return AppError::BadRequest {
            message: error.to_string(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    let (mut parts, _) = request.into_parts();
    if version == ContractVersion::V1 {
        let params = match Query::<ListMemoriesQuery>::from_request_parts(&mut parts, &state).await
        {
            Ok(params) => params,
            Err(error) => return error.into_response(),
        };
        return list_memories_v1(State(state), params).await.into_response();
    }
    let Query(params) =
        match Query::<Vec<(String, String)>>::from_request_parts(&mut parts, &state).await {
            Ok(params) => params,
            Err(error) => return error.into_response(),
        };
    list_memories_v2(state.storage.as_ref(), &state.default_namespace, &params).await
}
pub(crate) async fn list_memories_v2(
    storage: &dyn super::state::StorageEngine,
    default_namespace: &str,
    params: &[(String, String)],
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let result = match crate::serialization::list::ListPageRequest::from_http(params) {
        Ok(mut request) => {
            request
                .namespace
                .get_or_insert_with(|| default_namespace.into());
            storage.list_canonical_page(request).await
        }
        Err(error) => Err(error.into()),
    };
    let (status, envelope) = crate::canonical_list::response(result);
    (
        axum::http::StatusCode::from_u16(status).expect("fixed status"),
        Json(envelope),
    )
        .into_response()
}
async fn list_memories_v1(
    State(state): State<AppState>,
    Query(params): Query<ListMemoriesQuery>,
) -> Result<Json<ApiResponse<ListMemoriesResponse>>, AppError> {
    let stamp = state.storage.effects_stamp().await?;
    let result = async {
    let start = Instant::now();

    // Validate limit (cap at 1000)
    let limit = params.limit.unwrap_or(50).min(1000) as usize;
    let offset = params.offset.unwrap_or(0) as usize;

    // Validate phase
    if let Some(ref phase) = params.phase {
        if !["full", "summary", "ghost"].contains(&phase.as_str()) {
            return Err(AppError::BadRequest {
                message: format!("invalid phase '{phase}', must be: full, summary, ghost"),
                field: Some("phase".into()),
            });
        }
    }

    // Validate sort field
    let sort_field = params.sort.as_deref().unwrap_or("created");
    if !["created", "accessed", "strength", "stability"].contains(&sort_field) {
        return Err(AppError::BadRequest {
            message: format!(
                "invalid sort field '{sort_field}', must be: created, accessed, strength, stability"
            ),
            field: Some("sort".into()),
        });
    }

    // Validate order
    let order = params.order.as_deref().unwrap_or("desc");
    if !["asc", "desc"].contains(&order) {
        return Err(AppError::BadRequest {
            message: format!("invalid order '{order}', must be: asc, desc"),
            field: Some("order".into()),
        });
    }

    // Resolve namespace if specified
    let namespace_id = if let Some(ref ns_name) = params.namespace {
        let ns = state
            .namespaces
            .resolve(ns_name)
            .ok_or_else(|| AppError::NotFound {
                resource: "namespace",
                id: ns_name.clone(),
            })?;
        Some(ns.id)
    } else {
        None
    };

    // Issue 19: Use list_memories_filtered() for server-side pagination when namespace is given.
    // Build require_tags from tags + entities (Issue 21).
    let mut require_tags: Vec<crate::model::Tag> =
        crate::model::parse_tags_lossy("list_memories.tags", &params.tags);
    for entity in &params.entities {
        if let Ok(tag) = crate::model::Tag::new(&format!("entity/{}", entity.to_lowercase())) {
            require_tags.push(tag);
        }
    }

    if let Some(ns_id) = namespace_id {
        // Server-side filtered pagination (Issue 19)
        let (page, total) = state
            .storage
            .list_memories_filtered(
                ns_id,
                &require_tags,
                params.time_range_start,
                params.time_range_end,
                offset,
                limit,
            )
            .await?;

        let mut memories: Vec<MemoryResponse> = Vec::with_capacity(page.len());
        for (mid, record) in page {
            let ns_name = state
                .namespaces
                .name_for(NamespaceId::new(record.namespace_id))
                .unwrap_or_else(|| "unknown".to_string());
            let cached = crate::model::CachedRecord::from(&record);
            let mut mem = MemoryResponse::from_cached(&cached, ns_name);
            mem.full_text = state.storage.get_full_text(mid).await.unwrap_or(None);
            memories.push(mem);
        }

        let has_more = (offset + memories.len()) < total as usize;

        let response = ListMemoriesResponse {
            memories,
            total,
            limit: limit as u32,
            offset: offset as u32,
            has_more,
        };

        let took = start.elapsed().as_micros() as u64;

        return Ok(Json(ApiResponse {
            data: response,
            took_us: Some(took),
        }));
    }

    // Fallback: no namespace specified, use in-memory filtering
    let phase_filter = params.phase.as_ref().map(|p| match p.as_str() {
        "full" => crate::model::DecayPhase::Full,
        "summary" => crate::model::DecayPhase::Summary,
        "ghost" => crate::model::DecayPhase::Ghost,
        _ => crate::model::DecayPhase::Full,
    });

    let filter = ListFilter {
        namespace_id,
        phase: phase_filter,
        tags: {
            let mut all_tags = params.tags;
            for entity in &params.entities {
                all_tags.push(format!("entity/{}", entity.to_lowercase()));
            }
            all_tags
        },
        time_range_start: params.time_range_start,
        time_range_end: params.time_range_end,
    };

    let mut records = state.storage.list_memories(&filter).await?;

    match (sort_field, order) {
        ("created", "asc") => records.sort_by_key(|r| r.created_at),
        ("created", "desc") => {
            records.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        }
        ("accessed", "asc") => records.sort_by_key(|r| r.last_accessed_at),
        ("accessed", "desc") => {
            records.sort_by_key(|r| std::cmp::Reverse(r.last_accessed_at));
        }
        ("strength", "asc") => records.sort_by(|a, b| {
            a.decay_strength
                .partial_cmp(&b.decay_strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        ("strength", "desc") => records.sort_by(|a, b| {
            b.decay_strength
                .partial_cmp(&a.decay_strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        ("stability", "asc") => records.sort_by(|a, b| {
            a.stability
                .partial_cmp(&b.stability)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        ("stability", "desc") => records.sort_by(|a, b| {
            b.stability
                .partial_cmp(&a.stability)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        _ => {}
    }

    let total = records.len() as u64;

    let page_records: Vec<_> = records.into_iter().skip(offset).take(limit).collect();

    let mut memories: Vec<MemoryResponse> = Vec::with_capacity(page_records.len());
    for record in page_records {
        let ns_name = state
            .namespaces
            .name_for(record.namespace_id)
            .unwrap_or_else(|| "unknown".to_string());
        let mut mem = MemoryResponse::from_cached(&record, ns_name);
        mem.full_text = state.storage.get_full_text(record.id).await.unwrap_or(None);
        memories.push(mem);
    }

    let has_more = (offset + memories.len()) < total as usize;

    let response = ListMemoriesResponse {
        memories,
        total,
        limit: limit as u32,
        offset: offset as u32,
        has_more,
    };

    let took = start.elapsed().as_micros() as u64;

    Ok(Json(ApiResponse {
        data: response,
        took_us: Some(took),
    }))
    }.await;
    if state.storage.effects_stamp().await? != stamp {
        return Err(crate::storage::StorageError::RecoveryRequired(
            "effects changed during read".into(),
        )
        .into());
    }
    result
}

/// Helper: parse a UUID string from a path segment.
fn parse_memory_id(s: &str) -> Result<MemoryId, AppError> {
    Uuid::parse_str(s)
        .map(MemoryId::from)
        .map_err(|_| AppError::BadRequest {
            message: format!("invalid UUID: '{s}'"),
            field: Some("id".into()),
        })
}

// ═══════════════════════════════════════════════════════════════════════
// Search Handlers
// ═══════════════════════════════════════════════════════════════════════

/// POST /search -- multi-modal search.
///
/// Steps:
/// 1. Validate: at least one of `query` or `embedding` must be provided
///    (unless tag-only metadata search).
/// 2. Resolve namespace.
/// 3. Build `SearchQuery` from request fields.
/// 4. Delegate to `SearchPipeline::search()`.
/// 5. Convert results to `SearchHit`s.
/// 6. Return `SearchResponse` with scores and timing.
pub async fn search_memories(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, AppError> {
    let work = state.search.accept_work()?;
    let result = crate::performance::measure("rest.search", req.record_access, async {
        let stamp = state.storage.effects_stamp().await?;
        let observed =
            search_memories_admitted(State(state.clone()), Json(req), stamp, work.as_ref()).await;
        observed?.finish(check_read_admission(&state, stamp).await)
    })
    .await;
    state.search.finish_work(work);
    result
}

async fn search_memories_admitted(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
    admitted_stamp: u64,
    parent: Option<&crate::storage::work::WorkTicket>,
) -> Result<Observed<Json<SearchResponse>>, AppError> {
    let start = Instant::now();

    // Validate: need at least one search signal
    if req.query.is_none() && req.embedding.is_none() && req.tags.is_empty() {
        return Err(AppError::BadRequest {
            message: "at least one of 'query', 'embedding', or 'tags' must be provided".into(),
            field: None,
        });
    }

    // Validate limit
    let limit = req.limit.min(100) as usize;

    // Resolve namespace
    let ns = state
        .namespaces
        .resolve(&req.namespace)
        .ok_or_else(|| AppError::NotFound {
            resource: "namespace",
            id: req.namespace.clone(),
        })?;

    // Validate embedding dimensions if provided
    if let Some(ref emb) = req.embedding {
        if emb.len() != ns.embedding_dim as usize {
            return Err(AppError::UnprocessableEntity {
                message: format!(
                    "embedding has {} dimensions, namespace '{}' requires {}",
                    emb.len(),
                    ns.name,
                    ns.embedding_dim
                ),
                field: Some("embedding".into()),
            });
        }
    }

    // Merge entities/topics/emotions into tag filters (matching create_memory behaviour)
    let mut include_tags = req.tags.clone();
    for topic in &req.topics {
        let tag = format!("topic/{}", topic.to_lowercase());
        if !include_tags.contains(&tag) {
            include_tags.push(tag);
        }
    }
    for emotion in &req.emotions {
        let tag = format!("emotion/{}", emotion.to_lowercase());
        if !include_tags.contains(&tag) {
            include_tags.push(tag);
        }
    }

    let query_input = match (req.query, req.embedding) {
        (Some(text), Some(embedding)) => QueryInput::TextAndVector { text, embedding },
        (Some(text), None) => QueryInput::Text(text),
        (None, Some(embedding)) => QueryInput::Vector(embedding),
        (None, None) => QueryInput::Text(String::new()),
    };

    let search_query = SearchQuery {
        record_access: req.record_access,
        query: query_input,
        namespace_id: ns.id,
        namespace_name: ns.name.clone(),
        k: limit,
        include_tags,
        exclude_tags: vec![],
        decay_phases: None,
        min_score: req.min_strength,
        graph_depth: req.depth as usize,
        apply_rif: true,
        time_range_start: req.time_range_start,
        time_range_end: req.time_range_end,
        entities: req.entities.clone(),
    };

    let coherent = state
        .search
        .prepare_coherent_search_with_work(search_query.clone(), req.include_embeddings, parent)
        .await?;
    let hydration = crate::performance::Timer::new("rest.hydration");
    let (hits, exposures, evaluated_at) =
        if let Some(prepared) = coherent {
            // This serializer returns ranked hits only; reject unexpected
            // supplemental rows before exposing any unreturned neighbor.
            let exposures = prepared.exposures(false)?;
            let hydration = crate::performance::Timer::new("rest.hydration");
            let payload = prepared
                .payload
                .ok_or_else(|| AppError::ServiceUnavailable {
                    message: "coherent payload capture missing".into(),
                })?;
            if payload.namespace.id != ns.id
                || payload.rows.len() != prepared.response.response.results.len()
            {
                return Err(AppError::ServiceUnavailable {
                    message: "coherent page scope changed".into(),
                });
            }
            let mut hits = Vec::with_capacity(payload.rows.len());
            for (selected, row) in prepared
                .response
                .response
                .results
                .into_iter()
                .zip(payload.rows)
            {
                if row.record.id != *selected.memory_id.as_bytes() {
                    return Err(AppError::ServiceUnavailable {
                        message: "coherent page identity mismatch".into(),
                    });
                }
                let mut memory = MemoryResponse::from_cached(
                    &crate::model::CachedRecord::from(&row.record),
                    payload.namespace.name.clone(),
                );
                if row.record.schema.as_ref().is_some_and(|s| {
                    s.lifecycle.lifecycle_version
                        == crate::model::schema::LifecycleVersion::ExplicitFeedbackV2
                }) {
                    memory.strength = selected.retrievability;
                    memory.decay_strength = selected.effective_r;
                }
                memory.full_text = row.full_text;
                memory.embedding = row.embedding;
                hits.push(SearchHit {
                    memory,
                    score: selected.score.unwrap_or(0.0),
                });
            }
            hydration.complete();
            (hits, exposures, Some(prepared.evaluated_at))
        } else {
            let results = state
                .search
                .prepare_search_with_work(search_query, parent)
                .await?;

            let hydration = crate::performance::Timer::new("rest.hydration");
            // Batch-load embeddings with a single lock acquisition if requested.
            let embeddings = if req.include_embeddings {
                let ids: Vec<MemoryId> = results.iter().map(|r| r.memory.id).collect();
                state.search.get_embeddings_batch(&ids)
            } else {
                std::collections::HashMap::new()
            };

            // Convert to API response, loading full_text from storage
            let mut hits: Vec<SearchHit> = Vec::with_capacity(results.len());
            let mut exposures = Vec::new();
            for r in results {
                exposures.push((r.memory.id, r.access_kind));
                let canonical = state.storage.get_record(r.memory.id).await.ok_or_else(|| {
                    AppError::NotFound {
                        resource: "memory",
                        id: r.memory.id.to_string(),
                    }
                })?;
                if canonical.namespace_id != ns.id.get()
                    || matches!(canonical.phase, DecayPhase::Ghost | DecayPhase::Tombstone)
                {
                    return Err(AppError::ServiceUnavailable {
                        message: "search result changed during hydration".into(),
                    });
                }
                let mut mem = MemoryResponse::from_cached(
                    &crate::model::CachedRecord::from(&canonical),
                    ns.name.clone(),
                );
                if canonical.schema.as_ref().is_some_and(|s| {
                    s.lifecycle.lifecycle_version
                        == crate::model::schema::LifecycleVersion::ExplicitFeedbackV2
                }) {
                    mem.strength = r.memory.strength;
                    mem.decay_strength = r.memory.decay_strength;
                }
                mem.full_text = state.storage.get_full_text(r.memory.id).await?;
                if req.include_embeddings {
                    mem.embedding = embeddings.get(&r.memory.id).cloned();
                }
                hits.push(SearchHit {
                    memory: mem,
                    score: r.score,
                });
            }

            hydration.complete();
            (hits, exposures, None)
        };

    check_read_admission(&state, admitted_stamp).await?;
    let receipts = if req.record_access {
        let scope = crate::storage::control::ExposureScope {
            namespace: ns.name.clone(),
            source: None,
        };
        match evaluated_at {
            Some(at) => {
                state
                    .search
                    .record_coherent_exposures_with_work(&exposures, scope, at, parent)
                    .await?
            }
            None => {
                state
                    .search
                    .record_exposures_with_work(&exposures, scope, parent)
                    .await?
            }
        }
    } else {
        Vec::new()
    };
    let total = hits.len() as u64;
    let took = start.elapsed().as_micros() as u64;

    Ok(Observed {
        receipts,
        value: Json(SearchResponse {
            hits,
            total,
            took_us: Some(took),
        }),
    })
}

/// POST /similar/:id -- find memories similar to an existing memory.
///
/// Steps:
/// 1. Parse source memory UUID.
/// 2. Load source memory's embedding from the vector index.
/// 3. Execute vector search using that embedding as the query.
/// 4. Exclude the source memory from results.
/// 5. Return scored results.
pub async fn find_similar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<FindSimilarRequest>,
) -> Result<Json<SearchResponse>, AppError> {
    let work = state.search.accept_work()?;
    let result = async {
        let stamp = state.storage.effects_stamp().await?;
        let observed = find_similar_admitted(
            State(state.clone()),
            Path(id),
            Json(req),
            stamp,
            work.as_ref(),
        )
        .await;
        observed?.finish(check_read_admission(&state, stamp).await)
    }
    .await;
    state.search.finish_work(work);
    result
}

async fn find_similar_admitted(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<FindSimilarRequest>,
    admitted_stamp: u64,
    parent: Option<&crate::storage::work::WorkTicket>,
) -> Result<Observed<Json<SearchResponse>>, AppError> {
    let start = Instant::now();

    let memory_id = parse_memory_id(&id)?;

    if let Some(prepared) = state
        .search
        .prepare_coherent_similar_with_work(
            memory_id,
            req.namespace.as_deref(),
            req.limit.min(100) as usize,
            req.min_score,
            req.record_access,
            req.include_embeddings,
            parent,
        )
        .await?
    {
        let mut hits = Vec::with_capacity(prepared.results.len());
        for (selected, row) in prepared.results.into_iter().zip(prepared.payload.rows) {
            let mut memory = MemoryResponse::from_cached(
                &crate::model::CachedRecord::from(&row.record),
                prepared.payload.namespace.name.clone(),
            );
            if row.record.schema.as_ref().is_some_and(|s| {
                s.lifecycle.lifecycle_version
                    == crate::model::schema::LifecycleVersion::ExplicitFeedbackV2
            }) {
                memory.strength = selected.retrievability;
                memory.decay_strength = selected.effective_r;
            }
            memory.full_text = row.full_text;
            memory.embedding = row.embedding;
            hits.push(SearchHit {
                memory,
                score: selected.score.unwrap_or(0.0),
            });
        }
        check_read_admission(&state, admitted_stamp).await?;
        let accesses: Vec<_> = hits
            .iter()
            .map(|h| (h.memory.id, AccessKind::AssociativeRetrieval))
            .collect();
        let receipts = if prepared.requested_access {
            state
                .search
                .record_coherent_exposures_with_work(
                    &accesses,
                    crate::storage::control::ExposureScope {
                        namespace: prepared.payload.namespace.name,
                        source: Some(memory_id),
                    },
                    prepared.evaluated_at,
                    parent,
                )
                .await?
        } else {
            Vec::new()
        };
        return Ok(Observed {
            receipts,
            value: Json(SearchResponse {
                total: hits.len() as u64,
                hits,
                took_us: Some(start.elapsed().as_micros() as u64),
            }),
        });
    }

    if let Some(namespace) = req.namespace.as_deref() {
        state
            .storage
            .assert_memory_namespace(memory_id, namespace)
            .await?;
    }

    // The shared path resolves canonical scope, removes the source and applies
    // similarity minScore before exposure. /search minStrength is independent.
    let results = state
        .search
        .prepare_similar_with_work(
            memory_id,
            req.limit.min(100) as usize,
            req.min_score,
            req.record_access,
            parent,
        )
        .await?;
    let embeddings = if req.include_embeddings {
        let ids: Vec<_> = results.iter().map(|r| r.memory_id).collect();
        state.search.get_embeddings_batch(&ids)
    } else {
        std::collections::HashMap::new()
    };
    let mut hits = Vec::with_capacity(results.len());
    for r in results {
        let record =
            state
                .storage
                .get_record(r.memory_id)
                .await
                .ok_or_else(|| AppError::NotFound {
                    resource: "memory",
                    id: r.memory_id.to_string(),
                })?;
        if matches!(
            record.phase,
            crate::model::DecayPhase::Ghost | crate::model::DecayPhase::Tombstone
        ) {
            return Err(AppError::ServiceUnavailable {
                message: "similarity result changed during hydration; retry".into(),
            });
        }
        // Hydrate exact canonical metadata. Explicit-policy strengths are the
        // prepared operation's projection, not persisted compatibility values.
        let mut memory =
            MemoryResponse::from_cached(&crate::model::CachedRecord::from(&record), r.namespace);
        if record.schema.as_ref().is_some_and(|schema| {
            schema.lifecycle.lifecycle_version
                == crate::model::schema::LifecycleVersion::ExplicitFeedbackV2
        }) {
            memory.strength = r.retrievability;
            memory.decay_strength = r.effective_r;
        }
        memory.full_text = state.storage.get_full_text(r.memory_id).await?;
        memory.embedding = embeddings.get(&r.memory_id).cloned();
        hits.push(SearchHit {
            memory,
            score: r.score.unwrap_or(0.0),
        });
    }

    // Reject read-admission drift and hydration errors before any observation.
    if state.storage.effects_stamp().await? != admitted_stamp {
        return Err(AppError::ServiceUnavailable {
            message: "graph effects changed during query; retry".into(),
        });
    }
    let receipts = if req.record_access {
        let ids: Vec<_> = hits.iter().map(|hit| hit.memory.id).collect();
        state
            .search
            .record_similar_with_work(
                &ids,
                crate::storage::control::ExposureScope {
                    namespace: hits
                        .first()
                        .map(|h| h.memory.namespace.clone())
                        .unwrap_or_default(),
                    source: Some(memory_id),
                },
                parent,
            )
            .await?
    } else {
        Vec::new()
    };

    let total = hits.len() as u64;
    let took = start.elapsed().as_micros() as u64;

    Ok(Observed {
        receipts,
        value: Json(SearchResponse {
            hits,
            total,
            took_us: Some(took),
        }),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Namespace Handlers
// ═══════════════════════════════════════════════════════════════════════

/// GET /namespaces -- list all namespaces.
async fn list_namespaces_v1(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<NamespaceListItem>>>, AppError> {
    let stamp = state.storage.effects_stamp().await?;
    let result = async {
        let start = Instant::now();

        let namespaces = state.namespaces.list_all().await;

        let items: Vec<NamespaceListItem> = namespaces
            .into_iter()
            .map(|ns| NamespaceListItem {
                id: ns.id,
                name: ns.name,
                embedding_dim: ns.embedding_dim,
                memory_count: ns.memory_count,
                created_at: ns.created_at,
            })
            .collect();

        let took = start.elapsed().as_micros() as u64;

        Ok(Json(ApiResponse {
            data: items,
            took_us: Some(took),
        }))
    }
    .await;
    if state.storage.effects_stamp().await? != stamp {
        return Err(crate::storage::StorageError::RecoveryRequired(
            "effects changed during read".into(),
        )
        .into());
    }
    result
}

/// POST /namespaces -- create a new namespace.
///
/// Steps:
/// 1. Validate name format (1-64 UTF-8 bytes, ASCII alphanumeric +
///    hyphens + underscores).
/// 2. Check for duplicate name.
/// 3. Register namespace with fixed embedding dimensionality.
/// 4. Return 201 with namespace details.
pub async fn create_namespace(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use crate::serialization::contract::ContractVersion;
    use axum::extract::FromRequest;
    use axum::response::IntoResponse;
    let version = match super::middleware::request_contract_version(request.headers()) {
        Ok(version) => version,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(error) = version.require_supported() {
        return AppError::BadRequest {
            message: error.to_string(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    if version == ContractVersion::V1 {
        let Json(body) = match Json::<serde_json::Value>::from_request(request, &state).await {
            Ok(body) => body,
            Err(error) => return error.into_response(),
        };
        return create_namespace_v1(State(state), Json(body))
            .await
            .into_response();
    }
    let header_key = match catalog_header_key(request.headers()) {
        Ok(key) => key,
        Err(error) => return catalog_response(Err(error.into())),
    };
    let Json(body) = match Json::<serde_json::Value>::from_request(request, &state).await {
        Ok(body) => body,
        Err(error) => return error.into_response(),
    };
    create_namespace_v2(state.namespaces.as_ref(), &body, header_key.as_deref()).await
}

fn catalog_header_key(
    headers: &axum::http::HeaderMap,
) -> Result<Option<String>, crate::serialization::contract::InputError> {
    let mut keys = headers
        .get_all(crate::serialization::contract::KEY_HEADER)
        .iter();
    let first = keys.next();
    let invalid = || crate::serialization::contract::InputError {
        code: "INVALID_COMBINATION",
        field: "/idempotencyKey".into(),
        message: "one ASCII Idempotency-Key header is required when supplied",
    };
    if keys.next().is_some() {
        return Err(invalid());
    }
    first
        .map(|key| key.to_str().map(str::to_owned).map_err(|_| invalid()))
        .transpose()
}
fn catalog_response(
    result: Result<
        crate::storage::control::WriteReceipt,
        crate::native_catalog_adapter::CatalogError,
    >,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (status, envelope) = crate::native_catalog_adapter::response(result);
    (StatusCode::from_u16(status).unwrap(), Json(envelope)).into_response()
}
/// Real inner consumer; the outer route still rejects public contract2.
pub(crate) async fn create_namespace_v2(
    namespaces: &dyn super::state::NamespaceRegistry,
    body: &serde_json::Value,
    header_key: Option<&str>,
) -> axum::response::Response {
    let result =
        match crate::serialization::catalog::CreateNamespaceRequest::from_http(body, header_key) {
            Ok(request) => namespaces.create_native_namespace(request).await,
            Err(error) => Err(error.into()),
        };
    catalog_response(result)
}

async fn create_namespace_v1(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let start = Instant::now();

    let req = match crate::mcp::args::parse_create_namespace(&req) {
        Ok(req) => req,
        Err(error) => return argument_error(error).into_response(),
    };

    // Create namespace (registry checks for duplicates)
    let result = state
        .namespaces
        .create_bounded(
            req,
            crate::serialization::catalog_api1::CatalogReplyAdmission::http(),
        )
        .await;
    let ns = match result {
        Ok(ns) => ns,
        Err(error) => match error.downcast::<crate::native_catalog_adapter::CatalogError>() {
            Ok(error) => {
                let (status, value) = crate::serialization::catalog_api1::failure(&error);
                return (
                    StatusCode::from_u16(status).expect("catalog HTTP status"),
                    Json(value),
                )
                    .into_response();
            }
            Err(error) => return namespace_create_error(error).into_response(),
        },
    };

    let response = NamespaceResponse {
        id: ns.id.get(),
        name: ns.name,
        embedding_dim: ns.embedding_dim,
        initial_stability: ns.initial_stability,
        default_difficulty: ns.default_difficulty,
        permastore_threshold: ns.permastore_threshold,
        desired_retention: ns.desired_retention,
        created_at: ns.created_at,
        memory_count: 0,
    };

    let took = start.elapsed().as_micros() as u64;

    (
        StatusCode::CREATED,
        Json(ApiResponse {
            data: response,
            took_us: Some(took),
        }),
    )
        .into_response()
}

fn namespace_create_error(error: Box<dyn std::error::Error + Send + Sync>) -> AppError {
    use crate::decay::lifecycle::LifecycleError;
    use crate::derived_completion::catalog::CatalogCreateError;
    use crate::embedding::runtime::NamespaceProfileError;
    use crate::storage::StorageError;
    let error: Box<dyn std::error::Error + Send + Sync> = match error
        .downcast::<CatalogCreateError>()
    {
        Ok(error) => match *error {
            CatalogCreateError::Storage(error) => Box::new(error),
            CatalogCreateError::WorkerUnknown => {
                return AppError::MutationUnknown {
                    operation_id: None,
                    message: "namespace worker outcome unavailable; reconcile before retry".into(),
                };
            }
        },
        Err(error) => error,
    };
    if let Some(error) = error.downcast_ref::<NamespaceProfileError>() {
        return match error {
            NamespaceProfileError::InvalidDimension => AppError::BadRequest {
                message: error.to_string(),
                field: Some("embeddingDim".into()),
            },
            NamespaceProfileError::InactiveDimension { .. } => AppError::UnprocessableEntity {
                message: error.to_string(),
                field: Some("embeddingDim".into()),
            },
            NamespaceProfileError::Unavailable(_) => AppError::ServiceUnavailable {
                message: error.to_string(),
            },
        };
    }
    if matches!(
        error.downcast_ref::<LifecycleError>(),
        Some(LifecycleError::InvalidInput(_))
    ) {
        return AppError::BadRequest {
            message: error.to_string(),
            field: None,
        };
    }
    match error.downcast::<StorageError>() {
        Ok(error) => match *error {
            StorageError::InvalidMutation(message)
            | StorageError::InvalidNamespaceName {
                reason: message, ..
            } => AppError::BadRequest {
                message,
                field: None,
            },
            StorageError::CommitOutcomeUnknown { operation_id, .. } => AppError::MutationUnknown {
                operation_id: Some(operation_id),
                message: "namespace commit acknowledgement unavailable; reconcile before retry"
                    .into(),
            },
            error => error.into(),
        },
        Err(source) => AppError::Internal { source },
    }
}

/// GET /namespaces/:id/stats -- namespace statistics.
///
/// Accepts either namespace name (string) or integer ID.
pub async fn namespace_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<NamespaceStatsResponse>>, AppError> {
    let stamp = state.storage.effects_stamp().await?;
    let result = async {
        let start = Instant::now();

        // Try parsing as integer ID first, then fall back to name lookup
        let ns = if let Ok(numeric_id) = id.parse::<u32>() {
            state.namespaces.get_by_id(numeric_id)
        } else {
            state.namespaces.resolve(&id)
        }
        .ok_or_else(|| AppError::NotFound {
            resource: "namespace",
            id: id.clone(),
        })?;

        let stats = state.storage.namespace_stats(ns.id).await?;

        let response = NamespaceStatsResponse {
            name: ns.name,
            id: ns.id.get(),
            memory_count: stats.memory_count,
            tombstone_count: stats.tombstone_count,
            phase_counts: PhaseCounts {
                full: stats.phase_1_count,
                summary: stats.phase_2_count,
                ghost: stats.phase_3_count,
            },
            permastore_count: stats.permastore_count,
            avg_strength: stats.avg_strength,
            edge_count: stats.edge_count,
            embedding_dim: ns.embedding_dim,
            vector_bytes: stats.memory_count * ns.embedding_dim as u64 * 4,
        };

        let took = start.elapsed().as_micros() as u64;

        Ok(Json(ApiResponse {
            data: response,
            took_us: Some(took),
        }))
    }
    .await;
    if state.storage.effects_stamp().await? != stamp {
        return Err(crate::storage::StorageError::RecoveryRequired(
            "effects changed during read".into(),
        )
        .into());
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// Operational Handlers
// ═══════════════════════════════════════════════════════════════════════

/// GET /health -- health check with subsystem status.
///
/// Returns 200 with status `"healthy"` if all subsystems report up.
/// Returns 200 with status `"degraded"` if non-critical subsystems are
/// down (e.g. embedding provider). Returns 503 if critical subsystems
/// are down (storage, cache).
pub async fn health_check(State(state): State<AppState>) -> Result<Json<HealthResponse>, AppError> {
    let uptime = state.started_at.elapsed().as_secs();

    let storage_health = probe_component("storage", || async { state.storage.ping().await }).await;

    let cache_health = ComponentHealth {
        status: "up".to_string(),
        message: Some(format!(
            "entries: {}, hit_rate: {:.1}%",
            state.cache.entry_count(),
            state.cache.hit_rate() * 100.0
        )),
        latency_us: None,
    };

    let vector_health = ComponentHealth {
        status: "up".to_string(),
        message: Some(format!("indexed: {}", state.search.indexed_count())),
        latency_us: None,
    };

    let embedding_health = probe_component("embedding", || async {
        state.search.embedding_provider_healthy().await
    })
    .await;

    let decay_health = ComponentHealth {
        status: if state.decay.sweep_thread_alive() {
            "up"
        } else {
            "down"
        }
        .to_string(),
        message: state
            .decay
            .last_sweep_time()
            .map(|t| format!("last sweep: {}s ago", t.elapsed().as_secs())),
        latency_us: None,
    };

    let subsystems = SubsystemHealth {
        storage: storage_health,
        cache: cache_health,
        vector_index: vector_health,
        embedding: embedding_health,
        decay: decay_health,
    };

    // Determine overall status
    let status = if subsystems.storage.status == "down" || subsystems.cache.status == "down" {
        HealthStatus::Unhealthy
    } else if subsystems.embedding.status == "down" || subsystems.decay.status == "down" {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    Ok(Json(HealthResponse {
        status,
        uptime_secs: uptime,
        subsystems,
    }))
}

/// Helper: probe a component and measure latency.
async fn probe_component<F, Fut>(name: &str, f: F) -> ComponentHealth
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let probe_start = Instant::now();
    let is_up = f().await;
    let latency = probe_start.elapsed().as_micros() as u64;

    ComponentHealth {
        status: if is_up { "up" } else { "down" }.to_string(),
        message: if !is_up {
            Some(format!("{name} health check failed"))
        } else {
            None
        },
        latency_us: Some(latency),
    }
}

/// GET /health/report -- comprehensive decay health report.
///
/// Computes decay forecast, at-risk memories, storage breakdown,
/// age distribution, and tag statistics. Optionally scoped to a
/// single namespace via `?namespace=<name>`.
///
/// Results are cached for 60 seconds per scope key to avoid repeated
/// `scan_all()` calls that load the entire database into memory.
pub async fn health_report(
    State(state): State<AppState>,
    Query(params): Query<HealthReportQuery>,
) -> Result<Json<ApiResponse<HealthReport>>, AppError> {
    let start = Instant::now();
    let report = compute_health_report(
        state.storage.as_ref(),
        state.namespaces.as_ref(),
        state.decay.decay_rate_multiplier(),
        params.namespace,
    )
    .await?;
    Ok(Json(ApiResponse {
        data: report,
        took_us: Some(start.elapsed().as_micros() as u64),
    }))
}

/// The decay health report for one namespace (or all), shared by
/// `GET /health/report` and the daemon. Cached per scope as described on
/// [`health_report`].
pub async fn compute_health_report(
    storage: &dyn StorageEngine,
    namespaces: &dyn NamespaceRegistry,
    global_decay_multiplier: f64,
    namespace: Option<String>,
) -> Result<HealthReport, AppError> {
    let stamp = storage.effects_stamp().await?;
    let result = async {
        // Resolve optional namespace filter (validate before cache check)
        let namespace_filter: Option<NamespaceId> = if let Some(ref ns_name) = namespace {
            Some(
                namespaces
                    .resolve(ns_name)
                    .ok_or_else(|| AppError::NotFound {
                        resource: "namespace",
                        id: ns_name.clone(),
                    })?
                    .id,
            )
        } else {
            None
        };

        let scope = namespace.unwrap_or_else(|| "all".to_string());

        // --- Check cache ---
        {
            let cache = health_report_cache().read().await;
            if let Some((cached_stamp, generated_at, cached_report)) = cache.get(&scope) {
                if *cached_stamp == stamp
                    && generated_at.elapsed().as_secs() < HEALTH_REPORT_CACHE_TTL_SECS
                {
                    return Ok(cached_report.clone());
                }
            }
        }

        // --- Cache miss: compute the report ---
        let all_records = storage.scan_all().await?;

        // Filter records by namespace if applicable
        let filtered: Vec<_> = all_records
            .iter()
            .filter(|(_, r)| {
                if let Some(ns_id) = namespace_filter {
                    r.namespace_id == ns_id.get()
                } else {
                    true
                }
            })
            .collect();

        let overview = health_report_compute::compute_overview(&filtered);
        let decay_forecast = health_report_compute::compute_decay_forecast(
            &filtered,
            namespaces,
            global_decay_multiplier,
        );
        let at_risk =
            health_report_compute::compute_at_risk(&filtered, namespaces, global_decay_multiplier);
        let age_distribution = health_report_compute::compute_age_distribution(&filtered);
        let storage_breakdown =
            health_report_compute::compute_storage_breakdown(storage, namespaces, namespace_filter)
                .await;
        let metadata = health_report_compute::compute_metadata_stats(storage, namespace_filter)
            .await
            .map_err(|e| AppError::Internal { source: e })?;

        let report = HealthReport {
            scope: scope.clone(),
            overview,
            decay_forecast,
            at_risk,
            age_distribution,
            storage: storage_breakdown,
            metadata,
        };

        // --- Store in cache ---
        {
            let mut cache = health_report_cache().write().await;
            cache.insert(scope, (stamp, Instant::now(), report.clone()));
        }

        Ok(report)
    }
    .await;
    if storage.effects_stamp().await? != stamp {
        return Err(crate::storage::StorageError::RecoveryRequired(
            "effects changed during health report".into(),
        )
        .into());
    }
    result
}

/// POST /memories/batch -- batch create memories (Issue 16).
///
/// Creates multiple memories in sequence. Each memory goes through the
/// same validation and creation pipeline as the single-create endpoint.
pub async fn batch_store(
    State(state): State<AppState>,
    Json(req): Json<BatchStoreRequest>,
) -> Result<(StatusCode, Json<ApiResponse<BatchStoreResponse>>), AppError> {
    state.storage.effects_stamp().await?;

    let start = Instant::now();

    if req.memories.is_empty() {
        return Err(AppError::BadRequest {
            message: "memories array must not be empty".into(),
            field: Some("memories".into()),
        });
    }

    if req.memories.len() > MAX_BATCH_MEMORIES {
        return Err(AppError::BadRequest {
            message: format!("batch size exceeds the maximum of {MAX_BATCH_MEMORIES}"),
            field: Some("memories".into()),
        });
    }

    let mut created = Vec::with_capacity(req.memories.len());
    let mut failed: Vec<BatchStoreFailure> = Vec::new();

    for (index, mem_req) in req.memories.into_iter().enumerate() {
        // A bad item skips rather than failing the whole batch, but it no
        // longer skips silently: every `continue` below records why. The
        // limits themselves are the shared ones, which keeps an oversized
        // summary from reaching the u16-prefixed on-disk record encoder.
        if let Err(e) = validate_legacy_memory_input(MemoryInputRef {
            summary: &mem_req.summary,
            full_text: mem_req.full_text.as_deref(),
            tags: &mem_req.tags,
            entities: &mem_req.entities,
            topics: &mem_req.topics,
            emotions: &mem_req.emotions,
        }) {
            failed.push(BatchStoreFailure {
                index,
                error: e.to_string(),
                field: e.field().map(str::to_string),
            });
            continue;
        }

        let ns = match state.namespaces.resolve(&mem_req.namespace) {
            Some(ns) => ns,
            None => {
                failed.push(BatchStoreFailure {
                    index,
                    error: format!("namespace '{}' not found", mem_req.namespace),
                    field: Some("namespace".into()),
                });
                continue;
            }
        };

        // Validate caller-specified links before creating anything, so a
        // rejected link leaves no orphaned memory behind.
        let mut link_error: Option<crate::graph::LinkError> = None;
        for (field, target) in [
            ("parentId", mem_req.parent_id),
            ("supersedes", mem_req.supersedes),
        ] {
            if let Some(target) = target {
                if let Err(e) = state
                    .graph
                    .check_link_target(field, None, target, ns.id)
                    .await
                {
                    link_error = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = link_error {
            failed.push(BatchStoreFailure {
                index,
                error: e.to_string(),
                field: Some(e.field().into()),
            });
            continue;
        }

        // Merge entities/topics/emotions into tags
        let mut merged_tags = mem_req.tags.clone();
        for entity in &mem_req.entities {
            let tag = format!("{ENTITY_TAG_PREFIX}{}", entity.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
        }
        for topic in &mem_req.topics {
            let tag = format!("{TOPIC_TAG_PREFIX}{}", topic.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
        }
        for emotion in &mem_req.emotions {
            let tag = format!("{EMOTION_TAG_PREFIX}{}", emotion.to_lowercase());
            if !merged_tags.contains(&tag) {
                merged_tags.push(tag);
            }
        }

        // Embedding
        let raw_embedding = match mem_req.embedding {
            Some(ref vec) => {
                if vec.len() != ns.embedding_dim as usize {
                    failed.push(BatchStoreFailure {
                        index,
                        error: format!(
                            "embedding has {} dimensions, namespace '{}' requires {}",
                            vec.len(),
                            ns.name,
                            ns.embedding_dim
                        ),
                        field: Some("embedding".into()),
                    });
                    continue;
                }
                vec.clone().into()
            }
            None => {
                let mut embed_text = match &mem_req.full_text {
                    Some(ft) => format!("{}\n\n{}", mem_req.summary, ft),
                    None => mem_req.summary.clone(),
                };
                if !merged_tags.is_empty() {
                    embed_text = format!("{} {}", embed_text, merged_tags.join(" "));
                }
                match state.search.embed_text(&embed_text, &ns).await {
                    Ok(emb) => emb,
                    Err(e) => {
                        failed.push(BatchStoreFailure {
                            index,
                            error: format!("failed to generate embedding: {e}"),
                            field: None,
                        });
                        continue;
                    }
                }
            }
        };

        if let Err(error) =
            crate::embedding::runtime::check_namespace(&ns, raw_embedding.profile.as_ref())
        {
            failed.push(BatchStoreFailure {
                index,
                error: error.to_string(),
                field: None,
            });
            continue;
        }
        let mut embedding = raw_embedding.values.clone();
        if let Err(error) = crate::storage::insertion::normalize_embedding(
            &mut embedding,
            ns.embedding_dim,
            mem_req.embedding.is_some(),
        ) {
            failed.push(BatchStoreFailure {
                index,
                error: error.to_string(),
                field: mem_req.embedding.as_ref().map(|_| "embedding".into()),
            });
            continue;
        }

        // Near-duplicate advisory, before this item is indexed. Items are
        // created in order and each is indexed before the next is checked,
        // so an intra-batch duplicate is reported too — same property the
        // MCP batch tool has.
        let near_duplicates =
            near_duplicate_report(&state, &embedding, ns.id, mem_req.check_duplicates).await?;

        let inserted = match state
            .storage
            .insert_memory(
                ns.id,
                crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: mem_req.summary.clone(),
                    full_text: mem_req.full_text.clone(),
                    tags: crate::model::parse_tags_lossy("batch_store.tags", &merged_tags),
                    embedding: mem_req.embedding.clone(),
                    initial_stability: mem_req.initial_stability,
                    created_at: mem_req.created_at,
                    parent_id: mem_req.parent_id,
                    supersedes: mem_req.supersedes,
                },
                raw_embedding,
            )
            .await
        {
            Ok(value) => value,
            Err(error @ crate::storage::StorageError::CommittedMutation { .. }) => {
                return Err(error.into());
            }
            Err(error @ crate::storage::StorageError::CommitOutcomeUnknown { .. }) => {
                return Err(error.into());
            }
            Err(error) => {
                failed.push(BatchStoreFailure {
                    index,
                    error: error.to_string(),
                    field: None,
                });
                continue;
            }
        };
        let receipt = inserted.receipt;
        let memory = inserted.record.ok_or_else(|| {
            AppError::from(crate::storage::StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(crate::storage::StorageError::RecoveryRequired(
                    "inserted record unavailable".into(),
                )),
            })
        })?;
        let supersedes_outcome = mem_req.supersedes.map(|id| {
            crate::model::SupersedesOutcome::new(id, crate::model::SupersedesStatus::Applied)
        });

        // Post-creation links
        let created_at = memory.created_at;
        state
            .graph
            .perform_post_creation_links(
                memory.id,
                ns.id,
                &embedding,
                &merged_tags,
                &mem_req.entities,
                created_at,
            )
            .await;

        state.storage.complete_insertion(&receipt).await?;
        state.cache.insert(&memory).await;
        created.push(BatchStoreResult {
            id: memory.id,
            namespace: ns.name.clone(),
            supersedes: supersedes_outcome,
            near_duplicates,
        });
    }

    let total = created.len() as u64;
    let took = start.elapsed().as_micros() as u64;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            data: BatchStoreResponse {
                created,
                total,
                failed,
            },
            took_us: Some(took),
        }),
    ))
}

/// Preserve the v1 typed extractor/shape; v2 uses the same native runner as MCP.
pub async fn batch_store_versioned(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use crate::serialization::contract::{ContractVersion, KEY_HEADER};
    use axum::{extract::FromRequest, response::IntoResponse};
    let version = match super::middleware::request_contract_version(request.headers()) {
        Ok(v) => v,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(e) = version.require_supported() {
        return AppError::BadRequest {
            message: e.to_string(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    if version == ContractVersion::V1 {
        return match Json::<super::store_api1::BatchRequest>::from_request(request, &state).await {
            Ok(Json(body)) => super::store_api1::batch(state, body).await,
            Err(e) => e.into_response(),
        };
    }
    let mut keys = request.headers().get_all(KEY_HEADER).iter();
    let key = keys
        .next()
        .map(|v| v.to_str().map(str::to_owned))
        .transpose();
    if keys.next().is_some() || key.is_err() {
        return batch_failure_response(
            crate::serialization::contract::InputError {
                code: "INVALID_TYPE",
                field: "/idempotencyKey".into(),
                message: "one ASCII idempotency key header is required",
            }
            .into(),
        );
    }
    let body = match Json::<serde_json::Value>::from_request(request, &state).await {
        Ok(Json(body)) => body,
        Err(_) => {
            return batch_failure_response(
                crate::serialization::contract::InputError {
                    code: "INVALID_TYPE",
                    field: String::new(),
                    message: "body must be a JSON object",
                }
                .into(),
            );
        }
    };
    let Some(producer) = state.native_batches.as_deref() else {
        return batch_failure_response(
            crate::serialization::native_batch::BatchFailure::unavailable(),
        );
    };
    batch_store_v2(
        producer,
        &state.default_namespace,
        &body,
        key.unwrap().as_deref(),
    )
    .await
}
fn batch_failure_response(
    error: crate::serialization::native_batch::BatchFailure,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (status, envelope) = crate::serialization::native_batch::response(Err(error));
    (
        StatusCode::from_u16(status).expect("fixed batch status"),
        Json(envelope),
    )
        .into_response()
}
pub(crate) async fn batch_store_v2(
    producer: &dyn crate::mcp::bridge::StorageEngine,
    default_namespace: &str,
    body: &serde_json::Value,
    key: Option<&str>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let result =
        match crate::serialization::native_batch::parse(body, false, key, default_namespace) {
            Ok(input) => producer.mutate_native_batch(input).await,
            Err(e) => Err(e.into()),
        };
    let (status, envelope) = crate::serialization::native_batch::response(result);
    (
        StatusCode::from_u16(status).expect("fixed batch status"),
        Json(envelope),
    )
        .into_response()
}

/// POST /namespaces/:name/duplicates -- scan for duplicate memories (Issue 16).
///
/// Uses vector similarity to find clusters of near-duplicate memories
/// within a namespace.
pub async fn scan_duplicates(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ScanDuplicatesRequest>,
) -> Result<Json<ApiResponse<ScanDuplicatesResponse>>, AppError> {
    let stamp = state.storage.effects_stamp().await?;
    let result = async {
        let start = Instant::now();

        let ns = state
            .namespaces
            .resolve(&name)
            .ok_or_else(|| AppError::NotFound {
                resource: "namespace",
                id: name.clone(),
            })?;

        if !req.threshold.is_finite() || !(0.0..=1.0).contains(&req.threshold) {
            return Err(AppError::BadRequest {
                message: "threshold must be finite and between 0 and 1".into(),
                field: Some("threshold".into()),
            });
        }
        if !(1..=10_000).contains(&req.max_memories) {
            return Err(AppError::BadRequest {
                message: "maxMemories must be between 1 and 10000".into(),
                field: Some("maxMemories".into()),
            });
        }
        let threshold = req.threshold;
        let max_memories = req.max_memories;

        // Get memory IDs in this namespace
        let filter = ListFilter {
            namespace_id: Some(ns.id),
            phase: None,
            tags: vec![],
            time_range_start: None,
            time_range_end: None,
        };
        let records = state.storage.list_memories(&filter).await?;
        let memory_ids: Vec<MemoryId> = records.iter().take(max_memories).map(|r| r.id).collect();

        // Load embeddings
        let embeddings = state.search.get_embeddings_batch(&memory_ids);
        let sampled_count = memory_ids.len();
        let id_embeddings: Vec<(MemoryId, Vec<f32>)> = memory_ids
            .into_iter()
            .filter_map(|mid| embeddings.get(&mid).map(|emb| (mid, emb.clone())))
            .collect();

        let coverage = DuplicateScanCoverage {
            algorithm: "sampledPairs",
            eligible_count: records.len(),
            sampled_count,
            skipped_vector_count: sampled_count - id_embeddings.len(),
            sample_cap: max_memories,
            neighbor_cap: None,
            truncated: sampled_count < records.len(),
            // This legacy scan has no generation-consistent read view.
            complete: false,
        };

        // Union-Find based clustering
        let n = id_embeddings.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }

        fn union(parent: &mut [usize], a: usize, b: usize) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb {
                parent[rb] = ra;
            }
        }

        let mut max_sim: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();

        // Compare each memory's embedding against its nearest neighbors
        for (i, (mid_i, _emb_i)) in id_embeddings.iter().enumerate() {
            // Find similar via the search pipeline's get_embedding + manual cosine
            if let Some(emb) = state.search.get_embedding(*mid_i) {
                // Use pairwise comparison within the loaded set
                for (j, (_mid_j, emb_j)) in id_embeddings.iter().enumerate() {
                    if i >= j {
                        continue;
                    }
                    let score = crate::search::dot_product_simd(&emb, emb_j);
                    if score >= threshold {
                        union(&mut parent, i, j);
                        let root = find(&mut parent, i);
                        let entry = max_sim.entry(root).or_insert(0.0);
                        if score > *entry {
                            *entry = score;
                        }
                    }
                }
            }
        }

        // Group into clusters
        let mut clusters_map: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            clusters_map.entry(root).or_default().push(i);
        }

        let mut clusters: Vec<DuplicateCluster> = Vec::new();
        for (root, members) in &clusters_map {
            if members.len() < 2 {
                continue;
            }
            let entries: Vec<DuplicateEntry> = members
                .iter()
                .map(|&idx| {
                    let mid = id_embeddings[idx].0;
                    let summary = records
                        .iter()
                        .find(|r| r.id == mid)
                        .map(|r| r.summary.clone())
                        .unwrap_or_default();
                    DuplicateEntry {
                        id: mid.to_string(),
                        summary,
                    }
                })
                .collect();
            let max_similarity = max_sim.get(root).copied().unwrap_or(0.0);
            clusters.push(DuplicateCluster {
                memories: entries,
                max_similarity,
            });
        }

        clusters.sort_by(|a, b| {
            b.max_similarity
                .partial_cmp(&a.max_similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = clusters.len() as u64;
        let took = start.elapsed().as_micros() as u64;

        Ok(Json(ApiResponse {
            data: ScanDuplicatesResponse {
                clusters,
                total,
                coverage,
            },
            took_us: Some(took),
        }))
    }
    .await;
    if state.storage.effects_stamp().await? != stamp {
        return Err(crate::storage::StorageError::RecoveryRequired(
            "effects changed during read".into(),
        )
        .into());
    }
    result
}

/// GET /metrics -- Prometheus exposition format.
///
/// Returns `text/plain` with metrics lines. Not JSON.
/// Uses the `MetricsCollector` to gather counters and gauges.
pub async fn metrics(
    State(state): State<AppState>,
) -> Result<
    (
        StatusCode,
        [(axum::http::HeaderName, axum::http::HeaderValue); 1],
        String,
    ),
    AppError,
> {
    let output = state.metrics.render_prometheus().await;

    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        output,
    ))
}

/// POST /decay/sweep -- trigger an immediate decay sweep.
///
/// Uses the configured clock. Caller-supplied time overrides are rejected
/// before accepting work; isolated replay controls its existing owned clock.
pub async fn trigger_decay_sweep(
    State(state): State<AppState>,
    Json(req): Json<TriggerSweepRequest>,
) -> Result<Json<ApiResponse<TriggerSweepResponse>>, AppError> {
    if req.fields.contains_key("asOf") || req.fields.contains_key("as_of") {
        return Err(AppError::BadRequest {
            message: "Public sweeps use the configured clock; remove asOf/as_of".into(),
            field: Some("asOf".into()),
        });
    }
    let result = state.decay.trigger_sweep(None).await.map_err(|error| {
        match error.downcast::<crate::decay::sweep::evidence::SweepFailure>() {
            Ok(failure) => AppError::Sweep(*failure),
            Err(error) => match error.downcast::<crate::storage::StorageError>() {
                Ok(storage) => AppError::from(*storage),
                Err(source) => AppError::Internal { source },
            },
        }
    })?;

    Ok(Json(ApiResponse {
        data: TriggerSweepResponse {
            memories_scanned: result.memories_scanned,
            stale_skipped: result.stale_skipped,
            unchanged_skipped: result.unchanged_skipped,
            exempt_skipped: result.exempt_skipped,
            full_to_summary: result.full_to_summary,
            summary_to_ghost: result.summary_to_ghost,
            deletions: result.deletions,
            saved_by_connection_bonus: result.saved_by_connection_bonus,
            duration_ms: result.duration.as_millis() as u64,
        },
        took_us: Some(result.duration.as_micros() as u64),
    }))
}

#[cfg(test)]
mod lifecycle_error_tests {
    use super::*;
    use crate::api::FsrsEngine as _;
    use crate::model::DecayPhase;
    use crate::test_support::{Fixture, MemorySpec};
    use axum::response::IntoResponse;

    /// Exercise the handler's mapping with real adapter errors, including terminal content.
    #[tokio::test]
    async fn feedback_handler_maps_terminal_missing_and_invalid_inputs_by_type() {
        let f = Fixture::new();
        let adapter = crate::api::adapters::FsrsEngineAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            f.graph.clone(),
            std::sync::Arc::new(crate::config::RecalldConfig::default()),
            false,
        );
        for phase in [
            DecayPhase::Ghost,
            DecayPhase::Tombstone,
            DecayPhase::Summary,
        ] {
            let id = f
                .insert_memory_with(MemorySpec {
                    phase: if phase == DecayPhase::Tombstone {
                        DecayPhase::Full
                    } else {
                        phase
                    },
                    summary: "",
                    ..MemorySpec::new(NamespaceId::new(1))
                })
                .await;
            if phase == DecayPhase::Tombstone {
                f.tombstone(id);
            }
            let error = adapter.reinforce(id, 3, None).await.unwrap_err();
            let response = feedback_error(error, &id.to_string()).into_response();
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["error"], "CONFLICT");
            assert_eq!(f.record(id).unwrap().phase, phase);
            assert_eq!(f.record(id).unwrap().last_accessed_at, 0);
        }
        let missing = MemoryId::new();
        let error = adapter.reinforce(missing, 3, None).await.unwrap_err();
        assert_eq!(
            feedback_error(error, &missing.to_string())
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        let id = f.insert_memory().await;
        let error = adapter.reinforce(id, 0, None).await.unwrap_err();
        assert_eq!(
            feedback_error(error, &id.to_string())
                .into_response()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
pub(crate) mod validation_tests;

async fn check_read_admission(state: &AppState, stamp: u64) -> Result<(), AppError> {
    if state.storage.effects_stamp().await? != stamp {
        return Err(AppError::ServiceUnavailable {
            message: "graph effects changed during query".into(),
        });
    }
    Ok(())
}

/// Route selection for duplicates. The shared middleware and this direct-call
/// boundary both keep v2 closed until the native serving/wire contract is ready.
pub async fn dispatch_duplicates(
    State(state): State<super::state::AppState>,
    Path(name): Path<String>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use crate::serialization::contract::{ContractVersion, Envelope};
    use axum::{extract::FromRequest, response::IntoResponse};
    let version = super::middleware::request_contract_version(request.headers());
    let version = match version {
        Ok(version) => version,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(error) = version.require_supported() {
        return AppError::BadRequest {
            message: error.to_string(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    if version == ContractVersion::V1 {
        // Preserve Axum's legacy JSON extractor rejection and successful shape.
        return match Json::<super::models::ScanDuplicatesRequest>::from_request(request, &state)
            .await
        {
            Ok(Json(body)) => scan_duplicates(State(state), Path(name), Json(body))
                .await
                .into_response(),
            Err(rejection) => rejection.into_response(),
        };
    }
    let body = match Json::<serde_json::Value>::from_request(request, &state).await {
        Ok(Json(body)) => body,
        Err(_) => {
            let error = crate::search::exact_scan::ExactScanError::Input(
                crate::serialization::contract::InputError {
                    code: "INVALID_TYPE",
                    field: String::new(),
                    message: "body must be a JSON object",
                },
            )
            .execution_error();
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(Envelope::<crate::search::exact_scan::ExactScanData>::failure(None, error)),
            )
                .into_response();
        }
    };
    scan_duplicates_v2(state.storage.as_ref(), &name, &body).await
}

/// Actual gated handler logic, shared with native acceptance checks. The public
/// route must still pass the version/serving gate before entering this function.
pub(crate) async fn scan_duplicates_v2(
    storage: &dyn super::state::StorageEngine,
    name: &str,
    body: &serde_json::Value,
) -> axum::response::Response {
    use crate::serialization::contract::Envelope;
    use axum::response::IntoResponse;
    let result = match crate::serialization::scan::ExactScanRequest::from_http(body, name) {
        Ok(request) => storage.scan_duplicate_pairs(request).await,
        Err(error) => Err(crate::search::exact_scan::ExactScanError::Input(error)),
    };
    match result {
        Ok(page) => Json(page.into_envelope()).into_response(),
        Err(error) => {
            use crate::serialization::contract::ErrorCategory;
            let error = error.execution_error();
            let status = match error.category {
                ErrorCategory::InvalidInput => axum::http::StatusCode::BAD_REQUEST,
                ErrorCategory::NotFound => axum::http::StatusCode::NOT_FOUND,
                ErrorCategory::Conflict => axum::http::StatusCode::CONFLICT,
                _ => axum::http::StatusCode::SERVICE_UNAVAILABLE,
            };
            (
                status,
                Json(Envelope::<crate::search::exact_scan::ExactScanData>::failure(None, error)),
            )
                .into_response()
        }
    }
}

/// Prepared inner handler. Public middleware still rejects application2.
pub(crate) async fn search_v2(
    search: &dyn super::state::SearchPipeline,
    default_namespace: &str,
    body: &serde_json::Value,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let reply = match crate::serialization::query::QueryRequest::from_http(body, default_namespace)
    {
        Ok(request) => search.query_contract(request).await,
        Err(error) => crate::serialization::query_response::QueryReply::input(error),
    };
    (
        axum::http::StatusCode::from_u16(reply.http_status).expect("query HTTP status"),
        Json(reply),
    )
        .into_response()
}

pub(crate) async fn similar_v2(
    search: &dyn super::state::SearchPipeline,
    id: crate::model::MemoryId,
    body: &serde_json::Value,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let reply = match crate::serialization::query::SimilarRequest::from_http(body, id) {
        Ok(request) => search.similar_contract(request).await,
        Err(error) => crate::serialization::query_response::QueryReply::input(error),
    };
    (
        axum::http::StatusCode::from_u16(reply.http_status).expect("similarity HTTP status"),
        Json(reply),
    )
        .into_response()
}

/// Resolved v1 discovery only. Store/provider identity certifies no ReadView,
/// native writer readiness or historical vector provenance.
pub async fn capabilities(State(state): State<super::state::AppState>) -> axum::response::Response {
    use crate::serialization::session::{BackendContext, document};
    use axum::response::IntoResponse;
    let unavailable = || AppError::ServiceUnavailable {
        message: "backend context unavailable".into(),
    };
    let result = async {
        let store_id = state
            .storage
            .store_identity()
            .await
            .map_err(|_| unavailable())?;
        let active = state.search.active_embedding().map_err(|_| unavailable())?;
        let backend = BackendContext::new(store_id, active).map_err(|_| unavailable())?;
        let requested = crate::model::constants::DEFAULT_NAMESPACE_NAME;
        let namespace = state.namespaces.resolve(requested);
        Ok::<_, AppError>(Json(document(
            requested,
            "legacyHttpDefault",
            namespace.map(|ns| (ns.name, ns.id.get(), ns.embedding_dim)),
            Some(backend),
        )))
    }
    .await;
    let mut response = result.into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}

pub async fn list_namespaces(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    catalog_route(
        state,
        request,
        crate::serialization::catalog_page::CatalogKind::Namespaces,
    )
    .await
}
pub async fn list_tags(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    catalog_route(
        state,
        request,
        crate::serialization::catalog_page::CatalogKind::Tags,
    )
    .await
}
async fn catalog_route(
    state: AppState,
    request: axum::extract::Request,
    kind: crate::serialization::catalog_page::CatalogKind,
) -> axum::response::Response {
    use crate::serialization::{catalog_page::CatalogKind, contract::ContractVersion};
    use axum::{extract::FromRequestParts, response::IntoResponse};
    let version = match super::middleware::request_contract_version(request.headers()) {
        Ok(v) => v,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(error) = version.require_supported() {
        return AppError::BadRequest {
            message: error.to_string(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    if version == ContractVersion::V1 {
        return if kind == CatalogKind::Namespaces {
            list_namespaces_v1(State(state)).await.into_response()
        } else {
            AppError::BadRequest {
                message: "tag continuation requires contractVersion 2".into(),
                field: Some("contractVersion".into()),
            }
            .into_response()
        };
    }
    let (mut parts, _) = request.into_parts();
    let Query(params) =
        match Query::<Vec<(String, String)>>::from_request_parts(&mut parts, &state).await {
            Ok(q) => q,
            Err(error) => return error.into_response(),
        };
    catalog_v2(
        state.storage.as_ref(),
        &state.default_namespace,
        &params,
        kind,
    )
    .await
}
pub(crate) async fn catalog_v2(
    storage: &dyn super::state::StorageEngine,
    default_namespace: &str,
    params: &[(String, String)],
    kind: crate::serialization::catalog_page::CatalogKind,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let result =
        match crate::serialization::catalog_page::CatalogPageRequest::from_http(params, kind) {
            Ok(mut request) => {
                if !request.all_namespaces {
                    request
                        .namespace
                        .get_or_insert_with(|| default_namespace.into());
                }
                storage.list_catalog_page(request).await
            }
            Err(error) => Err(error.into()),
        };
    let result = result.map(|page| {
        let active = storage.catalog_profile_context();
        page.with_active_embedding(active)
    });
    let (status, envelope) = crate::catalog_page::response(result);
    #[cfg(test)]
    crate::serialization::page_schema::record_fixture(
        Some(kind),
        &serde_json::to_value(&envelope).unwrap(),
    );
    (
        StatusCode::from_u16(status).expect("fixed catalog status"),
        Json(envelope),
    )
        .into_response()
}

/// Contract2-only numeric policy replacement; existing application gate remains.
pub async fn update_namespace_policy(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use crate::serialization::contract::ContractVersion;
    use axum::{extract::FromRequest, response::IntoResponse};
    let version = match super::middleware::request_contract_version(request.headers()) {
        Ok(version) => version,
        Err(message) => {
            return AppError::BadRequest {
                message,
                field: Some("contractVersion".into()),
            }
            .into_response();
        }
    };
    if let Err(error) = version.require_supported() {
        return AppError::BadRequest {
            message: error.to_string(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    if version != ContractVersion::V2 {
        return AppError::BadRequest {
            message: "numeric policy replacement requires contractVersion 2".into(),
            field: Some("contractVersion".into()),
        }
        .into_response();
    }
    let key = match catalog_header_key(request.headers()) {
        Ok(key) => key,
        Err(error) => return native_policy_response(Err(error.into()), None),
    };
    let Json(body) = match Json::<serde_json::Value>::from_request(request, &state).await {
        Ok(body) => body,
        Err(error) => return error.into_response(),
    };
    update_namespace_policy_v2(state.namespaces.as_ref(), &id, &body, key.as_deref()).await
}
pub(crate) async fn update_namespace_policy_v2(
    registry: &dyn super::state::NamespaceRegistry,
    id: &str,
    body: &serde_json::Value,
    key: Option<&str>,
) -> axum::response::Response {
    let mut namespace_id = None;
    let result = match crate::serialization::policy::PolicyRequest::from_http(body, id, key) {
        Ok(request) => {
            namespace_id = Some(request.namespace_id);
            registry.update_native_policy(request).await
        }
        Err(error) => Err(error.into()),
    };
    native_policy_response(result, namespace_id)
}
fn native_policy_response(
    result: Result<
        crate::native_catalog_adapter::PolicyOutcome,
        crate::native_catalog_adapter::CatalogError,
    >,
    id: Option<NamespaceId>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let (status, envelope) = crate::native_catalog_adapter::policy_response(result, id);
    (
        StatusCode::from_u16(status).expect("fixed status"),
        Json(envelope),
    )
        .into_response()
}
