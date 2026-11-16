//! Metadata-only paging over the actual owning engine. No vector/profile/provider,
//! cache, derived index, full-text hydration or exposure is involved.
use crate::{
    model::{MemoryId, schema::StructuredLabels},
    serialization::{
        contract::{CommitState, Envelope, ErrorCategory, ExecutionError, InputError, Scope},
        list::{ListCursor, ListPageRequest},
    },
    storage::{
        RedbStorageEngine,
        canonical_vector_snapshot::{SnapshotBinding, SnapshotError},
        control::Generation,
    },
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

// Encoded metadata bound per decode, including unrelated rows. The total scan
// has no hidden record cap: it uses constant working memory plus one page.
pub(crate) const MAX_ROW_BYTES: usize = 1024 * 1024;
// 32x covers worst-case escaping and both MCP copies, even with legacy arrays
// alongside exact canonical labels. Oversize rejects the entire page, no skips.
pub(crate) const MAX_PAGE_BYTES: usize =
    (crate::daemon::protocol::MAX_MESSAGE_SIZE - 64 * 1024) / 32;
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListEntry {
    pub id: MemoryId,
    pub summary: String,
    pub phase: crate::model::DecayPhase,
    pub created_at: i64,
    pub tags: Vec<crate::model::Tag>,
    pub entities: Vec<String>,
    pub topics: Vec<String>,
    pub emotions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<StructuredLabels>,
}
impl ListEntry {
    pub(crate) fn from_record(record: crate::model::DiskRecord) -> Self {
        let labels = record.schema.map(|s| s.labels);
        let legacy = crate::model::tag::parse_structured_tags(&record.tags);
        Self {
            id: MemoryId::from_bytes(record.id),
            summary: record.summary,
            phase: record.phase,
            created_at: record.created_at,
            entities: labels
                .as_ref()
                .map(|l| l.entities.clone())
                .unwrap_or(legacy.entities),
            topics: labels
                .as_ref()
                .map(|l| l.topics.clone())
                .unwrap_or(legacy.topics),
            emotions: labels
                .as_ref()
                .map(|l| l.emotions.clone())
                .unwrap_or(legacy.emotions),
            tags: record.tags,
            labels,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListData {
    pub memories: Vec<ListEntry>,
    /// Exact filtered live count before continuation; decimal, never JSON number.
    pub total: Generation,
    pub limit: usize,
    pub complete: bool,
    pub next_cursor: Option<String>,
}
#[derive(Debug)]
pub struct ListPage {
    pub(crate) binding: SnapshotBinding,
    pub(crate) namespace: String,
    pub(crate) data: ListData,
}
impl ListPage {
    pub fn into_envelope(self) -> Envelope<ListData> {
        let mut envelope = Envelope::success(
            Scope::Namespace {
                namespace_id: self.binding.namespace_id,
                namespace: self.namespace,
            },
            self.data,
        );
        envelope.read_generation = Some(self.binding.at);
        envelope
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ListError {
    #[error("{0}")]
    Input(#[from] InputError),
    #[error("namespace not found")]
    NotFound,
    #[error("cursor generation, lineage or namespace policy changed")]
    Stale,
    #[error("metadata page exceeds the byte budget; retry with a smaller limit")]
    TooLarge,
    #[error("canonical metadata unavailable")]
    Unavailable,
}
impl From<SnapshotError> for ListError {
    fn from(e: SnapshotError) -> Self {
        match e {
            SnapshotError::NamespaceNotFound => Self::NotFound,
            SnapshotError::Stale => Self::Stale,
            SnapshotError::LimitExceeded => Self::TooLarge,
            SnapshotError::Unavailable(_) => Self::Unavailable,
        }
    }
}
impl From<crate::storage::StorageError> for ListError {
    fn from(_: crate::storage::StorageError) -> Self {
        Self::Unavailable
    }
}
impl ListError {
    pub fn execution_error(&self) -> ExecutionError {
        let (category, code, field, retryable) = match self {
            Self::Input(e) => (
                ErrorCategory::InvalidInput,
                e.code,
                Some(e.field.clone()),
                false,
            ),
            Self::NotFound => (
                ErrorCategory::NotFound,
                "NAMESPACE_NOT_FOUND",
                Some("/namespace".into()),
                false,
            ),
            Self::Stale => (
                ErrorCategory::Conflict,
                "CURSOR_STALE",
                Some("/cursor".into()),
                false,
            ),
            Self::TooLarge => (
                ErrorCategory::InvalidInput,
                "RESPONSE_TOO_LARGE",
                Some("/limit".into()),
                false,
            ),
            Self::Unavailable => (
                ErrorCategory::Unavailable,
                "STORAGE_UNAVAILABLE",
                None,
                true,
            ),
        };
        ExecutionError {
            category,
            code: code.into(),
            field,
            retryable,
            message: self.to_string(),
            commit_state: CommitState::NotCommitted,
            operation_id: None,
        }
    }
}
pub fn response(result: Result<ListPage, ListError>) -> (u16, Envelope<ListData>) {
    match result {
        Ok(page) => (200, page.into_envelope()),
        Err(error) => {
            let status = match error {
                ListError::Input(_) | ListError::TooLarge => 400,
                ListError::NotFound => 404,
                ListError::Stale => 409,
                ListError::Unavailable => 503,
            };
            (status, Envelope::failure(None, error.execution_error()))
        }
    }
}
pub(crate) async fn list_page(
    storage: Arc<RwLock<RedbStorageEngine>>,
    request: ListPageRequest,
) -> Result<ListPage, ListError> {
    if !(1..=crate::model::constants::MAX_LIST_LIMIT).contains(&request.limit) {
        return Err(crate::serialization::list::invalid(
            "OUT_OF_RANGE",
            "/limit",
            "limit must be 1–200",
        )
        .into());
    }
    let namespace = request.namespace.clone().ok_or_else(|| {
        crate::serialization::list::invalid(
            "INVALID_COMBINATION",
            "/namespace",
            "resolve namespace before reading",
        )
    })?;
    let cursor = request
        .cursor
        .as_deref()
        .map(|s| ListCursor::decode(s, &request))
        .transpose()?;
    // Admit before scheduling. Dropping a cancelled waiter does not drop work:
    // the blocking closure owns the root through canonical selection/validation.
    let root = storage
        .read()
        .map_err(|_| ListError::Unavailable)?
        .meta_store()
        .work_tracker()
        .admit(None)
        .map_err(crate::storage::StorageError::from)?;
    tokio::task::spawn_blocking(move || {
        let engine = storage.read().map_err(|_| ListError::Unavailable)?;
        let result =
            engine
                .meta_store()
                .canonical_list_page(&namespace, &request, cursor.as_ref(), &root);
        engine.meta_store().finish_work(root);
        result
    })
    .await
    .map_err(|_| ListError::Unavailable)?
}

#[cfg(test)]
#[path = "canonical_list_tests.rs"]
mod tests;
