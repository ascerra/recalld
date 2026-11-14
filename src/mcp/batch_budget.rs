//! Budget full receipts before executing native wire batches. This does not
//! alter canonical link settings, hashes, item order or retained outcomes.
use crate::storage::{
    StorageError,
    batch::{BatchItemPlan, BatchItemState, BatchProgress},
    control::WriteReceipt,
    native_graph::MAX_NATIVE_GRAPH_EDGES,
    native_store::{NativeStoreInput, NativeStoreSettings},
};
use std::io::Write;

// A JSON value can expand again inside MCP text and also appears as structured
// content. Leave frame/envelope space and a conservative factor of four.
pub(crate) const MAX_BATCH_JSON_BYTES: usize =
    (crate::daemon::protocol::MAX_MESSAGE_SIZE - 64 * 1024) / 4;
const ENVELOPE_BYTES: usize = 16 * 1024;
// Includes the response-only advisory's three 240-byte summaries with JSON
// escaping, bounded field diagnostics, stored result and per-item framing.
const ITEM_BYTES: usize = 8 * 1024;
// NativeRecordChange has at most two fixed numeric/hash states: no original
// summary, text, labels or profile descriptor. Profile ID is fixed 68 bytes.
const RECORD_CHANGE_BYTES: usize = 8 * 1024;
const EDGE_BYTES: usize = 512;
const RECEIPT_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy)]
pub(crate) struct BatchBudget {
    pub(crate) limit: usize,
}
impl BatchBudget {
    pub(crate) const STANDARD: Self = Self {
        limit: MAX_BATCH_JSON_BYTES,
    };
    fn error(receipt: Option<&WriteReceipt>) -> StorageError {
        StorageError::BatchResponseTooLarge {
            reservation_receipt: receipt.cloned().map(Box::new),
        }
    }
    fn charge(
        self,
        used: &mut usize,
        amount: usize,
        receipt: Option<&WriteReceipt>,
    ) -> Result<(), StorageError> {
        *used = used
            .checked_add(amount)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| Self::error(receipt))?;
        Ok(())
    }
    fn receipt_bound(input: &NativeStoreInput, settings: &NativeStoreSettings) -> usize {
        let explicit = usize::from(input.insert.memory.parent_id.is_some())
            + usize::from(input.insert.memory.supersedes.is_some());
        let automatic = if settings.autolink_enabled {
            u64::from(settings.max_auto_links)
                + u64::from(settings.max_entity_links)
                + if settings.temporal_window_ms == 0 {
                    0
                } else {
                    u64::from(settings.max_temporal_links)
                }
        } else {
            0
        };
        let edges = (automatic + explicit as u64).min(MAX_NATIVE_GRAPH_EDGES as u64) as usize;
        let records = 1
            + edges
            + if input.check_duplicates {
                crate::model::constants::MAX_NEAR_DUPLICATE_MATCHES
            } else {
                0
            };
        RECEIPT_BYTES + records * RECORD_CHANGE_BYTES + edges * EDGE_BYTES
    }
    pub(crate) fn fresh(
        self,
        plans: &[BatchItemPlan],
        settings: &NativeStoreSettings,
    ) -> Result<(), StorageError> {
        // Reserve additional parent receipt/bookkeeping framing before commit.
        let mut used = 2 * ENVELOPE_BYTES;
        for plan in plans {
            self.charge(&mut used, ITEM_BYTES, None)?;
            if let BatchItemPlan::NativeStore { input, .. } = plan {
                self.charge(&mut used, Self::receipt_bound(input, settings), None)?;
            }
        }
        Ok(())
    }
    pub(crate) fn retained(
        self,
        p: &BatchProgress,
        plans: &[BatchItemPlan],
    ) -> Result<(), StorageError> {
        let parent = Some(&p.reservation_receipt);
        let mut used = ENVELOPE_BYTES;
        self.encoded(&mut used, &p.reservation_receipt, parent)?;
        self.encoded(&mut used, &p.bookkeeping_receipts, parent)?;
        if p.items.len() != plans.len() {
            return Err(StorageError::InvalidControl(
                "batch budget plan count".into(),
            ));
        }
        for (item, plan) in p.items.iter().zip(plans) {
            self.charge(&mut used, ITEM_BYTES, parent)?;
            match &item.state {
                BatchItemState::Ok { receipt } => self.encoded(&mut used, receipt, parent)?,
                BatchItemState::Error { .. } => {}
                BatchItemState::NotAttempted {} => {
                    let BatchItemPlan::NativeStore { input, .. } = plan else {
                        // A formerly accepted item that current validation rejects
                        // can only acquire bounded rejection bookkeeping.
                        continue;
                    };
                    let settings = item
                        .defaults
                        .as_ref()
                        .and_then(|d| d.native.as_ref())
                        .and_then(|n| n.store_settings.as_ref())
                        .ok_or_else(|| {
                            StorageError::InvalidControl(
                                "pending native batch settings missing".into(),
                            )
                        })?;
                    self.charge(&mut used, Self::receipt_bound(input, settings), parent)?;
                }
            }
        }
        Ok(())
    }
    fn encoded(
        self,
        used: &mut usize,
        value: &impl serde::Serialize,
        receipt: Option<&WriteReceipt>,
    ) -> Result<(), StorageError> {
        let mut counter = Counter {
            used: *used,
            limit: self.limit,
        };
        serde_json::to_writer(&mut counter, value).map_err(|_| Self::error(receipt))?;
        *used = counter.used;
        Ok(())
    }
}
struct Counter {
    used: usize,
    limit: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.used = self
            .used
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::control::MutationOutcome;

