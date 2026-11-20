//! API1 catalog evidence admission for local and daemon-backed callers.
use crate::{
    mcp::protocol::{Era, JsonRpcId, JsonRpcResponse, ResultEnvelope, ToolCallResult},
    model::NamespaceConfig,
    native_catalog_adapter::CatalogError,
    storage::{
        StorageError,
        control::{MutationOutcome, WriteReceipt},
        maintenance::ReceiptAdmission,
    },
};
use serde_json::{Value, json};
use std::io::Write;
use uuid::Uuid;

const LOCAL_LIMIT: usize = 8_388_608;

type WorkerSnapshot = Result<
    (
        Option<crate::storage::maintenance::FailureKind>,
        Vec<WriteReceipt>,
        Option<WriteReceipt>,
    ),
    crate::storage::maintenance::InvalidReceiptEvidence,
>;

/// Opaque snapshot of the existing per-request collector, including invalid
/// state. It cannot be constructed from transport arguments.
#[derive(Debug)]
pub struct CatalogWorkerEvidence {
    snapshot: WorkerSnapshot,
}

impl std::fmt::Display for CatalogWorkerEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("catalog worker result evidence retained")
    }
}
impl std::error::Error for CatalogWorkerEvidence {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.snapshot {
            Err(invalid) => Some(invalid),
            Ok(_) => None,
        }
    }
}

impl CatalogWorkerEvidence {
    pub(crate) fn capture(admission: &ReceiptAdmission) -> Self {
        Self {
            snapshot: admission.snapshot(),
        }
    }

    #[cfg(test)]
    pub(crate) fn assert_invalid_retained_for_test(&self, mode: &str) {
        assert!(self.checked().is_err());
        match (&self.snapshot, mode) {
            (Ok((kind, receipts, failed)), "api1_invalid_multiple" | "api1_invalid_conflict") => {
                assert_eq!(receipts.len(), 2);
                assert!(failed.is_none());
                assert!(!receipts[0].replayed);
                if mode == "api1_invalid_conflict" {
                    assert_eq!(
                        *kind,
                        Some(crate::storage::maintenance::FailureKind::Conflict)
                    );
                    assert!(receipts[1].replayed);
                    assert_eq!(receipts[0].operation_id, receipts[1].operation_id);
                } else {
                    assert!(kind.is_none());
                    assert_eq!(receipts[1].operation_id, Uuid::from_u128(7));
                }
            }
            (Err(invalid), "api1_invalid_unadmitted") => {
                assert_eq!(invalid.prior.len(), 1);
                assert_eq!(invalid.unadmitted.len(), 1);
                assert!(invalid.failed_receipt.is_some());
                let MutationOutcome::NativeNamespaceCreated { transition } =
                    &invalid.unadmitted[0].outcome
                else {
                    panic!("full unadmitted proof lost")
                };
                assert_eq!(transition.name.len(), 8_388_609);
                assert_eq!(
                    crate::storage::control::encode(&invalid.unadmitted[0]).unwrap(),
                    crate::storage::control::encode(invalid.failed_receipt.as_ref().unwrap())
                        .unwrap()
                );
            }
            _ => panic!("unexpected invalid evidence kind"),
        }
    }

    pub(crate) fn checked(&self) -> Result<(Option<Uuid>, Option<&WriteReceipt>), &Self> {
        use crate::storage::maintenance::FailureKind;
        let Ok((kind, receipts, failed)) = &self.snapshot else {
            return Err(self);
        };
        let receipt = match receipts.as_slice() {
            [] => None,
            [receipt] => {
                if namespace(receipt).is_err() {
                    return Err(self);
                }
                Some(receipt)
            }
            _ => return Err(self),
        };
        if matches!(kind, Some(FailureKind::Conflict))
            || (matches!(kind, Some(FailureKind::Known)) && receipt.is_none())
        {
            return Err(self);
        }
        if let Some(failed) = failed {
            let Some(receipt) = receipt else {
                return Err(self);
            };
            match (
                crate::storage::control::encode(failed),
                crate::storage::control::encode(receipt),
            ) {
                (Ok(left), Ok(right)) if left == right => {}
                _ => return Err(self),
            }
        }
        let operation = match kind {
            Some(FailureKind::Unknown(Some(id))) if !id.is_nil() => Some(*id),
            Some(FailureKind::Unknown(Some(_))) => return Err(self),
            _ => None,
        };
        Ok((operation, receipt))
    }
}

