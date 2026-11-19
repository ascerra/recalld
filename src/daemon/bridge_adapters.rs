use std::sync::Arc;

use async_trait::async_trait;

use super::client::DaemonClient;
use super::protocol;
use crate::mcp::bridge::{
    self, BridgeError, CreateNamespaceInput, DuplicateCluster, HealthStatus, ListTagsInput,
    ListTagsResponse, MemoryRecord, NamespaceInfo, NamespaceStats, ReinforceResult, SearchHit,
    SearchInput, SearchPipeline, SearchResponse, StoreInput, StoredMemory, SubsystemHealth,
};
use crate::model::MemoryId;

// ═══════════════════════════════════════════════════════════════════════
// RemoteSearchAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Delegates search operations to the daemon over a Unix socket.
pub struct RemoteSearchAdapter {
    client: Arc<DaemonClient>,
}

impl RemoteSearchAdapter {
    /// Creates a new adapter backed by the given daemon client.
    pub fn new(client: Arc<DaemonClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SearchPipeline for RemoteSearchAdapter {
    async fn get_contract(
        &self,
        request: crate::serialization::get::GetRequest,
        response_limit: usize,
    ) -> crate::serialization::get::GetReply {
        super::prepared_get::remote(&self.client, request, response_limit).await
    }
    async fn scan_duplicate_pairs(
        &self,
        request: crate::serialization::scan::ExactScanRequest,
    ) -> Result<crate::search::exact_scan::ExactScanPage, crate::search::exact_scan::ExactScanError>
    {
        super::prepared_query::remote_pairs(&self.client, request).await
    }
    async fn query_contract(
        &self,
        request: crate::serialization::query::QueryRequest,
    ) -> crate::serialization::query_response::QueryReply {
        super::prepared_query::remote_query(&self.client, request).await
    }
    async fn similar_contract(
        &self,
        request: crate::serialization::query::SimilarRequest,
    ) -> crate::serialization::query_response::QueryReply {
        super::prepared_query::remote_similar(&self.client, request).await
    }
    async fn search(&self, query: SearchInput) -> Result<SearchResponse, BridgeError> {
        let params =
            serde_json::to_value(&query).map_err(|e| BridgeError::Internal(e.to_string()))?;
        // Private aliases fail closed against daemons that would ignore the flag.
        let method = if query.record_access {
            "search"
        } else {
            "search_read_only"
        };
        let result = self.client.call(method, params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn find_similar(
        &self,
        id: MemoryId,
        limit: usize,
        min_score: Option<f32>,
        same_namespace: bool,
        record_access: bool,
        namespace: Option<&str>,
    ) -> Result<Vec<SearchHit>, BridgeError> {
        let params = serde_json::to_value(protocol::FindSimilarParams {
            record_access,
            id: id.to_string(),
            limit,
            min_score,
            same_namespace,
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let method = if record_access {
            "find_similar"
        } else {
            "find_similar_read_only"
        };
        let result = call_scoped(&self.client, method, params, namespace).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn scan_duplicates(
        &self,
        namespace: &str,
        threshold: f32,
        max_memories: usize,
    ) -> Result<Vec<DuplicateCluster>, BridgeError> {
        let params = serde_json::to_value(protocol::ScanDuplicatesParams {
            namespace: namespace.to_string(),
            threshold,
            max_memories,
            include_coverage: false,
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("scan_duplicates", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }
    async fn scan_duplicates_with_coverage(
        &self,
        namespace: &str,
        threshold: f32,
        max_memories: usize,
    ) -> Result<bridge::DuplicateScanReport, BridgeError> {
        let params = serde_json::to_value(protocol::ScanDuplicatesParams {
            namespace: namespace.to_string(),
            threshold,
            max_memories,
            include_coverage: true,
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("scan_duplicates", params).await?;
        // Old daemons ignore the new flag and still return just clusters.
        if result.is_array() {
            let clusters = serde_json::from_value(result)
                .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))?;
            Ok(bridge::DuplicateScanReport {
                clusters,
                coverage: None,
            })
        } else {
            serde_json::from_value(result)
                .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RemoteStorageAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Delegates storage operations to the daemon over a Unix socket.
pub struct RemoteStorageAdapter {
    client: Arc<DaemonClient>,
}

impl RemoteStorageAdapter {
    #[cfg(test)]
    pub(crate) fn test_client(&self) -> Arc<DaemonClient> {
        self.client.clone()
    }

    /// Creates a new adapter backed by the given daemon client.
    pub fn new(client: Arc<DaemonClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl bridge::StorageEngine for RemoteStorageAdapter {
    async fn move_memories(
        &self,
        input: crate::storage::native_move::NativeMoveInput,
        admission: crate::serialization::move_memories::Admission,
    ) -> crate::serialization::move_memories::Reply {
        use crate::serialization::move_memories::Reply;
        if let Err(error) = input.validate().and_then(|()| admission.check()) {
            return Reply::from_result(Err(error));
        }
        // Deliberately no capability fallback or transparent mutation replay.
        match self.client.call("move_memories_v1", serde_json::json!({
            "input":input, "caller":admission.caller(),
        })).await {
            Ok(value) => serde_json::from_value::<Reply>(value).map_err(|e| e.to_string())
                .and_then(|reply| { reply.validate()?; Ok(reply) }).unwrap_or_else(|_| Reply::error(
                "SERVICE_UNAVAILABLE", "unknown", None,
                "Daemon move response could not be decoded. Reconcile the original request with its original key.")),
            Err(_) => Reply::error("SERVICE_UNAVAILABLE", "unknown", None,
                "Daemon move call failed. Preserve the original request and key; reconcile before retrying. An older daemon must be updated and restarted."),
        }
    }

    async fn list_canonical_page(
        &self,
        request: crate::serialization::list::ListPageRequest,
    ) -> Result<crate::canonical_list::ListPage, crate::canonical_list::ListError> {
        super::prepared_pages::remote_list(&self.client, request).await
    }

    async fn list_catalog_page(
        &self,
        request: crate::serialization::catalog_page::CatalogPageRequest,
    ) -> Result<crate::catalog_page::CatalogPage, crate::catalog_page::CatalogError> {
        super::prepared_pages::remote_catalog(&self.client, request).await
    }

    async fn store_api1(
        &self,
        request: crate::serialization::store_api1::Request,
    ) -> crate::serialization::store_api1::Reply {
        use crate::serialization::{native_single, store_api1::Reply};
        let legacy = if request.validate_legacy().is_ok() {
            serde_json::to_value(&request.input).ok()
        } else {
            None
        };
        let params = match serde_json::to_value(&request) {
            Ok(params) => params,
            Err(_) => {
                return Reply::Owned(native_single::Reply::failure(
                    crate::storage::StorageError::InvalidMutation(
                        "invalid API1 store request".into(),
                    ),
                ));
            }
        };
        match self.client.call_api1_store(params, legacy).await {
            Ok((true, value)) => Reply::decode(value, &request),
            Ok((false, value)) => match serde_json::from_value(value) {
                Ok(data) => Reply::Legacy(Ok(data)),
                Err(_) => Reply::Owned(native_single::Reply::uncertain()),
            },
            Err(_) => Reply::Owned(native_single::Reply::uncertain()),
        }
    }
    async fn store_native_wire(
        &self,
        request: crate::serialization::native_single::Request,
    ) -> crate::serialization::native_single::Reply {
        use crate::serialization::native_single::Reply;
        let params = match serde_json::to_value(request) {
            Ok(v) => v,
            Err(_) => {
                return Reply::failure(crate::storage::StorageError::InvalidMutation(
                    "invalid native single request".into(),
                ));
            }
        };
        match self
            .client
            .call("store_native_single_observed_v1", params)
            .await
        {
            Ok(value) => serde_json::from_value(value).unwrap_or_else(|_| Reply::uncertain()),
            Err(_) => Reply::uncertain(),
        }
    }
    async fn store_batch_api1(
        &self,
        request: crate::serialization::batch_api1::Request,
    ) -> crate::serialization::batch_api1::Reply {
        use crate::serialization::batch_api1::Reply;
        if let Err(e) = request.validate() {
            return Reply::invalid(e);
        }
        let legacy = request
            .legacy()
            .ok()
            .and_then(|_| serde_json::to_value(&request.input).ok());
        let params = match serde_json::to_value(&request) {
            Ok(v) => v,
            Err(_) => return Reply::Uncertain(None),
        };
        match self.client.call_api1_store_batch(params, legacy).await {
            Ok((true, value)) => Reply::decode(value, &request),
            Ok((false, value)) => match serde_json::from_value(value) {
                Ok(out) => Reply::Legacy(Ok(out)),
                Err(_) => Reply::Uncertain(None),
            },
            Err(_) => Reply::Uncertain(None),
        }
    }
    async fn mutate_native_batch(
        &self,
        input: crate::storage::batch::BatchInput,
    ) -> Result<
        crate::serialization::native_batch::NativeBatchOutcome,
        crate::serialization::native_batch::BatchFailure,
    > {
        use crate::serialization::native_batch::{BatchFailure, NativeBatchReply};
        let params = serde_json::to_value(input).map_err(|_| BatchFailure::unavailable())?;
        // Dedicated non-retrying RPC. Never fall back to the legacy writer.
        let value = self
            .client
            .call("mutate_native_batch_observed_v1", params)
            .await
            .map_err(|_| BatchFailure::uncertain())?;
        serde_json::from_value::<NativeBatchReply>(value)
            .map_err(|_| BatchFailure::uncertain())?
            .result()
    }

    async fn api1_lifecycle(
        &self,
        request: crate::serialization::native_lifecycle::Request,
        operation: crate::serialization::native_lifecycle::Operation,
    ) -> crate::serialization::lifecycle_api1::Reply {
        use crate::serialization::{
            lifecycle_api1::{self, Reply, Value},
            native_lifecycle::{self, Operation},
        };
        if let Err(error) = lifecycle_api1::parse(&request.wire(operation), operation) {
            return Reply::failure(native_lifecycle::Reply::input(error));
        }
        let (new_method, old_method) = match operation {
            Operation::Feedback => ("feedback_api1_v1", "reinforce_memory"),
            Operation::Forget => ("forget_api1_v1", "delete_memory"),
        };
        let mut old_params = serde_json::json!({"id":request.id});
        if operation == Operation::Feedback {
            old_params["quality"] = serde_json::json!(request.quality);
        }
        let old_method = if let Some(namespace) = request.namespace.as_ref() {
            old_params["namespace"] = serde_json::json!(namespace);
            format!("{old_method}_scoped")
        } else {
            old_method.into()
        };
        match self
            .client
            .call_api1_lifecycle(new_method, request.wire(operation), &old_method, old_params)
            .await
        {
            Ok((true, value)) => Reply::decode(value, &request, operation),
            Ok((false, value)) => {
                let parsed = match operation {
                    Operation::Feedback => {
                        serde_json::from_value(value).map(|result| Value::Feedback { result })
                    }
                    Operation::Forget => {
                        serde_json::from_value(value).map(|deleted| Value::Forget { deleted })
                    }
                };
                match parsed {
                    Ok(value) => Reply::compatibility(value),
                    Err(error) => Reply::failure(native_lifecycle::Reply::storage(
                        crate::storage::StorageError::WorkerOutcomeUnknown(format!(
                            "old API1 response decode: {error}"
                        )),
                    )),
                }
            }
            Err(error) => Reply::bridge_error(error),
        }
    }

    async fn forget_native(
        &self,
        request: crate::serialization::native_lifecycle::Request,
    ) -> crate::serialization::native_lifecycle::Reply {
        use crate::serialization::{
            contract::CommitState,
            native_lifecycle::{Operation, Reply},
        };
        match self
            .client
            .call(
                request.rpc_method(Operation::Forget),
                request.wire(Operation::Forget),
            )
            .await
        {
            Ok(value) => Reply::decode_response(value, &request, Operation::Forget),
            Err(error) => {
                let mut reply = Reply::unavailable(&error.to_string());
                // Only the server's typed rejection proves that the method was
                // rejected. Transport/decode failures never imply rollback.
                if !matches!(error, BridgeError::InvalidInput(_)) {
                    reply.error.as_mut().unwrap().commit_state = CommitState::Unknown;
                }
                reply
            }
        }
    }

    async fn feedback_native(
        &self,
        request: crate::serialization::native_lifecycle::Request,
    ) -> crate::serialization::native_lifecycle::Reply {
        use crate::serialization::{
            contract::CommitState,
            native_lifecycle::{Operation, Reply},
        };
        match self
            .client
            .call(
                request.rpc_method(Operation::Feedback),
                request.wire(Operation::Feedback),
            )
            .await
        {
            Ok(value) => Reply::decode_response(value, &request, Operation::Feedback),
            Err(error) => {
                let mut reply = Reply::unavailable(&error.to_string());
                // Only the server's typed rejection proves that the method was
                // rejected. Transport/decode failures never imply rollback.
                if !matches!(error, BridgeError::InvalidInput(_)) {
                    reply.error.as_mut().unwrap().commit_state = CommitState::Unknown;
                }
                reply
            }
        }
    }

    fn supports_durable_batches(&self) -> bool {
        true
    }

    async fn mutate_batch(
        &self,
        input: crate::storage::batch::BatchInput,
    ) -> Result<crate::storage::batch::BatchOutcome, BridgeError> {
        let params =
            serde_json::to_value(input).map_err(|e| BridgeError::Internal(e.to_string()))?;
        // This new method is mutating and never enters the client's read-only
        // retry allowlist. An old daemon rejects it before doing any work.
        let result = self.client.call("mutate_batch", params).await?;
        serde_json::from_value(result).map_err(|e| {
            BridgeError::PartiallyApplied(format!(
                "batch response decode failed; reconcile the same request and key: {e}"
            ))
        })
    }

    async fn backend_context(
        &self,
    ) -> Result<crate::serialization::session::BackendContext, BridgeError> {
        // Dedicated RPC: an old daemon cannot silently ignore new context fields.
        let value = self
            .client
            .call("session_context_v1", serde_json::json!({}))
            .await?;
        let context: crate::serialization::session::BackendContext = serde_json::from_value(value)
            .map_err(|_| BridgeError::Unavailable("invalid backend context response".into()))?;
        context
            .validate()
            .map_err(|e| BridgeError::Unavailable(e.into()))?;
        Ok(context)
    }
    async fn store_memory(&self, input: StoreInput) -> Result<StoredMemory, BridgeError> {
        let params =
            serde_json::to_value(&input).map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("store_memory", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    fn supports_api1_get(&self) -> bool {
        true
    }
    async fn get_api1(
        &self,
        request: crate::serialization::get_api1::Request,
        response_limit: usize,
    ) -> crate::serialization::get_api1::Reply {
        super::api1_get::remote(&self.client, request, response_limit).await
    }

    async fn get_memory(
        &self,
        id: MemoryId,
        record_access: bool,
        namespace: Option<&str>,
    ) -> Result<Option<MemoryRecord>, BridgeError> {
        let params = serde_json::to_value(protocol::GetMemoryParams {
            id: id.to_string(),
            record_access,
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let method = if record_access {
            "get_memory"
        } else {
            "get_memory_read_only"
        };
        let result = call_scoped(&self.client, method, params, namespace).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn delete_memory(
        &self,
        id: MemoryId,
        namespace: Option<&str>,
    ) -> Result<bool, BridgeError> {
        let params = serde_json::to_value(protocol::DeleteMemoryParams { id: id.to_string() })
            .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = call_scoped(&self.client, "delete_memory", params, namespace).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn delete_memories(&self, ids: &[MemoryId]) -> Result<Vec<bool>, BridgeError> {
        let params = serde_json::to_value(protocol::DeleteMemoriesParams {
            ids: ids.iter().map(|id| id.to_string()).collect(),
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("delete_memories", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn reinforce_memory(
        &self,
        id: MemoryId,
        quality: u8,
        namespace: Option<&str>,
    ) -> Result<ReinforceResult, BridgeError> {
        let params = serde_json::to_value(protocol::ReinforceParams {
            id: id.to_string(),
            quality,
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = call_scoped(&self.client, "reinforce_memory", params, namespace).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn list_memories(
        &self,
        input: bridge::ListMemoriesInput,
    ) -> Result<bridge::ListMemoriesResponse, BridgeError> {
        let params =
            serde_json::to_value(&input).map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("list_memories", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn list_tags(&self, input: ListTagsInput) -> Result<ListTagsResponse, BridgeError> {
        let params =
            serde_json::to_value(&input).map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("list_tags", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RemoteNamespaceAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Delegates namespace operations to the daemon over a Unix socket.
pub struct RemoteNamespaceAdapter {
    client: Arc<DaemonClient>,
}

impl RemoteNamespaceAdapter {
    /// Creates a new adapter backed by the given daemon client.
    pub fn new(client: Arc<DaemonClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl bridge::NamespaceRegistry for RemoteNamespaceAdapter {
    async fn list_namespaces(&self) -> Result<Vec<NamespaceInfo>, BridgeError> {
        let result = self
            .client
            .call("list_namespaces", serde_json::Value::Null)
            .await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn create_namespace(
        &self,
        input: CreateNamespaceInput,
    ) -> Result<NamespaceInfo, BridgeError> {
        let params =
            serde_json::to_value(&input).map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("create_namespace", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn create_namespace_bounded(
        &self,
        input: CreateNamespaceInput,
        admission: crate::serialization::catalog_api1::CatalogReplyAdmission,
    ) -> Result<NamespaceInfo, crate::serialization::catalog_api1::CatalogApi1Error> {
        let result = self
            .client
            .call(
                "create_namespace_bounded_v1",
                serde_json::json!({
                    "input": input, "caller": admission.caller(),
                }),
            )
            .await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")).into())
    }

    async fn delete_namespace(
        &self,
        input: bridge::DeleteNamespaceInput,
    ) -> Result<bridge::DeleteNamespaceResult, BridgeError> {
        let params =
            serde_json::to_value(&input).map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("delete_namespace", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }

    async fn namespace_stats(&self, name: &str) -> Result<NamespaceStats, BridgeError> {
        let params = serde_json::to_value(protocol::NamespaceStatsParams {
            name: name.to_string(),
        })
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
        let result = self.client.call("namespace_stats", params).await?;
        serde_json::from_value(result)
            .map_err(|e| BridgeError::Internal(format!("response decode: {e}")))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RemoteHealthAdapter
// ═══════════════════════════════════════════════════════════════════════

/// Delegates health checks to the daemon over a Unix socket.
pub struct RemoteHealthAdapter {
    client: Arc<DaemonClient>,
}

impl RemoteHealthAdapter {
    /// Creates a new adapter backed by the given daemon client.
    pub fn new(client: Arc<DaemonClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl bridge::HealthChecker for RemoteHealthAdapter {
    async fn check_health(&self) -> HealthStatus {
        match self
            .client
            .call("check_health", serde_json::Value::Null)
            .await
        {
            Ok(result) => serde_json::from_value(result).unwrap_or_else(|e| HealthStatus {
                status: "degraded".to_string(),
                uptime_secs: 0,
                subsystems: vec![SubsystemHealth {
                    name: "daemon".to_string(),
                    status: "error".to_string(),
                    message: Some(format!("response decode: {e}")),
                }],
            }),
            Err(e) => HealthStatus {
                status: "degraded".to_string(),
                uptime_secs: 0,
                subsystems: vec![SubsystemHealth {
                    name: "daemon".to_string(),
                    status: "error".to_string(),
                    message: Some(e.to_string()),
                }],
            },
        }
    }
}

// A separate method makes old daemons reject assertions instead of ignoring fields.
async fn call_scoped(
    client: &DaemonClient,
    method: &str,
    mut params: serde_json::Value,
    namespace: Option<&str>,
) -> Result<serde_json::Value, BridgeError> {
    if let Some(namespace) = namespace {
        params["namespace"] = serde_json::Value::String(namespace.to_owned());
        // Assertion-only servers reject recordAccess fields. Combined false
        // requests use their own alias so neither control can be ignored.
        params.as_object_mut().unwrap().remove("recordAccess");
        let method = match method.strip_suffix("_read_only") {
            Some(base) => format!("{base}_scoped_read_only"),
            None => format!("{method}_scoped"),
        };
        client.call(&method, params).await
    } else {
        client.call(method, params).await
    }
}
