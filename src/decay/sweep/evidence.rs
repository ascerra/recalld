//! Exact typed HTTP failure representation for the bounded manual consumer.
use crate::storage::control::WriteReceipt;
use crate::storage::maintenance::{FailureKind, ReceiptAdmission};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SweepFailure {
    error: &'static str,
    commit_state: &'static str,
    ready: bool,
    complete: bool,
    retryable: bool,
    receipts: Vec<WriteReceipt>,
    operation_id: Option<uuid::Uuid>,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<WriteReceipt>,
}

impl SweepFailure {
    pub(crate) fn from_admission(admission: &ReceiptAdmission) -> Option<Self> {
        let (kind, receipts, failed_receipt) = admission.snapshot();
        let kind = kind?;
        let (error, message) = match kind {
            FailureKind::Unknown(_) => (
                "COMMIT_OUTCOME_UNKNOWN",
                "Sweep outcome is unknown; reconcile before retry.",
            ),
            FailureKind::Conflict => (
                "SWEEP_EVIDENCE_CONFLICT",
                "Sweep evidence conflicts; reconcile before retry.",
            ),
            FailureKind::Budget => (
                "SWEEP_RESPONSE_LIMIT",
                "Sweep stopped at its response evidence limit.",
            ),
            FailureKind::Dependency => (
                "SWEEP_UNAVAILABLE",
                "Sweep stopped for pending work outside this pass.",
            ),
            FailureKind::Known => (
                "INDEX_NOT_READY",
                "Sweep changes committed but publication did not complete.",
            ),
            FailureKind::Unavailable => ("SWEEP_UNAVAILABLE", "Sweep could not complete."),
        };
        let unknown = matches!(kind, FailureKind::Unknown(_) | FailureKind::Conflict);
        Some(Self {
            error,
            commit_state: if unknown {
                "unknown"
            } else if receipts.is_empty() {
                "notCommitted"
            } else {
                "committed"
            },
            ready: false,
            complete: false,
            retryable: false,
            receipts,
            operation_id: match kind {
                FailureKind::Unknown(id) => id,
                _ => None,
            },
            message,
            receipt: if kind == FailureKind::Known {
                failed_receipt
            } else {
                None
            },
        })
    }
}

impl std::fmt::Display for SweepFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for SweepFailure {}