/// Bounded create retains typed errors until the local tool or daemon boundary.
#[derive(Debug, thiserror::Error)]
pub enum CatalogApi1Error {
    #[error(transparent)]
    Legacy(#[from] crate::mcp::bridge::BridgeError),
    #[error(transparent)]
    Native(#[from] CatalogError),
}

/// Public only because local adapter traits carry it. Callers cannot deserialize
/// or construct authority; the actual emitter constructs the descriptor.
#[derive(Clone)]
pub struct CatalogReplyAdmission {
    surface: Surface,
    limit: usize,
    daemon_limit: Option<usize>,
}

/// Response-format inputs only; the daemon reconstructs and caps admission.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogCaller {
    id: JsonRpcId,
    mcp: bool,
    modern: bool,
    timezone: String,
    limit: usize,
}

#[derive(Clone)]
enum Surface {
    Mcp {
        id: JsonRpcId,
        era: Era,
        timezone: chrono_tz::Tz,
    },
    Http,
}

impl CatalogReplyAdmission {
    pub(crate) fn mcp(id: JsonRpcId, era: Era) -> Self {
        Self {
            surface: Surface::Mcp {
                id,
                era,
                timezone: chrono_tz::UTC,
            },
            limit: LOCAL_LIMIT,
            daemon_limit: None,
        }
    }

    pub(crate) fn http() -> Self {
        Self {
            surface: Surface::Http,
            limit: LOCAL_LIMIT,
            daemon_limit: None,
        }
    }

    pub(crate) fn caller(&self) -> CatalogCaller {
        let (id, modern, timezone) = match &self.surface {
            Surface::Mcp { id, era, timezone } => {
                (id.clone(), *era == Era::Modern, timezone.to_string())
            }
            Surface::Http => (JsonRpcId::Null, false, "UTC".into()),
        };
        CatalogCaller {
            id,
            mcp: matches!(self.surface, Surface::Mcp { .. }),
            modern,
            timezone,
            limit: self.limit,
        }
    }

    pub(crate) fn from_caller(
        caller: CatalogCaller,
        frame_limit: usize,
    ) -> Result<Self, crate::mcp::bridge::BridgeError> {
        let timezone = caller.timezone.parse().map_err(|_| {
            crate::mcp::bridge::BridgeError::InvalidInput("invalid catalog timezone".into())
        })?;
        let mut admission = if caller.mcp {
            Self::mcp(
                caller.id,
                if caller.modern {
                    Era::Modern
                } else {
                    Era::Legacy
                },
            )
        } else {
            Self::http()
        }
        .with_timezone(timezone);
        admission.limit = caller.limit.min(LOCAL_LIMIT);
        admission.daemon_limit = Some(frame_limit.min(crate::daemon::protocol::MAX_MESSAGE_SIZE));
        Ok(admission)
    }

    pub(crate) fn with_timezone(mut self, timezone: chrono_tz::Tz) -> Self {
        if let Surface::Mcp {
            timezone: selected, ..
        } = &mut self.surface
        {
            *selected = timezone;
        }
        self
    }

    /// Invoke only after selecting the constructor-validated native owner.
    /// Descriptor construction itself must not impose a new legacy limit.
    pub(crate) fn admit_native(&self) -> Result<ReceiptAdmission, StorageError> {
        self.check_failure(None)?;
        ReceiptAdmission::new(self.limit)
    }

