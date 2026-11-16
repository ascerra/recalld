//! Fresh-request admission only. No persisted permit or alternate purge writer.
use super::*;
use crate::storage::PersistedEdge;
use crate::storage::control::{Generation, MutationOutcome, WriteReceipt};
use crate::storage::edges::{EdgeStore, FORWARD_EDGES};
use crate::storage::native_maintenance::{NativeDeletionWitness, NativePeerChange};
use crate::storage::native_purge::{NativePurgeAdvanced, NativePurgeChunk};
use crate::storage::native_records::{NativeEdgeProof, NativeRecordChange, NativeRecordState};
use crate::storage::purge::PurgeStage;
use std::io::Write;

// Bounds apply only to fresh native operations on a bounded response transport.
const MAX_EXAMINATIONS: usize = 100_000;
const MAX_SOURCE_BYTES: usize = 256 * 1024 * 1024;
const MAX_VALUE_BYTES: usize = 8 * 1024 * 1024;

fn too_large() -> StorageError {
    StorageError::PurgeResponseTooLarge
}

struct Count {
    bytes: usize,
    limit: usize,
}
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| std::io::Error::other("purge response bound exceeded"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Count {
    fn add(&mut self, value: &impl serde::Serialize) -> Result<(), StorageError> {
        // Match project()'s json!(receipt) representation, including f32 ->
        // Value's f64 widening. Each component was bounded before decoding;
        // never construct a Value containing the entire witness union.
        let value =
            serde_json::to_value(value).map_err(|e| StorageError::Serialize(e.to_string()))?;
        serde_json::to_writer(&mut *self, &value).map_err(|_| too_large())
    }
    fn element(&mut self, value: &impl serde::Serialize) -> Result<(), StorageError> {
        self.add(value)?;
        // One comma per item overcounts the first item, never undercounts.
        self.write_all(b",").map_err(|_| too_large())
    }
}

pub(super) struct Scan {
    items: usize,
    bytes: usize,
    value_limit: usize,
}
impl Scan {
    pub(super) fn new(value_limit: usize) -> Self {
        Self {
            items: 0,
            bytes: 0,
            value_limit,
        }
    }

    pub(super) fn resolve_name(
        &mut self,
        tx: &redb::WriteTransaction,
        name: &str,
    ) -> Result<Option<NamespaceId>, StorageError> {
        let mut exact = None;
        let mut folded = None;
        for row in tx.open_table(NAMESPACE_TABLE)?.iter()? {
            let (id, bytes) = row?;
            self.examine(bytes.value().len())?;
            // Same decoder/error as the existing list-based name resolver.
            let config: NamespaceConfig = serde_json::from_slice(bytes.value())
                .map_err(|e| StorageError::Deserialize(e.to_string()))?;
            if config.id.get() != id.value() {
                return Err(StorageError::InvalidControl(
                    "purge namespace key mismatch".into(),
                ));
            }
            if exact.is_none() && config.name == name {
                exact = Some(config.id);
            }
            if folded.is_none() && config.name.eq_ignore_ascii_case(name) {
                folded = Some(config.id);
            }
            // No early positive: later malformed/oversized rows must still
            // reject, and a later exact match takes precedence over a fold.
        }
        Ok(exact.or(folded))
    }
    fn examine(&mut self, bytes: usize) -> Result<(), StorageError> {
        self.items = self.items.checked_add(1).ok_or_else(too_large)?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(too_large)?;
        if self.items > MAX_EXAMINATIONS
            || self.bytes > MAX_SOURCE_BYTES
            || bytes > self.value_limit.min(MAX_VALUE_BYTES)
        {
            return Err(too_large());
        }
        Ok(())
    }
    fn record(
        &mut self,
        tx: &redb::WriteTransaction,
        id: MemoryId,
    ) -> Result<DiskRecord, StorageError> {
        let table = tx.open_table(META_TABLE)?;
        let bytes = table
            .get(id.as_bytes().as_slice())?
            .ok_or(StorageError::NotFound(id.into_inner()))?;
        self.examine(bytes.value().len())?;
        let record = DiskRecord::from_bytes(bytes.value())?;
        if record.id != *id.as_bytes() {
            return Err(StorageError::InvalidControl(
                "purge admission record key mismatch".into(),
            ));
        }
        Ok(record)
    }
    fn state(
        &mut self,
        tx: &redb::WriteTransaction,
        record: &DiskRecord,
    ) -> Result<NativeRecordState, StorageError> {
        let table = tx.open_table(NAMESPACE_TABLE)?;
        let bytes = table
            .get(record.namespace_id)?
            .ok_or(StorageError::NamespaceNotFound(record.namespace_id))?;
        self.examine(bytes.value().len())?;
        let namespace = NamespaceConfig::decode(bytes.value())?;
        // Only sizing: a digest byte takes at most three JSON digits. No text
        // read, invented commitment, or sizing state escapes into a receipt.
        let mut state = NativeRecordState::capture(record, &namespace, Some([255; 32]))?;
        // Permitted continuations can only decrement edge_count and increment
        // record_generation, including a target that is a peer before deletion.
        state.edge_count = u16::MAX;
        state.lifecycle.lifecycle.record_generation = Generation::MAX;
        Ok(state)
    }
}

/// Check before decoding a potentially large start namespace. This is a
/// conservative size refusal, not a new namespace schema limit.
pub(super) fn check_namespace_size(
    tx: &redb::WriteTransaction,
    namespace: NamespaceId,
    scan: &mut Scan,
) -> Result<(), StorageError> {
    let table = tx.open_table(NAMESPACE_TABLE)?;
    if let Some(bytes) = table.get(namespace.get())? {
        scan.examine(bytes.value().len())?;
    }
    Ok(())
}

// An ephemeral serialization envelope with maximal-width IDs/counters. Never
// validated, committed, returned, or used as evidence of an actual operation.
fn sizing_receipt(outcome: MutationOutcome) -> WriteReceipt {
    WriteReceipt {
        store_id: uuid::Uuid::from_u128(u128::MAX),
        operation_id: uuid::Uuid::from_u128(u128::MAX),
        committed_generation: Generation::MAX,
        receipt_version: 1,
        outcome,
        replayed: false,
    }
}

pub(super) fn admit(
    tx: &redb::WriteTransaction,
    namespace: NamespaceId,
    start: &MutationOutcome,
    edges: &EdgeStore,
    scan: &mut Scan,
    force: bool,
) -> Result<(), StorageError> {
    let limit = scan.value_limit;
    let mut start_count = Count { bytes: 0, limit };
    start_count.add(&sizing_receipt(start.clone()))?;
    // Count the union of every possible child, not just one predicted partition.
    // This deliberately refuses some large namespaces that smaller chunks could
    // serve. There is no durable budget and all existing resume paths stay intact.
    let mut count = Count { bytes: 0, limit };
    count.add(&sizing_receipt(
        MutationOutcome::NativeNamespacePurgeAdvanced {
            transition: NativePurgeAdvanced {
                parent_operation_id: uuid::Uuid::from_u128(u128::MAX),
                namespace_id: namespace,
                previous_generation: Generation::MAX,
                sequence: u64::MAX,
                stage: PurgeStage::Deleting,
                chunk: Some(NativePurgeChunk {
                    deletions: vec![],
                    peers: vec![],
                    removed_edges: vec![],
                }),
            },
        },
    ))?;
    // Cleanup/Complete have chunk=null instead of the three empty arrays. Check
    // both using their actual serializer instead of assuming their scaffold size.
    for stage in [PurgeStage::Cleanup, PurgeStage::Complete] {
        Count { bytes: 0, limit }.add(&sizing_receipt(
            MutationOutcome::NativeNamespacePurgeAdvanced {
                transition: NativePurgeAdvanced {
                    parent_operation_id: uuid::Uuid::from_u128(u128::MAX),
                    namespace_id: namespace,
                    previous_generation: Generation::MAX,
                    sequence: u64::MAX,
                    stage,
                    chunk: None,
                },
            },
        ))?;
    }
    for row in tx.open_table(META_TABLE)?.iter()? {
        let (key, bytes) = row?;
        scan.examine(bytes.value().len())?;
        let record = DiskRecord::from_bytes(bytes.value())?;
        if key.value() != record.id.as_slice() {
            return Err(StorageError::InvalidControl(
                "purge admission record key mismatch".into(),
            ));
        }
        if record.namespace_id == namespace.get() {
            if record.access_history.len() > ACCESS_HISTORY_MAX {
                return Err(StorageError::InvalidControl(
                    "native purge deletion history exceeds canonical bound".into(),
                ));
            }
            if record
                .schema
                .as_ref()
                .is_none_or(|s| !s.extensions.is_empty())
            {
                return Err(StorageError::RecoveryRequired(
                    "native purge requires a consumer for unknown record extensions".into(),
                ));
            }
            if !force && record.phase != crate::model::DecayPhase::Tombstone {
                return Err(StorageError::InvalidMutation(
                    "namespace contains live memories; force is required".into(),
                ));
            }
            let before = scan.state(tx, &record)?;
            count.element(&NativeDeletionWitness {
                memory_id: MemoryId::from_bytes(record.id),
                namespace_id: namespace,
                before,
                history: record.access_history,
            })?;
        }
    }
    // Use the existing raw read transaction; never load all edges into a Vec.
    let edge_tx = edges.read_transaction()?;
    for row in edge_tx.open_table(FORWARD_EDGES)?.iter()? {
        let (key, bytes) = row?;
        scan.examine(
            key.value()
                .len()
                .checked_add(bytes.value().len())
                .ok_or_else(too_large)?,
        )?;
        let edge = PersistedEdge::from_forward(key.value(), bytes.value())?;
        let source = scan.record(tx, edge.source)?;
        let target = scan.record(tx, edge.target)?;
        if source.namespace_id != namespace.get() && target.namespace_id != namespace.get() {
            continue;
        }
        count.element(&NativeEdgeProof::from(&edge))?;
        let before = scan.state(tx, &source)?;
        // Include target sources too: a different chunk can delete their target
        // first. Duplicate sources overcount intentionally, requiring no set.
        count.element(&NativePeerChange {
            namespace_id: NamespaceId::new(source.namespace_id),
            change: NativeRecordChange {
                memory_id: edge.source,
                before: Some(before.clone()),
                after: before,
            },
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_api1_purge_count_matches_projected_value_at_exact_limit() {
        #[derive(serde::Serialize)]
        struct Component<'a> {
            stability: f32,
            name: &'a str,
        }
        for stability in [3.7145_f32, f32::MIN_POSITIVE, 0.3, 1.] {
            let component = Component {
                stability,
                name: "escaped \" \\ \0 界",
            };
            let projected = serde_json::to_vec(&serde_json::to_value(&component).unwrap()).unwrap();
            if stability == 3.7145 {
                assert!(serde_json::to_vec(&component).unwrap().len() < projected.len());
            }
            let mut exact = Count {
                bytes: 0,
                limit: projected.len(),
            };
            exact.add(&component).unwrap();
            assert_eq!(exact.bytes, projected.len());
            assert!(matches!(
                Count {
                    bytes: 0,
                    limit: projected.len() - 1
                }
                .add(&component),
                Err(StorageError::PurgeResponseTooLarge)
            ));
        }
        assert!(
            Count {
                bytes: usize::MAX,
                limit: usize::MAX
            }
            .write_all(b"x")
            .is_err()
        );
    }

    #[test]
    fn native_api1_purge_raw_name_scan_preserves_precedence_and_rejects_late_caps() {
        let root = tempfile::tempdir().unwrap();
        let db = redb::Database::create(root.path().join("names.db")).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx.open_table(NAMESPACE_TABLE).unwrap();
            for (id, name) in [(1, "fold"), (2, "Fold"), (3, "other")] {
                let mut config = NamespaceConfig::default_namespace(100);
                config.id = NamespaceId::new(id);
                config.name = name.into();
                table
                    .insert(id, serde_json::to_vec(&config).unwrap().as_slice())
                    .unwrap();
            }
        }
        tx.commit().unwrap();
        let tx = db.begin_write().unwrap();
        for (name, expected) in [("Fold", Some(2)), ("FOLD", Some(1)), ("missing", None)] {
            let mut scan = Scan::new(4096);
            assert_eq!(
                scan.resolve_name(&tx, name).unwrap().map(|id| id.get()),
                expected
            );
            assert_eq!(scan.items, 3);
        }
        let mut scan = Scan::new(4096);
        scan.items = MAX_EXAMINATIONS - 2;
        assert!(matches!(
            scan.resolve_name(&tx, "fold"),
            Err(StorageError::PurgeResponseTooLarge)
        ));
        // The original exact match occurs first. An oversized malformed later
        // value must be refused before decoding, never hidden by that match.
        tx.open_table(NAMESPACE_TABLE)
            .unwrap()
            .insert(3, vec![b'x'; 8192].as_slice())
            .unwrap();
        assert!(matches!(
            Scan::new(4096).resolve_name(&tx, "fold"),
            Err(StorageError::PurgeResponseTooLarge)
        ));
        tx.open_table(NAMESPACE_TABLE)
            .unwrap()
            .insert(3, b"{".as_slice())
            .unwrap();
        assert!(matches!(
            Scan::new(4096).resolve_name(&tx, "fold"),
            Err(StorageError::Deserialize(_))
        ));
        let mut scan = Scan::new(4096);
        scan.bytes = MAX_SOURCE_BYTES;
        assert!(matches!(
            scan.examine(1),
            Err(StorageError::PurgeResponseTooLarge)
        ));
        tx.abort().unwrap();
    }
}
