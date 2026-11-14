//! Known replay evidence must survive validation failure without a second lookup.
use super::*;
use crate::storage::StorageEngine;
use crate::storage::native_purge_tests::{fixture, open};

#[tokio::test]
async fn native_context_purge_replay_validation_preserves_exact_known_receipt() {
    for contract in [1, 2] {
        // The final case fails inside lookup, before any receipt is available.
        for damage in ["start-proof", "missing-progress", "missing-receipt"] {
            let root = tempfile::tempdir().unwrap();
            let (path, namespace, _) = fixture(root.path());
            let session = open(&path);
            let fx = crate::test_support::Fixture::from_open_engine(root, session.engine);
            let publisher = crate::derived_completion::DerivedCompletion {
                storage: fx.storage.clone(),
                cache: fx.cache.clone(),
                graph: fx.graph.clone(),
                vectors: fx.vector_index.clone(),
                fts: fx.fts_index.clone(),
                entities: fx.entity_index.clone(),
            };
            let (key, start, state, original_receipt) = {
                let mut storage = fx.storage.write().unwrap();
                let before = storage.control_state().unwrap();
                let key = MutationContext {
                    store_id: Some(before.store_id),
                    scope: MutationScope::Namespace(namespace),
                    idempotency_key: Some("known-purge".into()),
                    expected_generation: Some(before.committed_generation),
                    contract_version: contract,
                };
                let start = storage.begin_namespace_purge(&key, true).unwrap();
                let state =
                    control::validate_existing(&storage.meta_store().db.begin_read().unwrap())
                        .unwrap()
                        .unwrap();
                let meta = storage.meta_store();
                let tx = meta.db.begin_write().unwrap();
                let receipt_key = control::scoped_key(
                    start.store_id,
                    key.scope,
                    start.start_receipt.operation_id.as_bytes(),
                );
                let original = tx
                    .open_table(control::RECEIPTS)
                    .unwrap()
                    .get(receipt_key.as_slice())
                    .unwrap()
                    .unwrap()
                    .value()
                    .to_vec();
                match damage {
                    "start-proof" => {
                        let mut corrupted = start.clone();
                        corrupted.name = "wrong-scope-name".into();
                        save(&tx, &corrupted).unwrap();
                    }
                    "missing-progress" => {
                        tx.open_table(PROGRESS)
                            .unwrap()
                            .remove(start.start_receipt.operation_id.as_bytes().as_slice())
                            .unwrap();
                    }
                    "missing-receipt" => {
                        tx.open_table(control::RECEIPTS)
                            .unwrap()
                            .remove(receipt_key.as_slice())
                            .unwrap();
                    }
                    _ => unreachable!(),
                }
                tx.commit().unwrap();
                (key, start, state, original)
            };
            let frozen = std::fs::read(path.join("meta.db")).unwrap();
            let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
            let work = tracker.begin().unwrap();
            let error = publisher
                .purge_native_accepted(key, true, &work)
                .await
                .unwrap_err();
            work.finish();
            if damage == "missing-receipt" {
                assert!(
                    matches!(error, StorageError::InvalidControl(_)),
                    "no invented receipt: {error:?}"
                );
            } else {
                let StorageError::CommittedMutation { receipt, source } = error else {
                    panic!("lost known receipt for contract{contract}/{damage}: {error:?}");
                };
                let mut expected = start.start_receipt.clone();
                expected.replayed = true;
                assert_eq!(*receipt, expected);
                assert!(matches!(*source, StorageError::InvalidControl(_)));
                let storage = fx.storage.read().unwrap();
                let read = storage.meta_store().db.begin_read().unwrap();
                let receipts = read.open_table(control::RECEIPTS).unwrap();
                let retained = receipts
                    .get(
                        control::scoped_key(
                            start.store_id,
                            MutationScope::Namespace(namespace),
                            receipt.operation_id.as_bytes(),
                        )
                        .as_slice(),
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(retained.value(), original_receipt);
                let mut canonical = control::validate_receipt(retained.value()).unwrap().receipt;
                canonical.replayed = true;
                assert_eq!(*receipt, canonical);
            }
            let storage = fx.storage.read().unwrap();
            let read = storage.meta_store().db.begin_read().unwrap();
            let current = read.open_table(control::CONTROL).unwrap();
            assert_eq!(
                ControlState::decode(current.get("state").unwrap().unwrap().value()).unwrap(),
                state
            );
            assert_eq!(std::fs::read(path.join("meta.db")).unwrap(), frozen);
        }
    }
}