    pub(crate) fn check_candidate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.check_failure(Some(receipt))?;
        let ns = namespace(receipt)?;
        let value = match &self.surface {
            Surface::Mcp { timezone, .. } => serde_json::to_value(info(ns, *timezone)),
            Surface::Http => {
                if let Some(limit) = self.daemon_limit {
                    check_encoded(
                        &crate::daemon::protocol::DaemonResponse::success(
                            u64::MAX,
                            serde_json::to_value(info(ns.clone(), chrono_tz::UTC))
                                .map_err(|_| StorageError::ReceiptBudgetExceeded)?,
                        ),
                        limit,
                    )?;
                }
                return check_encoded(
                    &super::ApiResponse {
                        data: http_namespace(ns),
                        took_us: Some(u64::MAX),
                    },
                    self.limit,
                );
            }
        }
        .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
        self.check_value(&value, false)
    }

    fn check_failure(&self, receipt: Option<&WriteReceipt>) -> Result<(), StorageError> {
        // Use the exact Value -> pretty tool text/structured content -> outer
        // envelope path. Typed f32 byte lengths are not a proxy for this path.
        for (code, state, ready) in [
            ("SERVICE_UNAVAILABLE", "committed", None),
            ("INDEX_NOT_READY", "committed", Some(false)),
            ("SERVICE_UNAVAILABLE", "unknown", None),
            ("RESPONSE_TOO_LARGE", "notCommitted", None),
        ] {
            let value = evidence(
                code,
                state,
                Some(Uuid::from_u128(u128::MAX)),
                receipt,
                ready,
                &"\0".repeat(512),
            );
            self.check_value(&value, true)?;
        }
        Ok(())
    }

    fn check_value(&self, value: &Value, failure: bool) -> Result<(), StorageError> {
        if let Some(limit) = self.daemon_limit {
            use crate::daemon::protocol::{DaemonResponse, DaemonRpcError, ERR_PARTIALLY_APPLIED};
            // The actual RPC ID is a u64. Reserve its longest encoding before
            // mutation, including the escaping of the complete error JSON.
            let response = if failure {
                DaemonResponse::error(
                    u64::MAX,
                    DaemonRpcError {
                        code: ERR_PARTIALLY_APPLIED,
                        message: value.to_string(),
                    },
                )
            } else {
                DaemonResponse::success(u64::MAX, value.clone())
            };
            check_encoded(&response, limit)?;
        }
        let (value, limit) = self.wire_value(value, failure)?;
        check_encoded(&value, limit)
    }

    fn wire_value(&self, value: &Value, failure: bool) -> Result<(Value, usize), StorageError> {
        Ok(match &self.surface {
            Surface::Http => (value.clone(), self.limit),
            Surface::Mcp { id, era, .. } => {
                // BridgeError::PartiallyApplied carries compact JSON text.
                // The tool handler parses that text before making its pretty
                // tool body. Reproduce this parse: f64 decimal decoding need
                // not retain the same representation on every serde build.
                let parsed;
                let value = if failure {
                    parsed = serde_json::from_str::<Value>(&value.to_string())
                        .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                    &parsed
                } else {
                    value
                };
                let mut tool =
                    ToolCallResult::json(value).map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                if failure {
                    tool.is_error = Some(true);
                }
                if *era == Era::Legacy {
                    tool.structured_content = None;
                }
                let tool =
                    serde_json::to_value(tool).map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                let env = serde_json::to_value(ResultEnvelope::complete(tool))
                    .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                let response = serde_json::to_value(JsonRpcResponse::success(id.clone(), env))
                    .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
                (
                    response,
                    self.limit
                        .checked_sub(1)
                        .ok_or(StorageError::ReceiptBudgetExceeded)?,
                )
            }
        })
    }

    #[cfg(test)]
    pub(crate) fn encoded_for_test(&self, value: &Value, failure: bool) -> Vec<u8> {
        let (value, _) = self.wire_value(value, failure).unwrap();
        let mut bytes = serde_json::to_vec(&value).unwrap();
        if matches!(self.surface, Surface::Mcp { .. }) {
            bytes.push(b'\n');
        }
        bytes
    }
}

