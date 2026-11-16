//! Ephemeral authority for one bounded maintenance pass, never durable state.
use crate::storage::{StorageError, control::WriteReceipt};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub(crate) const FIXED_OVERHEAD: usize = 2048;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReceiptSizeLimit(pub usize);

impl ReceiptSizeLimit {
    pub(crate) fn encode(self, receipt: &WriteReceipt) -> Result<Vec<u8>, StorageError> {
        struct Capped {
            bytes: Vec<u8>,
            limit: usize,
        }
        impl Write for Capped {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                    return Err(std::io::Error::other("receipt limit"));
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut out = Capped {
            bytes: Vec::new(),
            limit: self.0,
        };
        serde_json::to_writer(&mut out, receipt)
            .map_err(|_| StorageError::ReceiptBudgetExceeded)?;
        Ok(out.bytes)
    }

    pub(crate) fn check(self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.encode(receipt).map(|_| ())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailureKind {
    Unavailable,
    Known,
    Dependency,
    Budget,
    Conflict,
    Unknown(Option<Uuid>),
}

#[derive(Debug)]
struct Evidence {
    cap: usize,
    charged: usize,
    entries: Vec<(WriteReceipt, Vec<u8>)>,
    by_id: HashMap<(Uuid, Uuid), Vec<usize>>,
    operations: HashSet<Uuid>,
    failure: Option<FailureKind>,
    failed_receipt: Option<WriteReceipt>,
    unadmitted: Vec<WriteReceipt>,
}

/// Internal admission violation; never project it as an ordinary small failure.
#[derive(Debug, thiserror::Error)]
#[error("maintenance receipt admission invariant violated")]
pub(crate) struct InvalidReceiptEvidence {
    pub(crate) prior: Vec<WriteReceipt>,
    pub(crate) unadmitted: Vec<WriteReceipt>,
    pub(crate) failed_receipt: Option<WriteReceipt>,
    pub(crate) unknown: Option<Option<Uuid>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiptAdmission(Arc<Mutex<Evidence>>);

impl ReceiptAdmission {
    pub(crate) fn new(cap: usize) -> Result<Self, StorageError> {
        if cap < FIXED_OVERHEAD {
            return Err(StorageError::ReceiptBudgetExceeded);
        }
        Ok(Self(Arc::new(Mutex::new(Evidence {
            cap,
            charged: FIXED_OVERHEAD,
            entries: Vec::new(),
            by_id: HashMap::new(),
            operations: HashSet::new(),
            failure: None,
            failed_receipt: None,
            unadmitted: Vec::new(),
        }))))
    }

    pub(crate) fn limit(&self) -> Result<ReceiptSizeLimit, StorageError> {
        let evidence = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if evidence.failure.is_some() {
            return Err(StorageError::MaintenanceDependencyUnavailable);
        }
        let remaining = ((evidence.cap - evidence.charged) / 2)
            .checked_sub(1)
            .ok_or(StorageError::ReceiptBudgetExceeded)?;
        Ok(ReceiptSizeLimit(remaining))
    }

    pub(crate) fn stopped(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .failure
            .is_some()
    }

    pub(crate) fn unknown(&self) -> bool {
        matches!(
            self.0.lock().unwrap_or_else(|e| e.into_inner()).failure,
            Some(FailureKind::Unknown(_) | FailureKind::Conflict)
        )
    }

    pub(crate) fn permits_operation(&self, key: &[u8]) -> bool {
        Uuid::from_slice(key).is_ok_and(|id| {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .operations
                .contains(&id)
        })
    }

    pub(crate) fn check_receipt(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        let (cap, permitted) = {
            let evidence = self.0.lock().unwrap_or_else(|e| e.into_inner());
            (
                evidence.cap,
                evidence
                    .by_id
                    .contains_key(&(receipt.store_id, receipt.operation_id)),
            )
        };
        if !permitted {
            return Err(StorageError::MaintenanceDependencyUnavailable);
        }
        let encoded = ReceiptSizeLimit(cap).encode(receipt)?;
        let evidence = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if evidence
            .by_id
            .get(&(receipt.store_id, receipt.operation_id))
            .is_some_and(|indices| indices.iter().any(|i| evidence.entries[*i].1 == encoded))
        {
            Ok(())
        } else {
            Err(StorageError::MaintenanceDependencyUnavailable)
        }
    }

    /// Only actual returned receipts, never the transaction's private candidate.
    pub(crate) fn record(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        let cap = self.0.lock().unwrap_or_else(|e| e.into_inner()).cap;
        let bytes = ReceiptSizeLimit(cap).encode(receipt)?;
        let mut evidence = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let key = (receipt.store_id, receipt.operation_id);
        let conflict = if let Some(indices) = evidence.by_id.get(&key) {
            if indices.iter().any(|i| evidence.entries[*i].1 == bytes) {
                return Ok(());
            }
            true
        } else {
            false
        };
        let cost = bytes
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| evidence.charged.checked_add(n))
            .filter(|n| *n <= evidence.cap)
            .ok_or(StorageError::ReceiptBudgetExceeded)?;
        let index = evidence.entries.len();
        evidence.entries.push((receipt.clone(), bytes));
        evidence.by_id.entry(key).or_default().push(index);
        evidence.operations.insert(receipt.operation_id);
        evidence.charged = cost;
        if conflict {
            if !matches!(evidence.failure, Some(FailureKind::Unknown(_))) {
                evidence.failure = Some(FailureKind::Conflict);
            }
            return Err(StorageError::ReceiptEvidenceConflict);
        }
        Ok(())
    }

    pub(crate) fn fail(&self, error: &StorageError) {
        let mut leaf = error;
        let mut failed_receipt = None;
        let mut evidence_error = None;
        loop {
            match leaf {
                StorageError::CommittedMutation { receipt, source } => {
                    if failed_receipt.is_none() {
                        failed_receipt = Some((**receipt).clone());
                    }
                    if let Err(error) = self.record(receipt) {
                        if !matches!(error, StorageError::ReceiptEvidenceConflict) {
                            self.0
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .unadmitted
                                .push((**receipt).clone());
                        }
                        evidence_error = Some(error);
                    }
                    leaf = source;
                }
                StorageError::CommittedEffectsPending { source, .. } => leaf = source,
                _ => break,
            }
        }
        let kind = match leaf {
            StorageError::CommitOutcomeUnknown { operation_id, .. } => {
                FailureKind::Unknown(Some(*operation_id))
            }
            StorageError::WorkerOutcomeUnknown(_) => FailureKind::Unknown(None),
            StorageError::ReceiptBudgetExceeded => FailureKind::Budget,
            StorageError::MaintenanceDependencyUnavailable => FailureKind::Dependency,
            StorageError::ReceiptEvidenceConflict => FailureKind::Conflict,
            _ if evidence_error.is_some() => FailureKind::Conflict,
            _ if failed_receipt.is_some() => FailureKind::Known,
            _ => FailureKind::Unavailable,
        };
        let mut evidence = self.0.lock().unwrap_or_else(|e| e.into_inner());
        // A later secondary unavailable/dependency error cannot hide unknown.
        if evidence.failure.is_none()
            || (matches!(kind, FailureKind::Unknown(_))
                && !matches!(evidence.failure, Some(FailureKind::Unknown(Some(_)))))
        {
            evidence.failure = Some(kind);
        }
        if evidence.failed_receipt.is_none() {
            evidence.failed_receipt = failed_receipt;
        }
    }

    pub(crate) fn snapshot(
        &self,
    ) -> Result<
        (Option<FailureKind>, Vec<WriteReceipt>, Option<WriteReceipt>),
        InvalidReceiptEvidence,
    > {
        let evidence = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !evidence.unadmitted.is_empty() {
            return Err(InvalidReceiptEvidence {
                prior: evidence.entries.iter().map(|e| e.0.clone()).collect(),
                unadmitted: evidence.unadmitted.clone(),
                failed_receipt: evidence.failed_receipt.clone(),
                unknown: match evidence.failure {
                    Some(FailureKind::Unknown(id)) => Some(id),
                    _ => None,
                },
            });
        }
        Ok((
            evidence.failure,
            evidence.entries.iter().map(|e| e.0.clone()).collect(),
            evidence.failed_receipt.clone(),
        ))
    }
}
