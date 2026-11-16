//! Canonical catalog pages on the existing owning engine. No derived vocabulary
//! or vectors are read. Profile compatibility is configuration evidence only.
use crate::{
    model::{
        NamespaceId,
        schema::{BindingProvenance, EmbeddingBinding},
    },
    serialization::{
        catalog_page::{CatalogCursor, CatalogKind, CatalogPageRequest},
        contract::{CommitState, Envelope, ErrorCategory, ExecutionError, InputError, Scope},
        session::ActiveEmbedding,
    },
    storage::{
        RedbStorageEngine, StorageError,
        control::{Generation, GenerationRef},
    },
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

pub(crate) const MAX_NAMESPACE_ROWS: usize = 100_000;
pub(crate) const MAX_CATALOG_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_LABELS: usize = 100_000;
pub(crate) const MAX_LABEL_BYTES: usize = 8 * 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum ProfileCompatibility {
    Compatible,
    UnknownNamespaceProfile,
    UnknownActiveProfile,
    DimensionMismatch,
    ProfileMismatch,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamespaceEntry {
    pub id: NamespaceId,
    pub name: String,
    pub embedding_dim: u32,
    pub memory_count: Generation,
    pub created_at: i64,
    pub embedding_profile_id: Option<String>,
    pub binding_provenance: Option<BindingProvenance>,
    pub profile_compatibility: ProfileCompatibility,
}
impl NamespaceEntry {
    pub(crate) fn from_config(
        config: &crate::model::NamespaceConfig,
        count: u64,
    ) -> Result<Self, CatalogError> {
        let (embedding_profile_id, binding_provenance) =
            match config.schema.as_ref().map(|s| &s.embedding_binding) {
                Some(EmbeddingBinding::Bound {
                    profile_id,
                    provenance,
                    ..
                }) => (Some(profile_id.clone()), Some(*provenance)),
                _ => (None, None),
            };
        let profile_compatibility = if embedding_profile_id.is_none() {
            ProfileCompatibility::UnknownNamespaceProfile
        } else {
            ProfileCompatibility::UnknownActiveProfile
        };
        Ok(Self {
            id: config.id,
            name: config.name.clone(),
            embedding_dim: config.embedding_dim,
            memory_count: counter(count)?,
            created_at: config.created_at,
            embedding_profile_id,
            binding_provenance,
            profile_compatibility,
        })
    }
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabelSources {
    pub raw_tags: Generation,
    pub canonical_legacy_retained_only: Generation,
    pub canonical_native_v2: Generation,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabelEntry {
    pub name: String,
    pub count: Generation,
    pub sources: LabelSources,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabelBucket {
    pub total: Generation,
    /// True iff this bucket has more entries after this page's anchor.
    pub truncated: bool,
    pub items: Vec<LabelEntry>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagData {
    pub memories_counted: Generation,
    pub limit: usize,
    pub tags: LabelBucket,
    pub entities: LabelBucket,
    pub topics: LabelBucket,
    pub emotions: LabelBucket,
    pub complete: bool,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamespaceData {
    pub namespaces: Vec<NamespaceEntry>,
    pub total: Generation,
    pub limit: usize,
    pub complete: bool,
    pub next_cursor: Option<String>,
    pub configured_embedding: Option<ActiveEmbedding>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogData {
    Namespaces(NamespaceData),
    Tags(TagData),
}
#[derive(Debug)]
pub struct CatalogPage {
    pub(crate) at: GenerationRef,
    pub(crate) scope: Scope,
    pub(crate) data: CatalogData,
}
impl CatalogPage {
    /// Active configuration is not part of the canonical catalog or eligibility.
    /// Unavailable context never blocks metadata access or invents compatibility.
    pub(crate) fn with_active_embedding(mut self, active: Option<ActiveEmbedding>) -> Self {
        if let CatalogData::Namespaces(data) = &mut self.data {
            // A decoded Remote page already carries the daemon configuration.
            // Do not replace it with the client adapter context (including None).
            let active = data.configured_embedding.clone().or(active);
            for entry in &mut data.namespaces {
                entry.profile_compatibility = match (&entry.embedding_profile_id, &active) {
                    (None, _) => ProfileCompatibility::UnknownNamespaceProfile,
                    (_, None) => ProfileCompatibility::UnknownActiveProfile,
                    (_, Some(ActiveEmbedding::Unknown { dimension }))
                        if entry.embedding_dim != *dimension =>
                    {
                        ProfileCompatibility::DimensionMismatch
                    }
                    (_, Some(ActiveEmbedding::Unknown { .. })) => {
                        ProfileCompatibility::UnknownActiveProfile
                    }
                    (_, Some(ActiveEmbedding::Configuration { dimension, .. }))
                        if entry.embedding_dim != *dimension =>
                    {
                        ProfileCompatibility::DimensionMismatch
                    }
                    (Some(stored), Some(ActiveEmbedding::Configuration { profile_id, .. }))
                        if stored == profile_id =>
                    {
                        ProfileCompatibility::Compatible
                    }
                    _ => ProfileCompatibility::ProfileMismatch,
                };
            }
            data.configured_embedding = active;
        }
        self
    }
    pub fn into_envelope(self) -> Envelope<CatalogData> {
        let mut out = Envelope::success(self.scope, self.data);
        out.read_generation = Some(self.at);
        out
    }
}
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("{0}")]
    Input(#[from] InputError),
    #[error("namespace not found")]
    NotFound,
    #[error("catalog generation, lineage or scope changed")]
    Stale,
    #[error("catalog page exceeds response budget; retry with a smaller limit")]
    ResponseTooLarge,
    #[error("canonical catalog exceeds declared namespace or vocabulary aggregation bounds")]
    CatalogTooLarge,
    #[error("canonical catalog unavailable")]
    Unavailable,
}
impl From<StorageError> for CatalogError {
    fn from(_: StorageError) -> Self {
        Self::Unavailable
    }
}
impl From<crate::storage::canonical_vector_snapshot::SnapshotError> for CatalogError {
    fn from(value: crate::storage::canonical_vector_snapshot::SnapshotError) -> Self {
        use crate::storage::canonical_vector_snapshot::SnapshotError::*;
        match value {
            NamespaceNotFound => Self::NotFound,
            Stale => Self::Stale,
            LimitExceeded => Self::CatalogTooLarge,
            Unavailable(_) => Self::Unavailable,
        }
    }
}
impl CatalogError {
    fn execution_error(&self) -> ExecutionError {
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
            Self::ResponseTooLarge => (
                ErrorCategory::InvalidInput,
                "RESPONSE_TOO_LARGE",
                Some("/limit".into()),
                false,
            ),
            Self::CatalogTooLarge => (
                ErrorCategory::InvalidInput,
                "RESPONSE_TOO_LARGE",
                None,
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
            message: self.to_string(),
            field,
            retryable,
            commit_state: CommitState::NotCommitted,
            operation_id: None,
        }
    }
}
pub fn response(result: Result<CatalogPage, CatalogError>) -> (u16, Envelope<CatalogData>) {
    match result {
        Ok(page) => (200, page.into_envelope()),
        Err(e) => {
            let status = match &e {
                CatalogError::Input(_)
                | CatalogError::ResponseTooLarge
                | CatalogError::CatalogTooLarge => 400,
                CatalogError::NotFound => 404,
                CatalogError::Stale => 409,
                CatalogError::Unavailable => 503,
            };
            (status, Envelope::failure(None, e.execution_error()))
        }
    }
}
pub(crate) fn counter(n: u64) -> Result<Generation, CatalogError> {
    n.to_string()
        .try_into()
        .map_err(|_| CatalogError::CatalogTooLarge)
}
pub(crate) fn retain<T: Serialize>(value: &T, bytes: &mut usize) -> Result<(), CatalogError> {
    // Each entry is independently bounded by the canonical row/label limits.
    // Reuse PR100's conservative complete MCP text+structured/frame allowance.
    let size = serde_json::to_vec(value)
        .map_err(|_| CatalogError::Unavailable)?
        .len();
    *bytes = bytes
        .checked_add(size)
        .filter(|n| *n <= crate::canonical_list::MAX_PAGE_BYTES)
        .ok_or(CatalogError::ResponseTooLarge)?;
    Ok(())
}
pub(crate) async fn read(
    storage: Arc<RwLock<RedbStorageEngine>>,
    request: CatalogPageRequest,
) -> Result<CatalogPage, CatalogError> {
    if request.all_namespaces && request.namespace.is_some()
        || request.kind == CatalogKind::Namespaces && !request.all_namespaces
    {
        return Err(crate::serialization::list::invalid(
            "INVALID_COMBINATION",
            "/namespace",
            "invalid catalog scope combination",
        )
        .into());
    }
    let (_, max) = CatalogPageRequest::limits(request.kind);
    if request.limit == 0 || request.limit > max {
        return Err(crate::serialization::list::invalid(
            "OUT_OF_RANGE",
            "/limit",
            "limit exceeds catalog bounds",
        )
        .into());
    }
    if request.kind == CatalogKind::Tags && !request.all_namespaces && request.namespace.is_none() {
        return Err(crate::serialization::list::invalid(
            "INVALID_COMBINATION",
            "/namespace",
            "resolve namespace before catalog read",
        )
        .into());
    }
    let cursor = request
        .cursor
        .as_deref()
        .map(|c| CatalogCursor::decode(c, &request))
        .transpose()?;
    let root = storage
        .read()
        .map_err(|_| CatalogError::Unavailable)?
        .meta_store()
        .work_tracker()
        .admit(None)
        .map_err(StorageError::from)?;
    tokio::task::spawn_blocking(move || {
        let engine = storage.read().map_err(|_| CatalogError::Unavailable)?;
        let result = engine
            .meta_store()
            .canonical_catalog_page(&request, cursor.as_ref(), &root);
        engine.meta_store().finish_work(root);
        result
    })
    .await
    .map_err(|_| CatalogError::Unavailable)?
}

#[cfg(test)]
#[path = "catalog_page_tests.rs"]
mod tests;