fn check_encoded(value: &impl serde::Serialize, limit: usize) -> Result<(), StorageError> {
    struct Count {
        remaining: usize,
    }
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.remaining = self
                .remaining
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("catalog response limit"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Count { remaining: limit }, value)
        .map_err(|_| StorageError::ReceiptBudgetExceeded)
}

pub(crate) fn namespace(receipt: &WriteReceipt) -> Result<NamespaceConfig, StorageError> {
    if let MutationOutcome::NativeNamespaceCreated { transition } = &receipt.outcome {
        transition.validate(receipt)?;
        transition.namespace()
    } else {
        Err(StorageError::InvalidControl(
            "unexpected native catalog receipt".into(),
        ))
    }
}

pub(crate) fn committed_namespace(receipt: WriteReceipt) -> Result<NamespaceConfig, CatalogError> {
    namespace(&receipt).map_err(|source| {
        CatalogError::Storage(StorageError::CommittedMutation {
            receipt: Box::new(receipt),
            source: Box::new(source),
        })
    })
}

pub(crate) fn input(
    input: crate::mcp::bridge::CreateNamespaceInput,
) -> crate::storage::native_catalog::NativeNamespaceInput {
    crate::storage::native_catalog::NativeNamespaceInput {
        name: input.name,
        embedding_dim: input.embedding_dim.map(u32::from),
        embedding_profile_id: None,
        initial_stability: input.initial_stability,
        desired_retention: input.desired_retention,
        decay_rate_multiplier: input.decay_rate_multiplier,
    }
}

pub(crate) fn info(
    ns: NamespaceConfig,
    timezone: chrono_tz::Tz,
) -> crate::mcp::bridge::NamespaceInfo {
    crate::mcp::bridge::NamespaceInfo {
        id: ns.id.get(),
        name: ns.name,
        embedding_dim: ns.embedding_dim as u16,
        memory_count: 0,
        created_at: crate::time::format_timestamp(ns.created_at, timezone),
        policy: None,
    }
}

pub(crate) fn http_namespace(ns: NamespaceConfig) -> super::NamespaceResponse {
    super::NamespaceResponse {
        id: ns.id.get(),
        name: ns.name,
        embedding_dim: ns.embedding_dim,
        initial_stability: ns.initial_stability,
        default_difficulty: ns.default_difficulty,
        permastore_threshold: ns.permastore_threshold,
        desired_retention: ns.desired_retention,
        created_at: ns.created_at,
        memory_count: 0,
    }
}

fn evidence(
    code: &str,
    state: &str,
    operation: Option<Uuid>,
    receipt: Option<&WriteReceipt>,
    ready: Option<bool>,
    message: &str,
) -> Value {
    let mut value = json!({"error": code, "commitState": state, "operationId": operation,
        "message": message});
    if let Some(receipt) = receipt {
        value["receipt"] = json!(receipt);
    }
    if let Some(ready) = ready {
        value["ready"] = json!(ready);
    }
    value
}

/// Unknown takes precedence over separately retained known evidence. Generic
/// committed failures do not establish any derived readiness observation.
pub(crate) fn failure(error: &CatalogError) -> (u16, Value) {
    match try_failure(error) {
        Ok(value) => value,
        Err(invalid) => internal_failure(invalid),
    }
}

pub(crate) fn internal_failure(invalid: &CatalogWorkerEvidence) -> (u16, Value) {
    // Keep the full typed cause until this final endpoint boundary. Invalid
    // evidence is not a normally admitted receipt and must not become an
    // unbounded Debug payload on the wire.
    tracing::error!(evidence = ?invalid, "catalog worker evidence invariant violated");
    (
        500,
        evidence(
            "INTERNAL_ERROR",
            "unknown",
            None,
            None,
            None,
            "Catalog evidence invariant violated; outcome unknown. Reconcile before retry.",
        ),
    )
}

