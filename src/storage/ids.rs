//! Allocation state belongs to one metadata owner, never a process-global RNG.
//! A stream checkpoint alone is NOT a consistent whole-core snapshot.
use crate::model::MemoryId;
use crate::storage::StorageError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) enum IdKind {
    Store,
    Operation,
    TextGeneration,
}

pub(crate) trait IdSource: Send + Sync {
    fn uuid(&self, kind: IdKind) -> Result<Uuid, StorageError>;
    fn memory(&self, sampled_at: i64) -> Result<MemoryId, StorageError>;
    /// The caller must retain the whole-core freeze when bundling runtime state.
    fn checkpoint(&self) -> Result<Option<IdState>, StorageError>;
}

pub(crate) struct SystemIds;
impl IdSource for SystemIds {
    fn uuid(&self, _: IdKind) -> Result<Uuid, StorageError> {
        Ok(Uuid::new_v4())
    }
    fn memory(&self, _: i64) -> Result<MemoryId, StorageError> {
        Ok(MemoryId::new())
    }
    fn checkpoint(&self) -> Result<Option<IdState>, StorageError> {
        Ok(None)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdState {
    version: u32,
    seed: [u8; 32],
    counter: u64,
    last_memory_millis: Option<u64>,
}

pub(crate) struct SeededIds(Mutex<IdState>);
fn invalid(message: &str) -> StorageError {
    StorageError::InvalidControl(format!("owned allocation source: {message}"))
}
impl SeededIds {
    pub(crate) fn new(seed: [u8; 32]) -> Self {
        Self(Mutex::new(IdState {
            version: 1,
            seed,
            counter: 0,
            last_memory_millis: None,
        }))
    }
    /// Restore only when constructing a fresh isolated owner, never a live rewind.
    pub(crate) fn from_state(state: IdState) -> Result<Self, StorageError> {
        if state.version != 1
            || state.counter == u64::MAX
            || state.last_memory_millis.is_some_and(|v| v >= (1_u64 << 48))
        {
            return Err(invalid("unsupported or invalid state"));
        }
        Ok(Self(Mutex::new(state)))
    }
    fn next(&self, kind: u8, millis: Option<i64>) -> Result<Uuid, StorageError> {
        let mut state = self.0.lock().map_err(|_| invalid("poisoned state"))?;
        let millis = millis
            .map(|value| {
                let value = u64::try_from(value).map_err(|_| invalid("negative UUIDv7 time"))?;
                if value >= (1_u64 << 48) || state.last_memory_millis.is_some_and(|old| value < old)
                {
                    return Err(invalid("UUIDv7 time out of range or regressed"));
                }
                Ok(value)
            })
            .transpose()?;
        let next = state
            .counter
            .checked_add(1)
            .ok_or_else(|| invalid("counter exhausted"))?;
        let mut hash = Sha256::new();
        hash.update(b"recalld-isolated-ids-v1");
        hash.update(state.seed);
        hash.update([kind]);
        hash.update(next.to_be_bytes());
        let digest = hash.finalize();
        let mut bytes: [u8; 16] = digest[..16].try_into().unwrap();
        if let Some(millis) = millis {
            // Preserve all 74 available hash bits. UUID order is stable for a
            // frozen seed/time/sequence, not necessarily allocation order.
            bytes[..6].copy_from_slice(&millis.to_be_bytes()[2..]);
            bytes[6] = (bytes[6] & 0x0f) | 0x70;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            state.last_memory_millis = Some(millis);
        } else {
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
        }
        state.counter = next;
        Ok(Uuid::from_bytes(bytes))
    }
}
impl IdSource for SeededIds {
    fn uuid(&self, kind: IdKind) -> Result<Uuid, StorageError> {
        self.next(
            match kind {
                IdKind::Store => 0,
                IdKind::Operation => 1,
                IdKind::TextGeneration => 2,
            },
            None,
        )
    }
    fn memory(&self, sampled_at: i64) -> Result<MemoryId, StorageError> {
        self.next(3, Some(sampled_at)).map(MemoryId::from_uuid)
    }
    fn checkpoint(&self) -> Result<Option<IdState>, StorageError> {
        Ok(Some(
            self.0
                .lock()
                .map_err(|_| invalid("poisoned state"))?
                .clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn seed(n: u64) -> [u8; 32] {
        let mut seed = [0; 32];
        seed[..8].copy_from_slice(&n.to_be_bytes());
        seed
    }

    #[test]
    fn distinct_seeds_reproduce_old_collision_but_keep_full_hash_payload_distinct() {
        let mut old_prefixes = HashSet::new();
        let mut actual_ids = HashSet::new();
        let mut collisions = 0;
        // Pigeonhole proof, not a probabilistic expectation: 1025 inputs and
        // only 1024 old prefixes. Same timestamp/counter made those UUIDs equal.
        for n in 0..1025 {
            let seed = seed(n);
            let hash = Sha256::digest(seed);
            let old = u16::from_be_bytes([hash[0], hash[1]]) & 0x03ff;
            collisions += usize::from(!old_prefixes.insert(old));
            let id = SeededIds::new(seed).memory(1_767_225_600_000).unwrap();
            assert!(
                actual_ids.insert(id),
                "full hash collision for fixture seed {n}"
            );
            assert_eq!(id.as_bytes()[6] >> 4, 7);
            assert_eq!(id.as_bytes()[8] >> 6, 2);
        }
        assert!(collisions > 0);
        eprintln!(
            "1025 distinct seeds: old-layout collisions={collisions}; full-hash collisions=0"
        );
    }

    #[test]
    fn restored_stream_and_independent_owners_have_exact_future_identity() {
        let first = SeededIds::new(seed(7));
        let other = SeededIds::new(seed(7));
        assert_eq!(
            first.uuid(IdKind::Store).unwrap(),
            other.uuid(IdKind::Store).unwrap()
        );
        let state = first.checkpoint().unwrap().unwrap();
        let restored = SeededIds::from_state(state.clone()).unwrap();
        let expected = first.memory(1000).unwrap();
        assert_eq!(restored.memory(1000).unwrap(), expected);
        assert_eq!(other.checkpoint().unwrap().unwrap(), state);
        assert_eq!(
            first.uuid(IdKind::Operation).unwrap(),
            restored.uuid(IdKind::Operation).unwrap()
        );
        assert_eq!(first.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }

    #[test]
    fn exhausted_restore_and_invalid_time_fail_without_consuming_cursor() {
        let ids = SeededIds::new(seed(11));
        let before = ids.checkpoint().unwrap().unwrap();
        assert!(ids.memory(-1).is_err());
        assert!(ids.memory(1_i64 << 48).is_err());
        assert_eq!(ids.checkpoint().unwrap().unwrap(), before);
        ids.memory(1000).unwrap();
        let mut state = ids.checkpoint().unwrap().unwrap();
        assert!(ids.memory(999).is_err());
        assert_eq!(ids.checkpoint().unwrap().unwrap(), state);
        state.counter = u64::MAX;
        assert!(SeededIds::from_state(state.clone()).is_err());
        // A running owner can exhaust. It remains inspectable, but cannot be
        // restored as a writable episode or consume/wrap another identity.
        let exhausted = SeededIds(Mutex::new(state.clone()));
        assert!(exhausted.uuid(IdKind::Operation).is_err());
        assert_eq!(exhausted.checkpoint().unwrap().unwrap(), state);
    }
    #[test]
    fn owned_ids_actual_legacy_insert_replay_supplied_id_and_reopen_cursor() {
        use crate::model::{NamespaceConfig, NamespaceId};
        use crate::storage::control::MutationContext;
        use crate::storage::insertion::{InsertMemory, inserted_id};
        use crate::storage::{RedbStorageEngine, StorageEngine};
        let input = InsertMemory {
            id: None,
            summary: "scoped identity".into(),
            full_text: Some("exact payload".into()),
            tags: vec![],
            initial_stability: None,
            created_at: Some(17),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        let mut projections = Vec::new();
        for _ in 0..2 {
            let root = tempfile::tempdir().unwrap();
            let mut engine = RedbStorageEngine::open_with_ids(
                root.path(),
                false,
                Box::new(SeededIds::new(seed(7))),
            )
            .unwrap();
            let mut config = NamespaceConfig::default_namespace(0);
            config.embedding_dim = 4;
            let ns = engine.create_namespace(&config).unwrap();
            let ctx = MutationContext {
                store_id: Some(engine.control_state().unwrap().store_id),
                idempotency_key: Some("first".into()),
                ..MutationContext::unkeyed(ns)
            };
            let before = engine
                .meta_store()
                .id_source()
                .checkpoint()
                .unwrap()
                .unwrap();
            let receipt = engine
                .insert_memory_with_receipt(&ctx, &input, &[], 1000)
                .unwrap();
            let id = inserted_id(&receipt).unwrap();
            assert_eq!(&id.as_bytes()[..6], &1000_u64.to_be_bytes()[2..]);
            assert_eq!(engine.get_record(id).unwrap().unwrap().created_at, 17);
            let cursor = engine
                .meta_store()
                .id_source()
                .checkpoint()
                .unwrap()
                .unwrap();
            assert_eq!(cursor.counter, before.counter + 2); // memory + operation
            let replay = engine
                .insert_memory_with_receipt(&ctx, &input, &[], 9000)
                .unwrap();
            assert!(replay.replayed);
            assert_eq!(replay.operation_id, receipt.operation_id);
            assert_eq!(inserted_id(&replay).unwrap(), id);
            assert_eq!(
                engine
                    .meta_store()
                    .id_source()
                    .checkpoint()
                    .unwrap()
                    .unwrap(),
                cursor
            );
            projections.push(serde_json::to_value(&receipt).unwrap());
            drop(engine);
            // Controlled stream restore at an already closed owner, not a live
            // rewind or a claim that ordinary reopen implements A6 reset.
            let expected = SeededIds::from_state(cursor.clone()).unwrap();
            let next_id = expected.memory(2000).unwrap();
            let next_op = expected.uuid(IdKind::Operation).unwrap();
            let mut engine = RedbStorageEngine::open_with_ids(
                root.path(),
                false,
                Box::new(SeededIds::from_state(cursor).unwrap()),
            )
            .unwrap();
            let next = engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 2000)
                .unwrap();
            assert_eq!(inserted_id(&next).unwrap(), next_id);
            assert_eq!(next.operation_id, next_op);
            let cursor = engine
                .meta_store()
                .id_source()
                .checkpoint()
                .unwrap()
                .unwrap();
            let mut supplied = input.clone();
            supplied.id = Some(MemoryId::from_bytes([42; 16]));
            let receipt = engine
                .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &supplied, &[], 3000)
                .unwrap();
            assert_eq!(inserted_id(&receipt).unwrap(), supplied.id.unwrap());
            let after = engine
                .meta_store()
                .id_source()
                .checkpoint()
                .unwrap()
                .unwrap();
            assert_eq!(after.counter, cursor.counter + 1); // no generated memory ID
            assert!(
                engine
                    .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &supplied, &[], 3000)
                    .is_err()
            );
            assert_eq!(
                engine
                    .meta_store()
                    .id_source()
                    .checkpoint()
                    .unwrap()
                    .unwrap(),
                after
            );
            assert_eq!(engine.scan_all().unwrap().len(), 3);
            assert_eq!(ns, NamespaceId::new(1));
        }
        assert_eq!(projections[0], projections[1]);
    }
}
