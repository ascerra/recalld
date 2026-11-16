//! Real HTTP probes. The lost-ACK hook synthesizes an ambiguous return after
//! successful metadata commit; it does not simulate power loss or physical I/O.
use super::*;
use crate::model::{DecayPhase, MemoryId, NamespaceId};
use crate::storage::control::{self, MutationContext, MutationOutcome, WriteReceipt};
use crate::storage::native_lifecycle::{NativeLifecycleSession, NativePolicyInputs};
use crate::storage::schema_migration::{
    MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
};
use redb::ReadableTable;
use std::sync::atomic::{AtomicUsize, Ordering};

const NOW: i64 = 1000 + 10_000 * 86_400_000;
struct SweepClock(AtomicUsize);
impl crate::time::Clock for SweepClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        NOW
    }
}

async fn fixture() -> (Fixture, Vec<MemoryId>, Arc<SweepClock>) {
    let f = Fixture::new();
    let mut ids = Vec::new();
    for n in 0..2 {
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: format!("sweep receipt probe {n}"),
            full_text: Some(format!("retained full text {n}")),
            tags: vec![],
            initial_stability: Some(1.),
            created_at: Some(1000),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        let receipt = f
            .storage
            .write()
            .unwrap()
            .insert_memory_with_receipt(
                &MutationContext::unkeyed(NamespaceId::new(1)),
                &input,
                &[],
                1000,
            )
            .unwrap();
        ids.push(crate::storage::insertion::inserted_id(&receipt).unwrap());
    }
    f.completion().rebuild_all().await.unwrap();
    let legacy = f.close();
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("schema");
    migrate_schema(
        legacy.path(),
        &destination,
        &MigrationOptions {
            migration_now: 1000,
            expected_source: inspect_schema(legacy.path())
                .unwrap()
                .control
                .generation_ref(),
            attestations: vec![ProfileAttestationInput {
                namespace_id: NamespaceId::new(1),
                namespace_name: "default".into(),
                descriptor: crate::embedding::runtime::tests::profile(),
            }],
        },
    )
    .unwrap();
    crate::test_support::explicitly_adopt_inherited_policy(
        &destination,
        NamespaceId::new(1),
        1.,
        1000,
    );
    let clock = Arc::new(SweepClock(AtomicUsize::new(0)));
    let session =
        NativeLifecycleSession::open(&destination, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    f.completion().rebuild_all().await.unwrap();
    (f, ids, clock)
}

fn app(f: &Fixture, clock: Arc<SweepClock>) -> Router {
    let (mut state, _bridge) = fixture_state(f);
    state.decay = Arc::new(
        FsrsEngineAdapter::new(
            f.storage.clone(),
            f.cache.clone(),
            f.graph.clone(),
            Arc::new(Default::default()),
            false,
        )
        .with_indexes(
            f.vector_index.clone(),
            f.fts_index.clone(),
            f.entity_index.clone(),
        )
        .with_clock(clock)
        .with_native_lifecycle_policy(NativePolicyInputs::default()),
    );
    crate::api::routes::router(state, &Default::default())
}

fn receipts(read: &redb::ReadTransaction) -> Vec<(Vec<u8>, Vec<u8>)> {
    read.open_table(control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|row| {
            let (key, value) = row.unwrap();
            (key.value().to_vec(), value.value().to_vec())
        })
        .collect()
}

fn receipt_snapshot(path: &std::path::Path) -> Vec<(Vec<u8>, Vec<u8>)> {
    let (_copy, db) =
        crate::storage::validation::inspect_database_copy(&path.join("meta.db"), true).unwrap();
    receipts(&db.begin_read().unwrap())
}

async fn request(app: &Router, body: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/decay/sweep")
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    println!(
        "HTTP status={status} raw_body={}",
        String::from_utf8_lossy(&bytes)
    );
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn probe(nth: usize, known: bool) {
    let (f, ids, clock) = fixture().await;
    let path = f.storage.read().unwrap().db_path().to_owned();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let old_receipts = receipt_snapshot(&path);
    let original: Vec<_> = ids.iter().map(|id| f.record(*id).unwrap()).collect();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    if known {
        // Existing real derived rebuild failure, not the synthetic lost ACK.
        *f.vector_index.write().await = crate::search::VectorIndexOwner::uniform(8);
    } else {
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .record_commit_failure_nth
            .store(nth, Ordering::SeqCst);
    }
    let router = app(&f, clock.clone());
    let (status, reply) = request(&router, "{}").await;
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    if !known {
        let engine = f.storage.read().unwrap();
        assert!(engine.meta_store().ensure_available().is_err());
        assert_eq!(
            engine
                .meta_store()
                .record_commit_failure_nth
                .load(Ordering::SeqCst),
            0
        );
        let work = tracker.status();
        assert!(
            work.pending > 0 || work.unresolved > 0,
            "unknown cannot finish accepted work"
        );
    }
    let target = before.generation_ref();
    assert!(
        f.completion()
            .readiness_barrier(
                target,
                std::time::Instant::now() + std::time::Duration::from_millis(100),
            )
            .await
            .is_err(),
        "failed request must not manufacture ready"
    );
    drop(router);
    let _root = f.close();

    // Retain the actual outbox/ledger evidence before writable recovery. The
    // copy helper repairs only its disposable copy, not the closed source.
    let (_copy, db) =
        crate::storage::validation::inspect_database_copy(&path.join("meta.db"), true).unwrap();
    let read = db.begin_read().unwrap();
    let control = control::validate_existing(&read).unwrap().unwrap();
    assert_eq!(control.store_id, before.store_id);
    assert_eq!(
        control.committed_generation.get(),
        before.committed_generation.get() + nth as u64
    );
    let rows = receipts(&read);
    for old in &old_receipts {
        assert!(rows.contains(old), "historical receipt bytes changed");
    }
    let mut new: Vec<_> = rows
        .iter()
        .filter(|row| !old_receipts.contains(row))
        .map(|(_, bytes)| control::validate_receipt(bytes).unwrap())
        .collect();
    new.sort_by_key(|r| r.receipt.committed_generation);
    assert_eq!(new.len(), nth, "no extra canonical mutation/compaction");
    let pending = crate::storage::metadata::effects::validate_existing(&read).unwrap();
    assert_eq!(pending.len(), 1, "only failed publication remains pending");
    assert_eq!(pending[0].receipt, new[nth - 1].receipt);
    let ledger = crate::storage::metadata::removal_ledger::read_export(&read, Some(&control))
        .unwrap()
        .unwrap();
    let mut changed = Vec::new();
    for (index, stored) in new.iter().enumerate() {
        let receipt = &stored.receipt;
        let MutationOutcome::NativePhaseMaterialized { transition } = &receipt.outcome else {
            panic!("unexpected canonical operation: {receipt:?}");
        };
        assert_eq!(receipt.store_id, before.store_id);
        assert_eq!(
            receipt.committed_generation.get(),
            before.committed_generation.get() + index as u64 + 1
        );
        assert_eq!(transition.namespace_id, NamespaceId::new(1));
        assert_eq!(transition.sampled_at, NOW);
        assert_eq!(transition.change.after.lifecycle.phase, DecayPhase::Summary);
        assert!(ids.contains(&transition.memory_id));
        let ctx = MutationContext::unkeyed(transition.namespace_id);
        assert_eq!(
            stored.request_hash,
            control::request_hash(
                &ctx,
                "native_maintenance",
                &[transition.memory_id.as_bytes()],
            ),
            "actual public API1 origin"
        );
        assert!(ledger.entries.iter().any(|e| e.memory_uuid.as_bytes()
            == transition.memory_id.as_bytes()
            && e.namespace_id == 1
            && e.operation_id == receipt.operation_id
            && e.generation == receipt.committed_generation
            && e.terminal_kind.is_none()
            && e.removed_fields_mask == crate::storage::metadata::removal_ledger::FULL_TEXT));
        changed.push(transition.memory_id);
    }
    let new: Vec<WriteReceipt> = new.into_iter().map(|r| r.receipt).collect();
    drop((read, db));

    // Actual strict native reopen validates history and recovers durable effects.
    let reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
    assert_eq!(
        reopened
            .engine
            .control_state()
            .unwrap()
            .committed_generation,
        control.committed_generation
    );
    for (id, old) in ids.iter().zip(&original) {
        let current = reopened.engine.get_record(*id).unwrap().unwrap();
        if changed.contains(id) {
            assert_eq!(current.phase, DecayPhase::Summary);
            assert_eq!(current.text_length, 0);
            assert_eq!(current.summary, old.summary);
            assert_eq!(current.tags, old.tags);
            assert_eq!(current.access_history, old.access_history);
            assert_eq!(current.created_at, old.created_at);
            assert_eq!(current.stability, old.stability);
            assert_eq!(current.last_accessed_at, old.last_accessed_at);
            assert_eq!(
                current.schema.as_ref().unwrap().lifecycle.decay_anchor_at,
                old.schema.as_ref().unwrap().lifecycle.decay_anchor_at
            );
            assert_eq!(
                current.schema.as_ref().unwrap().lifecycle.phase_changed_at,
                NOW
            );
        } else {
            assert_eq!(current.encode().unwrap(), old.encode().unwrap());
        }
    }
    assert_eq!(
        rows,
        receipt_snapshot(&path),
        "reopen must not replay mutation"
    );
    println!(
        "durable pre_response_assertions: known={known} nth={nth} G={:?}->{:?} receipts={}",
        before.committed_generation,
        control.committed_generation,
        serde_json::to_string(&new).unwrap()
    );

    // These assertions intentionally come after all durable checks. First/later
    // unknown probes must fail here on the unchanged production HTTP mapping.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    if known {
        assert_eq!(reply["error"], "INDEX_NOT_READY");
        assert_eq!(reply["commitState"], "committed");
        assert_eq!(reply["ready"], false);
        assert_eq!(reply["receipt"], serde_json::to_value(&new[0]).unwrap());
    } else {
        assert_eq!(
            reply["commitState"], "unknown",
            "actual ID must not be only message text"
        );
        assert_eq!(reply["operationId"], new[nth - 1].operation_id.to_string());
        assert_eq!(reply["ready"], false);
        assert_eq!(
            reply["receipts"],
            serde_json::to_value(&new[..nth - 1]).unwrap(),
            "only prior known receipts; never synthesize current receipt"
        );
        assert!(reply.get("receipt").is_none_or(Value::is_null));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_first_lost_ack_retains_typed_unknown() {
    probe(1, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_later_lost_ack_retains_prior_receipt_and_typed_unknown() {
    probe(2, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_known_publication_failure_retains_full_receipt() {
    probe(1, true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_override_rejects_before_armed_commit_hook() {
    let (f, ids, clock) = fixture().await;
    let old: Vec<_> = ids
        .iter()
        .map(|id| f.record(*id).unwrap().encode().unwrap())
        .collect();
    let before = f.storage.read().unwrap().control_state().unwrap();
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .record_commit_failure_nth
        .store(1, Ordering::SeqCst);
    let router = app(&f, clock.clone());
    let (status, reply) = request(&router, "{\"asOf\":null}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(reply["field"], "asOf");
    assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    let engine = f.storage.read().unwrap();
    assert_eq!(
        engine.control_state().unwrap().generation_ref(),
        before.generation_ref()
    );
    assert_eq!(
        engine
            .meta_store()
            .record_commit_failure_nth
            .load(Ordering::SeqCst),
        1
    );
    engine.meta_store().ensure_available().unwrap();
    for (id, bytes) in ids.iter().zip(old) {
        assert_eq!(
            engine.get_record(*id).unwrap().unwrap().encode().unwrap(),
            bytes
        );
    }
    assert_eq!(engine.meta_store().work_tracker().status().pending, 0);
}