pub(crate) fn try_failure(error: &CatalogError) -> Result<(u16, Value), &CatalogWorkerEvidence> {
    if let CatalogError::Api1WorkerUnknown(evidence) = error {
        let (operation, receipt) = evidence.checked()?;
        return Ok((
            503,
            self::evidence(
                "SERVICE_UNAVAILABLE",
                "unknown",
                operation,
                receipt,
                None,
                "Catalog worker outcome unknown; reconcile retained evidence before retry.",
            ),
        ));
    }
    let (mut receipt, mut source) = match error {
        CatalogError::Storage(source) => (None, Some(source)),
        _ => (None, None),
    };
    loop {
        match source {
            Some(StorageError::CommittedMutation {
                receipt: known,
                source: cause,
            }) => {
                if receipt.is_none() {
                    receipt = Some(known.as_ref());
                }
                source = Some(cause.as_ref());
            }
            Some(StorageError::CommittedEffectsPending { source: cause, .. }) => {
                source = Some(cause.as_ref())
            }
            _ => break,
        }
    }
    let unknown = match source {
        Some(StorageError::CommitOutcomeUnknown { operation_id, .. }) => Some(Some(*operation_id)),
        Some(StorageError::WorkerOutcomeUnknown(_)) => Some(None),
        _ if matches!(error, CatalogError::WorkerUnknown) => Some(None),
        _ => None,
    };
    let (status, code, state, operation, ready) = if let Some(operation) = unknown {
        (503, "SERVICE_UNAVAILABLE", "unknown", operation, None)
    } else if let Some(receipt) = receipt {
        (
            503,
            "SERVICE_UNAVAILABLE",
            "committed",
            Some(receipt.operation_id),
            None,
        )
    } else if matches!(source, Some(StorageError::ReceiptBudgetExceeded)) {
        (413, "RESPONSE_TOO_LARGE", "notCommitted", None, None)
    } else if matches!(source, Some(StorageError::DuplicateName(_))) {
        (409, "NAMESPACE_EXISTS", "notCommitted", None, None)
    } else if matches!(
        source,
        Some(
            StorageError::InvalidMutation(_)
                | StorageError::InvalidMutationContext(_)
                | StorageError::InvalidNamespaceName { .. }
                | StorageError::NamespaceEmbeddingProfileMismatch { .. }
        )
    ) || matches!(error, CatalogError::Input(_))
    {
        (400, "INVALID_INPUT", "notCommitted", None, None)
    } else {
        (503, "SERVICE_UNAVAILABLE", "notCommitted", None, None)
    };
    let message = source
        .map(ToString::to_string)
        .unwrap_or_else(|| error.to_string());
    let message = crate::model::validation::truncate_on_char_boundary(&message, 512);
    Ok((
        status,
        evidence(code, state, operation, receipt, ready, message),
    ))
}

