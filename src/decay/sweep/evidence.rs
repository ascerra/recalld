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
    pub(crate) fn from_admission(
        admission: &ReceiptAdmission,
    ) -> Result<Option<Self>, crate::storage::maintenance::InvalidReceiptEvidence> {
        let (kind, receipts, failed_receipt) = admission.snapshot()?;
        let Some(kind) = kind else {
            return Ok(None);
        };
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
        Ok(Some(Self {
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
        }))
    }
}

impl std::fmt::Display for SweepFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for SweepFailure {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        StorageError,
        control::{Generation, MutationOutcome},
        tombstone::TombstoneStatus,
    };
    use axum::response::IntoResponse;

    fn receipt() -> WriteReceipt {
        // Legal payload-free legacy receipt shape; this is a serialization/
        // evidence test, not a claim that a storage mutation was executed.
        WriteReceipt {
            store_id: uuid::Uuid::from_u128(1),
            operation_id: uuid::Uuid::from_u128(2),
            committed_generation: Generation::MAX,
            receipt_version: 1,
            replayed: false,
            outcome: MutationOutcome::MemoryTombstoned {
                memory_id: crate::model::MemoryId::from_bytes([3; 16]),
                status: TombstoneStatus::Tombstoned,
            },
        }
    }
    fn unknown(id: uuid::Uuid) -> StorageError {
        StorageError::CommitOutcomeUnknown {
            operation_id: id,
            source: redb::CommitError::Storage(redb::StorageError::Io(std::io::Error::other(
                "unit lost ACK cause",
            ))),
        }
    }

    #[test]
    fn unknown_identity_survives_conflicting_full_receipts_and_secondary_worker_loss() {
        let a = ReceiptAdmission::new(16_384).unwrap();
        let first = receipt();
        a.record(&first).unwrap();
        let id = uuid::Uuid::from_u128(42);
        a.fail(&unknown(id));
        let mut conflicting = first.clone();
        conflicting.replayed = true;
        assert!(matches!(
            a.record(&conflicting),
            Err(StorageError::ReceiptEvidenceConflict)
        ));
        a.fail(&StorageError::WorkerOutcomeUnknown(
            "secondary worker".into(),
        ));
        let body = SweepFailure::from_admission(&a).unwrap().unwrap();
        assert_eq!(body.commit_state, "unknown");
        assert_eq!(body.operation_id, Some(id));
        assert_eq!(body.receipts, vec![first, conflicting]);
        assert!(body.receipt.is_none());
    }

    #[test]
    fn unadmitted_known_wrapper_cannot_project_not_committed_or_drop_owned_evidence() {
        for kind in 0..4 {
            let a = ReceiptAdmission::new(2048).unwrap();
            let receipt = receipt();
            let id = uuid::Uuid::from_u128(42);
            let source = match kind {
                0 => StorageError::ReceiptBudgetExceeded,
                1 => StorageError::MaintenanceDependencyUnavailable,
                2 => StorageError::RecoveryRequired("known publication failure".into()),
                _ => unknown(id),
            };
            a.fail(&StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(source),
            });
            let invalid = SweepFailure::from_admission(&a).unwrap_err();
            assert!(invalid.prior.is_empty());
            assert_eq!(invalid.unadmitted, vec![receipt.clone()]);
            assert_eq!(invalid.failed_receipt, Some(receipt));
            assert_eq!(
                invalid.unknown,
                if kind == 3 { Some(Some(id)) } else { None }
            );
            // No ordinary SweepFailure exists to serialize a false rollback or
            // omit its known receipt. Actual writers must make this unreachable.
        }
    }

    #[tokio::test]
    async fn every_failure_projection_uses_exact_typed_cost_and_bounded_fixed_overhead() {
        for prior in [false, true] {
            for kind in 0..7 {
                let a = ReceiptAdmission::new(16_384).unwrap();
                let receipt = receipt();
                if prior {
                    a.record(&receipt).unwrap();
                }
                let error = match kind {
                    0 => StorageError::RecoveryRequired(
                        "unbounded raw diagnostic must not be emitted".repeat(1000),
                    ),
                    1 => StorageError::MaintenanceDependencyUnavailable,
                    2 => StorageError::ReceiptBudgetExceeded,
                    3 => StorageError::ReceiptEvidenceConflict,
                    4 => StorageError::CommittedMutation {
                        receipt: Box::new(receipt),
                        source: Box::new(StorageError::RecoveryRequired(
                            "raw nested receipt".into(),
                        )),
                    },
                    5 => unknown(uuid::Uuid::from_u128(u128::MAX)),
                    _ => StorageError::WorkerOutcomeUnknown("worker".into()),
                };
                a.fail(&error);
                let body = SweepFailure::from_admission(&a).unwrap().unwrap();
                let payload: usize = body
                    .receipts
                    .iter()
                    .map(|r| serde_json::to_vec(r).unwrap().len())
                    .sum();
                let alias = body
                    .receipt
                    .as_ref()
                    .map_or(0, |r| serde_json::to_vec(r).unwrap().len());
                let charge = 2048 + 2 * (payload + body.receipts.len());
                let expected = serde_json::to_vec(&body).unwrap();
                let response = crate::api::AppError::Sweep(body).into_response();
                assert_eq!(
                    response.status(),
                    axum::http::StatusCode::SERVICE_UNAVAILABLE
                );
                let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap();
                assert_eq!(bytes.as_ref(), expected);
                assert!(bytes.len() <= charge);
                assert!(bytes.len() - payload - alias <= 2048);
                assert!(!String::from_utf8_lossy(&bytes).contains("raw diagnostic"));
            }
        }
    }
}
