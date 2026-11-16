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

async fn legacy_fixture() -> (Fixture, Vec<MemoryId>, Arc<SweepClock>) {
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
    (f, ids, Arc::new(SweepClock(AtomicUsize::new(0))))
}

async fn fixture() -> (Fixture, Vec<MemoryId>, Arc<SweepClock>) {
    let (f, ids, clock) = legacy_fixture().await;
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
    let session =
        NativeLifecycleSession::open(&destination, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    f.completion().rebuild_all().await.unwrap();
    (f, ids, clock)
}

fn app(f: &Fixture, clock: Arc<SweepClock>) -> Router {
    app_with_limit(f, clock, crate::daemon::protocol::MAX_MESSAGE_SIZE)
}

fn app_with_limit(f: &Fixture, clock: Arc<SweepClock>, limit: usize) -> Router {
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
        .with_sweep_evidence_limit(limit)
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

fn control_snapshot(path: &std::path::Path) -> control::ControlState {
    // Active purge intentionally rejects the ordinary admitted control reader.
    // Inspect a disposable metadata copy; never bypass/change the source guard.
    let (_copy, db) =
        crate::storage::validation::inspect_database_copy(&path.join("meta.db"), true).unwrap();
    control::validate_existing(&db.begin_read().unwrap())
        .unwrap()
        .unwrap()
}

async fn request(app: &Router, body: &str) -> (StatusCode, Value) {
    request_with_limit(app, body, crate::daemon::protocol::MAX_MESSAGE_SIZE).await
}

async fn request_with_limit(app: &Router, body: &str, limit: usize) -> (StatusCode, Value) {
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
    assert!(
        bytes.len() <= limit,
        "actual HTTP bytes {} exceed {limit}",
        bytes.len()
    );
    println!(
        "HTTP status={status} raw_body={}",
        String::from_utf8_lossy(&bytes)
    );
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn probe(nth: usize, known: bool) {
    probe_with_limit(nth, known, crate::daemon::protocol::MAX_MESSAGE_SIZE).await
}

async fn probe_with_limit(nth: usize, known: bool, limit: usize) {
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
    let router = app_with_limit(&f, clock.clone(), limit);
    let (status, reply) = request_with_limit(&router, "{}", limit).await;
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
        // Intentional failure-wire change: typed f32 JSON, not the old
        // json!(receipt) Value promotion. Historical bytes remain untouched.
        let expected: Value =
            serde_json::from_slice(&serde_json::to_vec(&new[0]).unwrap()).unwrap();
        assert_eq!(reply["receipt"], expected);
        assert_eq!(
            serde_json::from_value::<WriteReceipt>(reply["receipt"].clone()).unwrap(),
            new[0]
        );
    } else {
        assert_eq!(
            reply["commitState"], "unknown",
            "actual ID must not be only message text"
        );
        assert_eq!(reply["operationId"], new[nth - 1].operation_id.to_string());
        assert_eq!(reply["ready"], false);
        assert_eq!(
            reply["receipts"],
            serde_json::from_slice::<Value>(&serde_json::to_vec(&new[..nth - 1]).unwrap()).unwrap(),
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

fn canonical(f: &Fixture) -> Vec<u8> {
    let engine = f.storage.read().unwrap();
    let (_copy, db) =
        crate::storage::validation::inspect_database_copy(&engine.db_path().join("meta.db"), true)
            .unwrap();
    let read = db.begin_read().unwrap();
    let mut tables = Vec::new();
    for table in [
        control::RECEIPTS,
        crate::storage::metadata::effects::EFFECTS,
        crate::storage::metadata::removal_ledger::LEDGER,
    ] {
        tables.push(
            read.open_table(table)
                .unwrap()
                .iter()
                .unwrap()
                .map(|row| {
                    let (key, value) = row.unwrap();
                    (key.value().to_vec(), value.value().to_vec())
                })
                .collect::<Vec<_>>(),
        );
    }
    serde_json::to_vec(&(
        engine.control_state().unwrap(),
        engine.list_namespaces().unwrap(),
        engine
            .scan_all()
            .unwrap()
            .into_iter()
            .map(|(id, r)| (id, r.encode().unwrap()))
            .collect::<Vec<_>>(),
        engine.load_all_edges().unwrap(),
        engine.meta_store().text_selection().unwrap(),
        tables,
    ))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_first_and_later_budget_reject_before_canonical_commit() {
    for (limit, committed) in [(2048, 0), (10_000, 1)] {
        let (f, ids, clock) = fixture().await;
        let old = canonical(&f);
        let before = f.storage.read().unwrap().control_state().unwrap();
        let path = f.storage.read().unwrap().db_path().to_owned();
        let before_receipts = receipt_snapshot(&path);
        let originals: Vec<_> = ids
            .iter()
            .map(|id| f.record(*id).unwrap().encode().unwrap())
            .collect();
        let router = app_with_limit(&f, clock.clone(), limit);
        let (status, reply) = request_with_limit(&router, "{}", limit).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reply["error"], "SWEEP_RESPONSE_LIMIT");
        assert_eq!(
            reply["commitState"],
            if committed == 0 {
                "notCommitted"
            } else {
                "committed"
            }
        );
        assert_eq!(reply["complete"], false);
        assert_eq!(reply["retryable"], false);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        let values: Vec<WriteReceipt> = serde_json::from_value(reply["receipts"].clone()).unwrap();
        assert_eq!(values.len(), committed);
        let rows = receipt_snapshot(&path);
        assert_eq!(rows.len(), before_receipts.len() + committed);
        assert!(before_receipts.iter().all(|r| rows.contains(r)));
        assert_eq!(
            f.storage
                .read()
                .unwrap()
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.committed_generation.get() + committed as u64
        );
        if committed == 0 {
            assert_eq!(canonical(&f), old);
        }
        for receipt in &values {
            assert!(
                rows.iter().any(
                    |(_, bytes)| control::validate_receipt(bytes).unwrap().receipt == *receipt
                )
            );
            assert!(matches!(
                receipt.outcome,
                MutationOutcome::NativePhaseMaterialized { .. }
            ));
        }
        let unchanged = ids
            .iter()
            .zip(&originals)
            .filter(|(id, old)| f.record(**id).unwrap().encode().unwrap() == **old)
            .count();
        assert_eq!(unchanged, ids.len() - committed);
        drop(router);
        let _root = f.close();
        let reopened =
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
        assert_eq!(
            reopened
                .engine
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.committed_generation.get() + committed as u64
        );
        assert_eq!(receipt_snapshot(&path), rows);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_admitted_known_wrapper_fits_actual_bounded_body() {
    // A real complete ~3.5KiB phase proof plus array/alias fits this cap; a
    // second complete proof does not. No arbitrary oversized fake outcome.
    probe_with_limit(1, true, 10_000).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_read_only_capture_loss_is_not_a_mutation_unknown() {
    let (f, _ids, clock) = fixture().await;
    let before = canonical(&f);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .sweep_capture_fault
        .store(true, Ordering::SeqCst);
    let router = app(&f, clock.clone());
    let (status, reply) = request(&router, "{}").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply["commitState"], "notCommitted");
    assert_eq!(reply["error"], "SWEEP_UNAVAILABLE");
    assert_eq!(reply["receipts"], json!([]));
    assert!(reply["operationId"].is_null());
    assert_eq!(canonical(&f), before);
    let work = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .status();
    assert_eq!((work.pending, work.unresolved), (0, 0));
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_completion_worker_loss_preserves_unknown_and_prior_receipt() {
    for after_commit in [false, true] {
        let (f, _ids, clock) = fixture().await;
        let before = f.storage.read().unwrap().control_state().unwrap();
        let path = f.storage.read().unwrap().db_path().to_owned();
        let before_receipts = receipt_snapshot(&path);
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .bounded_completion_failure_nth
            .store(if after_commit { 2 } else { 1 }, Ordering::SeqCst);
        let router = app(&f, clock.clone());
        let (status, reply) = request(&router, "{}").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reply["commitState"], "unknown");
        assert!(reply["operationId"].is_null());
        assert!(reply.get("receipt").is_none());
        assert_eq!(reply["ready"], false);
        let known: Vec<WriteReceipt> = serde_json::from_value(reply["receipts"].clone()).unwrap();
        assert_eq!(known.len(), usize::from(after_commit));
        assert!(tracker.status().unresolved > 0);
        drop(router);
        let _root = f.close();
        let reopened =
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
        let rows = receipt_snapshot(&path);
        assert_eq!(
            rows.len(),
            before_receipts.len() + usize::from(after_commit)
        );
        assert!(before_receipts.iter().all(|r| rows.contains(r)));
        assert_eq!(
            reopened
                .engine
                .control_state()
                .unwrap()
                .committed_generation
                .get(),
            before.committed_generation.get() + u64::from(after_commit)
        );
        for receipt in known {
            assert!(
                rows.iter()
                    .any(|(_, bytes)| control::validate_receipt(bytes).unwrap().receipt == receipt)
            );
        }
    }
}

async fn create_purge_namespace(f: &Fixture) {
    f.storage
        .write()
        .unwrap()
        .create_native_namespace(
            &MutationContext {
                scope: control::MutationScope::Catalog,
                ..MutationContext::unkeyed(NamespaceId::new(1))
            },
            &crate::storage::native_catalog::NativeNamespaceInput {
                name: "purge-race".into(),
                embedding_dim: Some(4),
                embedding_profile_id: None,
                initial_stability: None,
                desired_retention: None,
                decay_rate_multiplier: None,
            },
            &crate::embedding::runtime::tests::profile(),
            &NativePolicyInputs::default(),
            &crate::time::ManualClock::new(1000),
            None,
        )
        .unwrap();
    f.completion().rebuild_all().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_peer_effect_and_purge_race_refuse_without_foreign_ack() {
    for purge in [false, true] {
        let (f, ids, clock) = fixture().await;
        if purge {
            create_purge_namespace(&f).await;
        }
        let before = f.storage.read().unwrap().control_state().unwrap();
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let (release, resume) = tokio::sync::oneshot::channel();
        *f.storage
            .read()
            .unwrap()
            .meta_store()
            .bounded_completion_pause
            .lock()
            .unwrap() = Some((entered, resume));
        let router = app(&f, clock.clone());
        let task = tokio::spawn(async move { request(&router, "{}").await });
        tokio::time::timeout(std::time::Duration::from_secs(10), waiting)
            .await
            .unwrap()
            .unwrap();
        let (own, foreign, pending, progress, after_peer) = {
            let mut engine = f.storage.write().unwrap();
            let own = engine.meta_store().pending_effects().unwrap()[0]
                .receipt
                .clone();
            let (foreign, progress) = if purge {
                let p = engine
                    .begin_native_purge_named_admitted(
                        "purge-race",
                        true,
                        None,
                        crate::daemon::protocol::MAX_MESSAGE_SIZE,
                    )
                    .unwrap()
                    .unwrap();
                (p.start_receipt.clone(), Some(p))
            } else {
                let peer = *ids
                    .iter()
                    .find(|id| engine.get_record(**id).unwrap().unwrap().phase == DecayPhase::Full)
                    .unwrap();
                let intent = engine
                    .meta_store()
                    .capture_native_decay(
                        peer,
                        NamespaceId::new(1),
                        &NativePolicyInputs::default(),
                        NOW,
                    )
                    .unwrap();
                let result = engine
                    .apply_native_decay(
                        &MutationContext::unkeyed(NamespaceId::new(1)),
                        &intent,
                        &NativePolicyInputs::default(),
                        None,
                    )
                    .unwrap();
                let crate::storage::native_maintenance::NativeDecayResult::Committed(receipt) =
                    result
                else {
                    panic!("actual peer must commit");
                };
                (receipt, None)
            };
            let pending = engine.meta_store().pending_effects().unwrap();
            (
                own,
                foreign,
                serde_json::to_vec(&pending).unwrap(),
                progress,
                control_snapshot(engine.db_path()),
            )
        };
        assert_eq!(
            after_peer.committed_generation.get(),
            before.committed_generation.get() + 2
        );
        release.send(()).unwrap();
        let (status, reply) = tokio::time::timeout(std::time::Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reply["error"], "SWEEP_UNAVAILABLE");
        assert_eq!(reply["commitState"], "committed");
        assert_eq!(reply["ready"], false);
        let known: Vec<WriteReceipt> = serde_json::from_value(reply["receipts"].clone()).unwrap();
        assert_eq!(known, vec![own]);
        assert!(!known.contains(&foreign));
        let engine = f.storage.read().unwrap();
        assert_eq!(
            control_snapshot(engine.db_path()).generation_ref(),
            after_peer.generation_ref()
        );
        assert_eq!(
            serde_json::to_vec(&engine.meta_store().pending_effects().unwrap()).unwrap(),
            pending
        );
        if purge {
            assert!(engine.meta_store().publication_state().is_err());
        } else {
            assert!(
                !engine
                    .meta_store()
                    .publication_state()
                    .unwrap()
                    .matches_canonical()
            );
        }
        if let Some(progress) = progress {
            assert_eq!(
                engine
                    .lookup_namespace_purge(
                        progress.store_id,
                        progress.namespace_id,
                        progress.start_receipt.operation_id
                    )
                    .unwrap(),
                Some(progress)
            );
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_both_formats_phase_and_delete_budget_abort_then_strict_reopen() {
    for native in [false, true] {
        let (f, ids, clock) = if native {
            fixture().await
        } else {
            legacy_fixture().await
        };
        let path = f.storage.read().unwrap().db_path().to_owned();
        let original_receipts = receipt_snapshot(&path);
        for next in [Some(DecayPhase::Summary), Some(DecayPhase::Ghost), None] {
            let before = canonical(&f);
            // A nonzero candidate allowance reaches the final transaction's
            // encoder, then rejects the complete real proof before commit.
            let bounded = app_with_limit(&f, clock.clone(), 2200);
            let (status, failure) = request_with_limit(&bounded, "{}", 2200).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(failure["error"], "SWEEP_RESPONSE_LIMIT");
            assert_eq!(failure["commitState"], "notCommitted");
            assert_eq!(failure["receipts"], json!([]));
            assert_eq!(canonical(&f), before);
            drop(bounded);

            let router = app(&f, clock.clone());
            let (status, _) = request(&router, "{}").await;
            assert_eq!(status, StatusCode::OK);
            for id in &ids {
                assert_eq!(f.record(*id).map(|r| r.phase), next);
            }
            assert!(
                f.storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .publication_state()
                    .unwrap()
                    .matches_canonical()
            );
            drop(router);
        }
        assert_eq!(clock.0.load(Ordering::SeqCst), 6);
        let rows = receipt_snapshot(&path);
        assert!(original_receipts.iter().all(|r| rows.contains(r)));
        let new: Vec<_> = rows
            .iter()
            .filter(|r| !original_receipts.contains(r))
            .map(|(_, b)| control::validate_receipt(b).unwrap())
            .collect();
        assert!(new.iter().all(|r| r.request_hash_version == 1));
        assert!(new.iter().any(|r| matches!(
            r.receipt.outcome,
            MutationOutcome::TextGenerationSelected { .. }
                | MutationOutcome::NativeTextGenerationSelected { .. }
        )));
        if native {
            assert_eq!(
                new.iter()
                    .filter(|r| matches!(
                        r.receipt.outcome,
                        MutationOutcome::NativeMemoryHardDeleted { .. }
                    ))
                    .count(),
                2
            );
        }
        let before = f
            .storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
        let _root = f.close();
        let reopened = if native {
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock)
                .unwrap()
                .engine
        } else {
            crate::storage::RedbStorageEngine::open(&path).unwrap()
        };
        assert_eq!(reopened.control_state().unwrap().generation_ref(), before);
        assert!(
            ids.iter()
                .all(|id| reopened.get_record(*id).unwrap().is_none())
        );
        assert_eq!(receipt_snapshot(&path), rows);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_compaction_both_formats_rejects_final_receipt_without_selection() {
    for native in [false, true] {
        let (f, _ids, clock) = if native {
            fixture().await
        } else {
            legacy_fixture().await
        };
        let path = f.storage.read().unwrap().db_path().to_owned();
        let before = canonical(&f);
        let admission = crate::storage::maintenance::ReceiptAdmission::new(2200).unwrap();
        let error = f
            .completion()
            .compact_text_with_work_bounded(None, Some(&admission))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            crate::storage::StorageError::ReceiptBudgetExceeded
        ));
        assert_eq!(canonical(&f), before);
        let (_, known, _) = admission.snapshot().unwrap();
        assert!(known.is_empty());
        let rows = receipt_snapshot(&path);
        let selection = f
            .storage
            .read()
            .unwrap()
            .meta_store()
            .text_selection()
            .unwrap();
        let _root = f.close();
        let reopened = if native {
            NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock)
                .unwrap()
                .engine
        } else {
            crate::storage::RedbStorageEngine::open(&path).unwrap()
        };
        assert_eq!(reopened.meta_store().text_selection().unwrap(), selection);
        assert_eq!(receipt_snapshot(&path), rows);
        // Prepared unselected files/IDs may exist; no claim of zero allocation.
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_compaction_completion_loss_retains_physical_receipt_and_reopens() {
    let (f, _, clock) = fixture().await;
    let path = f.storage.read().unwrap().db_path().to_owned();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let old = receipt_snapshot(&path);
    // Initial completion, two phase publications, pre-compaction completion,
    // then worker loss in post-selection completion. Not a lost selection ACK.
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .bounded_completion_failure_nth
        .store(5, Ordering::SeqCst);
    let router = app(&f, clock.clone());
    let (status, body) = request(&router, "{}").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["commitState"], "unknown");
    assert!(body["operationId"].is_null());
    assert!(body.get("receipt").is_none());
    let known: Vec<WriteReceipt> = serde_json::from_value(body["receipts"].clone()).unwrap();
    assert_eq!(known.len(), 3);
    assert!(matches!(
        known[2].outcome,
        MutationOutcome::NativeTextGenerationSelected { .. }
    ));
    assert!(
        known
            .windows(2)
            .all(|w| w[0].committed_generation < w[1].committed_generation)
    );
    drop(router);
    let _root = f.close();
    let reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
    assert_eq!(
        reopened
            .engine
            .control_state()
            .unwrap()
            .committed_generation
            .get(),
        before.committed_generation.get() + 3
    );
    let rows = receipt_snapshot(&path);
    assert_eq!(rows.len(), old.len() + 3);
    assert!(old.iter().all(|r| rows.contains(r)));
    for receipt in known {
        assert!(
            rows.iter()
                .any(|(_, b)| control::validate_receipt(b).unwrap().receipt == receipt)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_legacy_strength_and_bridged_delete_abort_all_staged_effects() {
    let (f, ids, _) = legacy_fixture().await;
    let third = f.insert_memory().await;
    let edge = |source, target| crate::storage::PersistedEdge {
        source,
        target,
        edge_type: crate::model::EdgeType::Associative,
        weight: 0.5,
        auto_created: true,
        created_at: 1000,
    };
    f.storage
        .read()
        .unwrap()
        .put_edges_with_effects(&[edge(ids[0], ids[1]), edge(ids[0], third)])
        .unwrap()
        .unwrap();
    f.completion().rebuild_all().await.unwrap();
    let path = f.storage.read().unwrap().db_path().to_owned();
    for deletion in [false, true] {
        let before = canonical(&f);
        let denied = crate::storage::maintenance::ReceiptAdmission::new(2200).unwrap();
        let apply = |admission: &crate::storage::maintenance::ReceiptAdmission| {
            let mut engine = f.storage.write().unwrap();
            if deletion {
                engine
                    .delete_memory_with_edges_with_origin_work_bounded(
                        ids[0],
                        &[edge(ids[1], third)],
                        None,
                        1,
                        Some(admission),
                    )
                    .map(|result| result.unwrap().receipt)
            } else {
                let record = engine.get_record(ids[0]).unwrap().unwrap();
                let ns = engine
                    .get_namespace(NamespaceId::new(record.namespace_id))
                    .unwrap()
                    .unwrap();
                engine
                    .apply_legacy_decay_with_work_bounded(
                        &MutationContext::unkeyed(ns.id),
                        ids[0],
                        &record,
                        &ns,
                        record.phase,
                        0.5,
                        0.5,
                        None,
                        Some(admission),
                    )
                    .map(|result| {
                        let crate::storage::maintenance::DecayMutationResult::Committed(receipt) =
                            result
                        else {
                            panic!("actual patch required");
                        };
                        receipt
                    })
            }
        };
        assert!(matches!(
            apply(&denied),
            Err(crate::storage::StorageError::ReceiptBudgetExceeded)
        ));
        assert_eq!(canonical(&f), before);
        let allowed = crate::storage::maintenance::ReceiptAdmission::new(
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .unwrap();
        let receipt = apply(&allowed).unwrap();
        allowed.record(&receipt).unwrap();
        f.completion()
            .complete_with_admission(None, Some(&allowed))
            .await
            .unwrap();
        assert!(
            receipt_snapshot(&path)
                .iter()
                .any(|(_, b)| control::validate_receipt(b).unwrap().receipt == receipt)
        );
    }
    assert!(f.record(ids[0]).is_none());
    assert_eq!(
        f.storage.read().unwrap().load_all_edges().unwrap(),
        vec![edge(ids[1], third)]
    );
    let before = canonical(&f);
    let _root = f.close();
    let engine = crate::storage::RedbStorageEngine::open(&path).unwrap();
    let reopened = Fixture::from_open_engine(_root, engine);
    assert_eq!(canonical(&reopened), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_later_known_publication_failure_retains_ordered_full_receipts() {
    let (f, _, clock) = fixture().await;
    let path = f.storage.read().unwrap().db_path().to_owned();
    let before = f.storage.read().unwrap().control_state().unwrap();
    let old = receipt_snapshot(&path);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .vector_publication_failure_nth
        .store(2, Ordering::SeqCst);
    let router = app(&f, clock.clone());
    let (status, body) = request(&router, "{}").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["commitState"], "committed");
    assert_eq!(body["error"], "INDEX_NOT_READY");
    assert_eq!(body["ready"], false);
    let known: Vec<WriteReceipt> = serde_json::from_value(body["receipts"].clone()).unwrap();
    assert_eq!(known.len(), 2);
    assert_eq!(
        serde_json::from_value::<WriteReceipt>(body["receipt"].clone()).unwrap(),
        known[1]
    );
    // The singular operationId denotes uncertain work; known identity is in
    // the complete receipt/receipts fields and is never fabricated here.
    assert!(body["operationId"].is_null());
    assert!(known[0].committed_generation < known[1].committed_generation);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .ensure_available()
            .is_err()
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    drop(router);
    let _root = f.close();
    let reopened =
        NativeLifecycleSession::open(&path, NativePolicyInputs::default(), clock).unwrap();
    assert_eq!(
        reopened
            .engine
            .control_state()
            .unwrap()
            .committed_generation
            .get(),
        before.committed_generation.get() + 2
    );
    let rows = receipt_snapshot(&path);
    assert_eq!(rows.len(), old.len() + 2);
    assert!(old.iter().all(|r| rows.contains(r)));
    for receipt in known {
        assert!(
            rows.iter()
                .any(|(_, b)| control::validate_receipt(b).unwrap().receipt == receipt)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_preexisting_pending_or_purge_has_no_receipt_authority() {
    for purge in [false, true] {
        let (f, ids, clock) = fixture().await;
        if purge {
            create_purge_namespace(&f).await;
        }
        let path = f.storage.read().unwrap().db_path().to_owned();
        let progress = {
            let mut engine = f.storage.write().unwrap();
            if purge {
                engine
                    .begin_native_purge_named_admitted(
                        "purge-race",
                        true,
                        None,
                        crate::daemon::protocol::MAX_MESSAGE_SIZE,
                    )
                    .unwrap()
            } else {
                let policy = NativePolicyInputs::default();
                let intent = engine
                    .meta_store()
                    .capture_native_decay(ids[0], NamespaceId::new(1), &policy, NOW)
                    .unwrap();
                assert!(matches!(
                    engine
                        .apply_native_decay(
                            &MutationContext::unkeyed(NamespaceId::new(1)),
                            &intent,
                            &policy,
                            None
                        )
                        .unwrap(),
                    crate::storage::native_maintenance::NativeDecayResult::Committed(_)
                ));
                None
            }
        };
        let before = control_snapshot(&path).generation_ref();
        let pending = serde_json::to_vec(
            &f.storage
                .read()
                .unwrap()
                .meta_store()
                .pending_effects()
                .unwrap(),
        )
        .unwrap();
        let receipts = receipt_snapshot(&path);
        let router = app(&f, clock.clone());
        let (status, body) = request(&router, "{}").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["commitState"], "notCommitted");
        assert_eq!(body["receipts"], json!([]));
        assert!(body["operationId"].is_null());
        assert_eq!(body["ready"], false);
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        assert_eq!(receipt_snapshot(&path), receipts);
        let engine = f.storage.read().unwrap();
        assert_eq!(control_snapshot(&path).generation_ref(), before);
        assert_eq!(
            serde_json::to_vec(&engine.meta_store().pending_effects().unwrap()).unwrap(),
            pending
        );
        if let Some(p) = progress {
            assert_eq!(
                engine
                    .lookup_namespace_purge(
                        p.store_id,
                        p.namespace_id,
                        p.start_receipt.operation_id
                    )
                    .unwrap(),
                Some(p)
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_sweep_initial_broken_unpublished_view_rejects_without_canonical_work() {
    let f = Fixture::new();
    let id = f.insert_memory().await;
    assert!(
        !f.storage
            .read()
            .unwrap()
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    let before = canonical(&f);
    *f.vector_index.write().await =
        crate::search::VectorIndexOwner::uniform(crate::test_support::DIM + 1);
    let clock = Arc::new(SweepClock(AtomicUsize::new(0)));
    let router = app(&f, clock.clone());
    let (status, body) = request(&router, "{}").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "SWEEP_UNAVAILABLE");
    assert_eq!(body["commitState"], "notCommitted");
    assert_eq!(body["ready"], false);
    assert_eq!(body["receipts"], json!([]));
    assert!(body["operationId"].is_null());
    assert!(body.get("receipt").is_none());
    assert_eq!(canonical(&f), before);
    assert_eq!(f.record(id).unwrap().phase, DecayPhase::Full);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    let status = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .status();
    assert_eq!((status.pending, status.unresolved), (0, 0));
}