pub(crate) fn outcome_unknown(error: &CatalogError) -> bool {
    fn storage(error: &StorageError) -> bool {
        match error {
            StorageError::CommitOutcomeUnknown { .. } | StorageError::WorkerOutcomeUnknown(_) => {
                true
            }
            StorageError::CommittedMutation { source, .. }
            | StorageError::CommittedEffectsPending { source, .. } => storage(source),
            _ => false,
        }
    }
    match error {
        CatalogError::WorkerUnknown | CatalogError::Api1WorkerUnknown(_) => true,
        CatalogError::Storage(source) => storage(source),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_catalog_adapter::{
        NativeCatalogAdapter,
        tests::{FixedClock, fixture},
    };

    #[test]
    fn daemon_caller_roundtrip_caps_limits_and_preserves_mcp_null_id_and_eras() {
        for era in [Era::Legacy, Era::Modern] {
            for id in [
                JsonRpcId::Null,
                JsonRpcId::Number(i64::MIN),
                JsonRpcId::String("id\n\"\\λ".into()),
            ] {
                let original = CatalogReplyAdmission::mcp(id, era)
                    .with_timezone(chrono_tz::America::Los_Angeles);
                let mut caller = serde_json::to_value(original.caller()).unwrap();
                caller["limit"] = json!(usize::MAX);
                let restored = CatalogReplyAdmission::from_caller(
                    serde_json::from_value(caller).unwrap(),
                    usize::MAX,
                )
                .unwrap();
                assert_eq!(restored.limit, LOCAL_LIMIT);
                assert_eq!(
                    restored.daemon_limit,
                    Some(crate::daemon::protocol::MAX_MESSAGE_SIZE)
                );
                assert_eq!(
                    original.encoded_for_test(&json!({"name":"example"}), false),
                    restored.encoded_for_test(&json!({"name":"example"}), false)
                );
            }
        }
        let mut caller = serde_json::to_value(CatalogReplyAdmission::http().caller()).unwrap();
        caller["timezone"] = json!("invalid timezone");
        assert!(
            CatalogReplyAdmission::from_caller(
                serde_json::from_value(caller).unwrap(),
                LOCAL_LIMIT
            )
            .is_err()
        );
    }

    #[test]
    fn daemon_and_caller_envelopes_each_enforce_exact_byte_boundary() {
        use crate::daemon::protocol::{DaemonResponse, DaemonRpcError, ERR_PARTIALLY_APPLIED};
        for era in [Era::Legacy, Era::Modern] {
            for failure in [false, true] {
                let original =
                    CatalogReplyAdmission::mcp(JsonRpcId::String("\"\\\n".repeat(1000)), era);
                let value = json!({"receipt":{"name":"\0\"\\λ".repeat(100)},"message":"failure"});
                let mut admission =
                    CatalogReplyAdmission::from_caller(original.caller(), LOCAL_LIMIT).unwrap();
                let caller_len = original.encoded_for_test(&value, failure).len();
                admission.limit = caller_len;
                admission.check_value(&value, failure).unwrap();
                admission.limit -= 1;
                assert!(admission.check_value(&value, failure).is_err());
                admission.limit = LOCAL_LIMIT;
                let response = if failure {
                    DaemonResponse::error(
                        u64::MAX,
                        DaemonRpcError {
                            code: ERR_PARTIALLY_APPLIED,
                            message: value.to_string(),
                        },
                    )
                } else {
                    DaemonResponse::success(u64::MAX, value.clone())
                };
                let frame_len = serde_json::to_vec(&response).unwrap().len();
                admission.daemon_limit = Some(frame_len);
                admission.check_value(&value, failure).unwrap();
                admission.daemon_limit = Some(frame_len - 1);
                assert!(admission.check_value(&value, failure).is_err());
            }
        }
    }

    struct CountClock(std::sync::atomic::AtomicUsize);
    impl crate::time::Clock for CountClock {
        fn now_millis(&self) -> i64 {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            2000
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_catalog_api1_real_candidate_budget_rolls_back_before_namespace_stage() {
        use crate::storage::{
            StorageEngine,
            ids::{IdKind, IdSource, SeededIds},
        };
        let (f, path, provider) = crate::native_catalog_adapter::tests::api1_seeded_fixture();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let ids = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .id_source()
            .checkpoint()
            .unwrap()
            .unwrap();
        let oracle = SeededIds::from_state(ids).unwrap();
        let rejected_id = oracle.uuid(IdKind::Operation).unwrap();
        let clock = std::sync::Arc::new(CountClock(std::sync::atomic::AtomicUsize::new(0)));
        let adapter =
            NativeCatalogAdapter::new(f.completion(), provider, Default::default(), clock.clone());
        let mut admission =
            CatalogReplyAdmission::mcp(JsonRpcId::String("x".repeat(100_000)), Era::Modern);
        // Find the exact minimum envelope allowance. Its collector has ample
        // space for a creation receipt, but the final outer-ID frame does not.
        let (mut low, mut high) = (0, LOCAL_LIMIT);
        while low < high {
            let middle = low + (high - low) / 2;
            admission.limit = middle;
            if admission.admit_native().is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        admission.limit = low;
        admission.admit_native().unwrap();
        let input = input(crate::mcp::bridge::CreateNamespaceInput {
            name: "candidate".into(),
            embedding_dim: None,
            initial_stability: Some(3.7145),
            desired_retention: None,
            decay_rate_multiplier: None,
        });
        let error = adapter
            .create_api1_bounded(input.clone(), admission)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            CatalogError::Storage(StorageError::ReceiptBudgetExceeded)
        ));
        assert_eq!(clock.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        {
            let engine = f.storage.read().unwrap();
            assert_eq!(engine.control_state().unwrap(), before);
            assert!(engine.get_namespace_by_name("candidate").unwrap().is_none());
            assert!(
                engine
                    .meta_store()
                    .lookup_operation(
                        before.store_id,
                        crate::storage::control::MutationScope::Catalog,
                        rejected_id
                    )
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                engine.meta_store().id_source().checkpoint().unwrap(),
                oracle.checkpoint().unwrap()
            );
        }
        assert!(!path.join("candidate").exists());
        assert!(std::fs::read_dir(&path).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".recalld-catalog-")
        }));
        let receipt = adapter
            .create_api1_bounded(input, CatalogReplyAdmission::http())
            .await
            .unwrap();
        assert_eq!(
            receipt.operation_id,
            oracle.uuid(IdKind::Operation).unwrap()
        );
        assert_eq!(
            receipt.committed_generation,
            before.committed_generation.next().unwrap()
        );
        assert_eq!(namespace(&receipt).unwrap().id.get(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_catalog_api1_full_encoding_limits_and_unknown_over_known() {
        let (f, _, provider) = fixture();
        let adapter = NativeCatalogAdapter::new(
            f.completion(),
            provider,
            Default::default(),
            std::sync::Arc::new(FixedClock),
        );
        let receipt = adapter
            .create_api1_bounded(
                input(crate::mcp::bridge::CreateNamespaceInput {
                    name: "encoded".into(),
                    embedding_dim: None,
                    initial_stability: Some(3.7145),
                    desired_retention: Some(0.9),
                    decay_rate_multiplier: None,
                }),
                CatalogReplyAdmission::http(),
            )
            .await
            .unwrap();
        let error = CatalogError::Storage(StorageError::CommittedMutation {
            receipt: Box::new(receipt.clone()),
            source: Box::new(StorageError::WorkerOutcomeUnknown("worker lost".into())),
        });
        assert!(outcome_unknown(&error));
        let (_, value) = failure(&error);
        assert_eq!(value["commitState"], "unknown");
        assert_eq!(value["operationId"], Value::Null);
        assert_eq!(value["receipt"], json!(receipt));
        assert!(value.get("ready").is_none());
        let value = evidence(
            "SERVICE_UNAVAILABLE",
            "unknown",
            Some(Uuid::from_u128(u128::MAX)),
            Some(&receipt),
            None,
            &"\0".repeat(512),
        );
        for era in [Era::Legacy, Era::Modern] {
            let id = JsonRpcId::String("\0\"\\é".repeat(64));
            let parsed: Value = serde_json::from_str(&value.to_string()).unwrap();
            let mut tool = ToolCallResult::json(&parsed).unwrap();
            tool.is_error = Some(true);
            if era == Era::Legacy {
                tool.structured_content = None;
            }
            let response = JsonRpcResponse::success(
                id.clone(),
                serde_json::to_value(ResultEnvelope::complete(
                    serde_json::to_value(tool).unwrap(),
                ))
                .unwrap(),
            );
            let size = serde_json::to_vec(&response).unwrap().len() + 1;
            let mut admission = CatalogReplyAdmission::mcp(id, era);
            admission.limit = size;
            admission.check_value(&value, true).unwrap();
            admission.limit -= 1;
            assert!(matches!(
                admission.check_value(&value, true),
                Err(StorageError::ReceiptBudgetExceeded)
            ));
        }
        let size = serde_json::to_vec(&value).unwrap().len();
        let mut admission = CatalogReplyAdmission::http();
        admission.limit = size;
        admission.check_value(&value, true).unwrap();
        admission.limit -= 1;
        assert!(matches!(
            admission.check_value(&value, true),
            Err(StorageError::ReceiptBudgetExceeded)
        ));
    }
}
