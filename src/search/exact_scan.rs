//! Concrete canonical/vector snapshot consumer. Public v2 transport activation
//! remains gated; this module never substitutes an effects epoch or provider ID.
use super::exact_pairs::{PairEdge, PairError, compare_page};
use crate::{
    model::{DecayPhase, NamespaceId, schema::EmbeddingBinding},
    serialization::{
        contract::{CommitState, ErrorCategory, ExecutionError, InputError},
        pair_cursor::{PAIR_ELIGIBILITY_VERSION, PairCursor},
        scan::{ExactScanRequest, MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_RECORDS},
    },
    storage::{
        RedbStorageEngine,
        canonical_vector_snapshot::{
            SnapshotBinding, SnapshotError, SnapshotLimits, SnapshotVector,
        },
        control::GenerationRef,
    },
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactScanPage {
    pub namespace: String,
    pub namespace_id: NamespaceId,
    pub data: ExactScanData,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub(crate) diagnostics: ScanDiagnostics,
}
impl ExactScanPage {
    pub fn into_envelope(self) -> crate::serialization::contract::Envelope<ExactScanData> {
        tracing::debug!(rows=self.diagnostics.rows,
            record_encoded_bytes=self.diagnostics.record_encoded_bytes,
            vector_examined_bytes=self.diagnostics.vector_examined_bytes,
            vector_copied_bytes=self.diagnostics.vector_copied_bytes,
            capture_us=%self.diagnostics.capture_time.as_micros(),
            comparison_us=%self.diagnostics.comparison_time.as_micros(),
            compared=self.data.coverage.compared_this_page,
            "canonical exact scan page");
        let at = self.data.coverage.read_generation;
        let mut envelope = crate::serialization::contract::Envelope::success(
            crate::serialization::contract::Scope::Namespace {
                namespace_id: self.namespace_id,
                namespace: self.namespace,
            },
            self.data,
        );
        envelope.read_generation = Some(at);
        envelope.warnings = self.warnings;
        envelope
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactScanData {
    pub pairs: Vec<PairEdge>,
    pub coverage: ExactCoverage,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactCoverage {
    pub algorithm: ExactPairAlgorithm,
    pub read_generation: GenerationRef,
    pub embedding_profile_id: String,
    pub eligible_count: u64,
    pub excluded_count: u64,
    pub total_pairs: u64,
    pub compared_this_page: u32,
    pub compared_so_far: u64,
    pub complete: bool,
    /// All captured namespace rows, including terminal and excluded rows.
    pub phase_counts: BTreeMap<&'static str, u64>,
    pub exclusions: BTreeMap<&'static str, u64>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExactPairAlgorithm {
    ExactPairs,
}
#[derive(Debug)]
pub(crate) struct ScanDiagnostics {
    pub rows: usize,
    pub record_encoded_bytes: usize,
    pub vector_examined_bytes: usize,
    pub vector_copied_bytes: usize,
    pub capture_time: Duration,
    pub comparison_time: Duration,
}
#[derive(Debug, thiserror::Error)]
pub enum ExactScanError {
    #[error("{0}")]
    Input(#[from] InputError),
    #[error("namespace not found")]
    NamespaceNotFound,
    #[error("CURSOR_STALE: canonical namespace, profile, policy or lineage changed")]
    Stale,
    #[error("canonical snapshot exceeds the record or byte budget")]
    TooLarge,
    #[error("exact scan response exceeds the transport byte budget; reduce pairBudget")]
    ResponseTooLarge,
    #[error("namespace requires an explicit compatible embedding profile binding")]
    ProfileUnavailable,
    #[error("canonical vector snapshot is unavailable")]
    Unavailable,
}
impl From<SnapshotError> for ExactScanError {
    fn from(error: SnapshotError) -> Self {
        match error {
            SnapshotError::NamespaceNotFound => Self::NamespaceNotFound,
            SnapshotError::Stale => Self::Stale,
            SnapshotError::LimitExceeded => Self::TooLarge,
            SnapshotError::Unavailable(_) => Self::Unavailable,
        }
    }
}
impl ExactScanError {
    pub fn execution_error(&self) -> ExecutionError {
        let (category, code, field, retryable) = match self {
            Self::Input(e) => (
                ErrorCategory::InvalidInput,
                e.code,
                Some(e.field.clone()),
                false,
            ),
            Self::NamespaceNotFound => (
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
                None,
                false,
            ),
            Self::ResponseTooLarge => (
                ErrorCategory::InvalidInput,
                "RESPONSE_TOO_LARGE",
                Some("/pairBudget".into()),
                false,
            ),
            Self::ProfileUnavailable => (
                ErrorCategory::Conflict,
                "EMBEDDING_PROFILE_MISMATCH",
                Some("/namespace".into()),
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

pub(crate) async fn scan_exact(
    storage: Arc<RwLock<RedbStorageEngine>>,
    request: ExactScanRequest,
) -> Result<ExactScanPage, ExactScanError> {
    scan_exact_after_capture(storage, request, || {}).await
}

// Deterministic test synchronization uses this callback after releasing capture
// locks; production supplies a no-op. Storage and comparator are always real.
pub(crate) async fn scan_exact_after_capture<F>(
    storage: Arc<RwLock<RedbStorageEngine>>,
    request: ExactScanRequest,
    after_capture: F,
) -> Result<ExactScanPage, ExactScanError>
where
    F: FnOnce() + Send + 'static,
{
    let namespace = request
        .namespace
        .as_deref()
        .ok_or_else(|| InputError {
            code: "INVALID_COMBINATION",
            field: "/namespace".into(),
            message: "resolve namespace before scanning",
        })?
        .to_owned();
    crate::model::validation::validate_namespace_name(&namespace).map_err(|_| InputError {
        code: "OUT_OF_RANGE",
        field: "/namespace".into(),
        message: "invalid namespace",
    })?;
    let cursor = request
        .cursor
        .as_deref()
        .map(|value| PairCursor::decode(value, &request))
        .transpose()?;
    tokio::task::spawn_blocking(move || {
        let expected = cursor.as_ref().map(|cursor| SnapshotBinding {
            at: cursor.at,
            namespace_id: cursor.namespace_id,
            namespace_sha256: cursor.namespace_sha256,
        });
        let started = Instant::now();
        let snapshot = {
            let engine = storage.read().map_err(|_| ExactScanError::Unavailable)?;
            engine.canonical_vector_snapshot(
                &namespace,
                expected.as_ref(),
                SnapshotLimits { max_records: MAX_SNAPSHOT_RECORDS, max_bytes: MAX_SNAPSHOT_BYTES },
            )?
        }; // Capture guards end before vector comparison.
        let capture_time = started.elapsed();
        after_capture();
        let profile = match snapshot.namespace.schema.as_ref().map(|s| &s.embedding_binding) {
            Some(EmbeddingBinding::Bound { profile_id, descriptor, .. })
                if descriptor.dimension == snapshot.namespace.embedding_dim
                    && descriptor.profile_id().as_ref() == Ok(profile_id) => profile_id.clone(),
            _ => return Err(ExactScanError::ProfileUnavailable),
        };
        if let Some(cursor) = &cursor { cursor.matches_profile(&profile)?; }
        let mut diagnostics = ScanDiagnostics {
            rows: snapshot.rows.len(),
            record_encoded_bytes: snapshot.accounting.record_encoded_bytes,
            vector_examined_bytes: snapshot.accounting.vector_examined_bytes,
            vector_copied_bytes: snapshot.accounting.vector_copied_bytes,
            capture_time,
            comparison_time: Duration::ZERO,
        };
        let compare_started = Instant::now();
        let mut phase_counts = BTreeMap::from([
            ("full", 0), ("summary", 0), ("ghost", 0), ("tombstone", 0),
        ]);
        let mut exclusions = BTreeMap::new();
        let mut vectors = Vec::new();
        for row in snapshot.rows {
            let phase = match row.record.phase {
                DecayPhase::Full => "full",
                DecayPhase::Summary => "summary",
                DecayPhase::Ghost => "ghost",
                DecayPhase::Tombstone => "tombstone",
            };
            *phase_counts.get_mut(phase).expect("known phase") += 1;
            if row.record.phase == DecayPhase::Tombstone { continue; }
            use crate::storage::canonical_vector_snapshot::InvalidVectorReason;
            let reason = match row.vector {
                SnapshotVector::Available(values) => {
                    vectors.push((row.id, values));
                    continue;
                }
                SnapshotVector::Missing => "missingVector",
                SnapshotVector::Invalid(InvalidVectorReason::DimensionMismatch) => "dimensionMismatch",
                SnapshotVector::Invalid(InvalidVectorReason::NonFinite) => "nonFinite",
                SnapshotVector::Invalid(InvalidVectorReason::ZeroNorm) => "zeroNorm",
            };
            *exclusions.entry(reason).or_insert(0u64) += 1;
        }
        let compared = compare_page(&vectors, &request, cursor.as_ref().map(|c| c.next_pair))
            .map_err(|error| match error {
                PairError::InvalidPosition => ExactScanError::Input(InputError {
                    code: "INVALID_CURSOR",
                    field: "/cursor".into(),
                    message: "cursor pair position is outside eligible pairs",
                }),
                PairError::Bounds => ExactScanError::TooLarge,
                PairError::ResponseTooLarge => ExactScanError::ResponseTooLarge,
                PairError::InvalidVectors => ExactScanError::Unavailable,
            })?;
        diagnostics.comparison_time = compare_started.elapsed();
        let next_cursor = compared.next_pair.map(|next_pair| PairCursor {
            cursor_version: 1,
            contract_version: 2,
            at: snapshot.binding.at,
            namespace_id: snapshot.binding.namespace_id,
            namespace_sha256: snapshot.binding.namespace_sha256,
            profile_id: profile.clone(),
            threshold_bits: request.threshold.to_bits(),
            eligibility_version: PAIR_ELIGIBILITY_VERSION,
            next_pair,
        }.encode()).transpose()?;
        let excluded_count = exclusions.values().sum();
        let warnings = if excluded_count > 0 {
            vec!["Some live namespace records lack comparable vectors; terminal coverage cannot be complete.".into()]
        } else { vec![] };
        let page = ExactScanPage {
            namespace: snapshot.namespace.name,
            namespace_id: snapshot.binding.namespace_id,
            data: ExactScanData {
                pairs: compared.pairs,
                coverage: ExactCoverage {
                    algorithm: ExactPairAlgorithm::ExactPairs,
                    read_generation: snapshot.binding.at,
                    embedding_profile_id: profile,
                    eligible_count: vectors.len() as u64,
                    excluded_count,
                    total_pairs: compared.total_pairs,
                    compared_this_page: compared.compared_this_page,
                    compared_so_far: compared.compared_so_far,
                    complete: next_cursor.is_none() && excluded_count == 0,
                    phase_counts,
                    exclusions,
                },
                next_cursor,
            },
            warnings,
            diagnostics,
        };
        // Discard the entire computed page if actual generation or binding changed.
        storage.read().map_err(|_| ExactScanError::Unavailable)?
            .validate_vector_snapshot(&snapshot.binding)?;
        Ok(page)
    }).await.map_err(|_| ExactScanError::Unavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn exact_scan_response_budget_covers_real_mcp_text_structured_daemon_and_rest_serializers()
     {
        use crate::serialization::scan::{
            MAX_CURSOR_BYTES, MAX_RESPONSE_PAIRS, MAX_SCAN_RESPONSE_BYTES,
        };
        use crate::storage::control::Generation;
        // Authored serializer stress fixture, not canonical/native serving proof.
        // Maximum-width fixed fields and long f64 encodings exercise the actual
        // envelope and MCP serializer (including escaped pretty text duplication).
        let at = GenerationRef {
            store_id: uuid::Uuid::from_u128(u128::MAX),
            generation: Generation::MAX,
        };
        let pairs = (0..MAX_RESPONSE_PAIRS)
            .map(|i| PairEdge {
                left_id: crate::model::MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX - 1)),
                right_id: crate::model::MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX)),
                similarity: [f64::MIN_POSITIVE, f64::from_bits(1), 0.12345678901234567][i % 3],
            })
            .collect();
        let page = ExactScanPage {
            namespace: "n".repeat(64), namespace_id: NamespaceId::new(u32::MAX),
            data: ExactScanData {
                pairs,
                coverage: ExactCoverage {
                    algorithm: ExactPairAlgorithm::ExactPairs, read_generation: at,
                    embedding_profile_id: format!("ep1:{}", "f".repeat(64)),
                    eligible_count: u64::MAX, excluded_count: u64::MAX, total_pairs: u64::MAX,
                    compared_this_page: u32::MAX, compared_so_far: u64::MAX, complete: false,
                    phase_counts: BTreeMap::from([("full",u64::MAX),("summary",u64::MAX),("ghost",u64::MAX),("tombstone",u64::MAX)]),
                    exclusions: BTreeMap::from([("missingVector",u64::MAX),("dimensionMismatch",u64::MAX),("nonFinite",u64::MAX),("zeroNorm",u64::MAX)]),
                },
                next_cursor: Some("c".repeat(MAX_CURSOR_BYTES)),
            },
            warnings: vec!["Some live namespace records lack comparable vectors; terminal coverage cannot be complete.".into()],
            diagnostics: ScanDiagnostics {rows:0,record_encoded_bytes:0,vector_examined_bytes:0,vector_copied_bytes:0,capture_time:Duration::ZERO,comparison_time:Duration::ZERO},
        };
        let envelope = page.into_envelope();
        assert!(serde_json::to_vec(&envelope).unwrap().len() <= MAX_SCAN_RESPONSE_BYTES);
        let result = crate::mcp::protocol::ToolCallResult::json(&envelope).unwrap();
        assert!(result.structured_content.is_some());
        let rpc = crate::mcp::protocol::JsonRpcResponse::success(
            crate::mcp::protocol::JsonRpcId::Number(i64::MAX),
            serde_json::to_value(result).unwrap(),
        );
        assert!(serde_json::to_vec(&rpc).unwrap().len() <= MAX_SCAN_RESPONSE_BYTES);
        assert!(serde_json::to_vec_pretty(&rpc).unwrap().len() <= MAX_SCAN_RESPONSE_BYTES);
        let daemon = crate::daemon::protocol::DaemonResponse::success(
            u64::MAX,
            serde_json::to_value(&envelope).unwrap(),
        );
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        let (written, read) = tokio::join!(
            crate::daemon::protocol::write_framed_message(
                &mut writer,
                &daemon,
                MAX_SCAN_RESPONSE_BYTES
            ),
            crate::daemon::protocol::read_framed_response(&mut reader, MAX_SCAN_RESPONSE_BYTES),
        );
        written.unwrap();
        assert_eq!(read.unwrap().result, daemon.result);
        use axum::response::IntoResponse;
        let rest = axum::Json(&envelope).into_response();
        assert_eq!(rest.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(rest.into_body(), MAX_SCAN_RESPONSE_BYTES)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::to_value(&envelope).unwrap()
        );
        let error = ExactScanError::ResponseTooLarge.execution_error();
        assert_eq!(error.code, "RESPONSE_TOO_LARGE");
        assert_eq!(error.field.as_deref(), Some("/pairBudget"));
        assert_eq!(error.commit_state, CommitState::NotCommitted);
    }
    #[tokio::test]
    async fn exact_scan_actual_legacy_store_never_blesses_unknown_profile_or_writes() {
        use crate::storage::StorageEngine as _;
        let fixture = crate::test_support::Fixture::new();
        fixture.insert_memory().await;
        let before = fixture.storage.read().unwrap().control_state().unwrap();
        let records: Vec<_> = fixture
            .storage
            .read()
            .unwrap()
            .scan_all()
            .unwrap()
            .into_iter()
            .map(|(id, record)| (id, record.to_bytes()))
            .collect();
        let request = ExactScanRequest::from_http(&json!({}), "default").unwrap();
        assert!(matches!(
            scan_exact(fixture.storage.clone(), request).await,
            Err(ExactScanError::ProfileUnavailable)
        ));
        assert_eq!(
            fixture.storage.read().unwrap().control_state().unwrap(),
            before
        );
        let after: Vec<_> = fixture
            .storage
            .read()
            .unwrap()
            .scan_all()
            .unwrap()
            .into_iter()
            .map(|(id, record)| (id, record.to_bytes()))
            .collect();
        assert_eq!(after, records);
    }
}