    #[test]
    fn batch_wire_budget_exact_boundaries_and_complete_transport_framing() {
        let raw = include_bytes!("../../tests/fixtures/native-batch-before106/v3-manifest.json");
        let mut p: BatchProgress = serde_json::from_slice(raw).unwrap();
        let request: crate::storage::batch::BatchInput = serde_json::from_slice(include_bytes!(
            "../../tests/fixtures/native-batch-before106/v3-request.json"
        ))
        .unwrap();
        let plans = crate::mcp::batches::prepare_native_store(&request)
            .unwrap()
            .into_iter()
            .map(|i| i.plan)
            .collect::<Vec<_>>();
        let BatchItemPlan::NativeStore { input, .. } = &plans[0] else {
            panic!()
        };
        let settings = p.items[0]
            .defaults
            .as_ref()
            .unwrap()
            .native
            .as_ref()
            .unwrap()
            .store_settings
            .as_ref()
            .unwrap();
        let fresh = 2 * ENVELOPE_BYTES + ITEM_BYTES + BatchBudget::receipt_bound(input, settings);
        BatchBudget { limit: fresh }
            .fresh(&plans, settings)
            .unwrap();
        assert!(matches!(
            BatchBudget { limit: fresh - 1 }.fresh(&plans, settings),
            Err(StorageError::BatchResponseTooLarge {
                reservation_receipt: None
            })
        ));
        let BatchItemState::Ok { receipt } = &p.items[0].state else {
            panic!()
        };
        let actual = ENVELOPE_BYTES
            + ITEM_BYTES
            + serde_json::to_vec(&p.reservation_receipt).unwrap().len()
            + serde_json::to_vec(&p.bookkeeping_receipts).unwrap().len()
            + serde_json::to_vec(receipt).unwrap().len();
        BatchBudget { limit: actual }.retained(&p, &plans).unwrap();
        assert!(matches!(
            BatchBudget { limit: actual - 1 }.retained(&p, &plans),
            Err(StorageError::BatchResponseTooLarge {
                reservation_receipt: Some(_)
            })
        ));
        let mut receipt = receipt.clone();
        let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &mut receipt.outcome
        else {
            panic!()
        };
        // Synthetic typed serializer stress, not authored canonical state. Max
        // hashes/timestamps/optional fields exercise more than one-hot receipts.
        let mut record = transition.records[0].clone();
        let s = &mut record.after;
        s.edge_count = u16::MAX;
        s.history_sha256 = [255; 32];
        s.content.summary_sha256 = [255; 32];
        s.content.tags_sha256 = [255; 32];
        s.content.labels_sha256 = [255; 32];
        s.content.extensions_sha256 = [255; 32];
        s.content.text_sha256 = Some([255; 32]);
        s.content.profile_id = Some(format!("ep1:{}", "f".repeat(64)));
        s.lifecycle.created_at = i64::MIN;
        s.lifecycle.last_accessed_at = i64::MIN;
        s.lifecycle.stability = f32::MAX.try_into().unwrap();
        s.lifecycle.difficulty = f32::MIN_POSITIVE.try_into().unwrap();
        s.lifecycle.strength = (-f32::MAX).try_into().unwrap();
        s.lifecycle.decay_strength = f32::MIN_POSITIVE.try_into().unwrap();
        let l = &mut s.lifecycle.lifecycle;
        l.observed_at = i64::MIN;
        l.retrieved_at = Some(i64::MIN);
        l.reinforced_at = Some(i64::MIN);
        l.decay_anchor_at = i64::MIN;
        l.lifecycle_updated_at = i64::MIN;
        l.phase_changed_at = i64::MIN;
        l.policy_revision = "9223372036854775807".to_owned().try_into().unwrap();
        l.record_generation = l.policy_revision;
        record.before = Some(record.after.clone());
        assert!(serde_json::to_vec(&record).unwrap().len() < RECORD_CHANGE_BYTES);
        let edge = crate::storage::native_records::NativeEdgeProof {
            source: record.memory_id,
            target: record.memory_id,
            edge_type: crate::model::EdgeType::Temporal,
            weight_bits: u32::MAX,
            auto_created: false,
            created_at: u64::MAX,
        };
        assert!(serde_json::to_vec(&edge).unwrap().len() < EDGE_BYTES);
        // Largest conservatively admitted single receipt, plus worst advisory
        // string escaping and the complete modern MCP/daemon/REST framing.
        let count = (MAX_BATCH_JSON_BYTES - 2 * ENVELOPE_BYTES - ITEM_BYTES - RECEIPT_BYTES)
            / (RECORD_CHANGE_BYTES + EDGE_BYTES);
        transition.records = vec![record; count];
        transition.edges = vec![edge; count.saturating_sub(1)];
        p.items[0].state = BatchItemState::Ok {
            receipt: receipt.clone(),
        };
        BatchBudget::STANDARD.retained(&p, &plans).unwrap();
        let item = crate::storage::batch::BatchItemOutcome {
            index: 0,
            status: crate::storage::batch::BatchStatus::Ok,
            operation_id: Some(receipt.operation_id),
            namespace_id: Some(crate::model::NamespaceId::new(1)),
            receipt: Some(receipt),
            error: None,
            ready: true,
            retryable: false,
            result: Some(
                serde_json::json!({"nearDuplicates":[{"summary":"\u{0001}".repeat(240)},{"summary":"\"".repeat(240)},{"summary":"\\".repeat(240)}]}),
            ),
        };
        let outcome = crate::storage::batch::BatchOutcome {
            result_version: 1,
            batch_operation_id: p.reservation_receipt.operation_id,
            store_id: p.reservation_receipt.store_id,
            reservation_state: crate::storage::batch::BatchReservationState::Committed,
            reservation_receipt: Some(p.reservation_receipt),
            bookkeeping_receipts: vec![],
            unknown_bookkeeping_operations: vec![],
            items: vec![item],
            total: 1,
            succeeded: 1,
            failed: 0,
            unresolved: 0,
        };
        let reply = crate::serialization::native_batch::NativeBatchReply::Outcome(outcome.clone());
        let daemon = crate::daemon::protocol::DaemonResponse::success(
            u64::MAX,
            serde_json::to_value(reply).unwrap(),
        );
        let (status, envelope) = crate::serialization::native_batch::response(Ok(outcome));
        assert_eq!(status, 200);
        let mcp = crate::serialization::native_batch::mcp_result(status, &envelope);
        for value in [
            serde_json::to_vec(&envelope).unwrap(),
            serde_json::to_vec(&daemon).unwrap(),
            serde_json::to_vec(&serde_json::json!({"jsonrpc":"2.0","id":u64::MAX,"result":mcp}))
                .unwrap(),
        ] {
            assert!(
                value.len() < crate::daemon::protocol::MAX_MESSAGE_SIZE,
                "whole framed response {}",
                value.len()
            );
        }
    }
}
