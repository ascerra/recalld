//! Actual post-TX owned certification publication failures, distinct from lost ACKs.
use super::*;
use crate::model::AccessKind;
use crate::search::{RetrievalSemantics, VersionMode};
use redb::ReadableTable;

async fn postcommit_failure(nth: usize, origin: u32, query: bool) {
    let root = tempfile::tempdir().unwrap();
    let mut second = None;
    let d = certified_fixture_with_seed(root.path(), |engine| {
        let mut namespace = NamespaceConfig::default_namespace(0);
        namespace.name = "second".into();
        namespace.embedding_dim = 4;
        namespace.decay_rate_multiplier = Some(1.);
        let ns = engine.create_namespace(&namespace).unwrap();
        let input = crate::storage::insertion::InsertMemory {
            id: None,
            summary: "second namespace retained payload".into(),
            full_text: Some("second full text".into()),
            tags: vec![],
            initial_stability: None,
            created_at: Some(1000),
            embedding: Some(vec![1., 0., 0., 0.]),
            parent_id: None,
            supersedes: None,
        };
        let receipt = engine
            .insert_memory_with_receipt(&MutationContext::unkeyed(ns), &input, &[], 1000)
            .unwrap();
        second = Some((
            ns,
            crate::storage::insertion::inserted_id(&receipt).unwrap(),
        ));
    });
    let (second_ns, second_id) = second.unwrap();
    assert!(d.ns < second_ns);
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let session = open(&d, clock.clone());
    let f = Fixture::from_open_engine(root, session.engine);
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    let initial = [event_count(&f, d.parent), event_count(&f, second_id)];
    let before = f.storage.read().unwrap().control_state().unwrap();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .vector_publication_failure_nth
        .store(nth, Ordering::SeqCst);
    let calls = clock.0.load(Ordering::SeqCst);
    let error = if query {
        let request: SearchQuery = serde_json::from_value(json!({
            "namespace":"native", "queryMode":"metadata_only", "graphDepth":0,
            "filter":{"minStrength":0.0}, "recordAccess":true
        }))
        .unwrap();
        // A page is never returned when its mandatory exposure fails, even though
        // the exposure transaction committed. This is the real QueryEngine path.
        observation(
            engine
                .search_checked_with_origin_work(
                    request,
                    RetrievalSemantics::V2(VersionMode::Current),
                    false,
                    None,
                    origin,
                )
                .await
                .err()
                .expect("failed observation must reject the whole page"),
        )
    } else {
        owner
            .record_native_exposures_with_work(
                vec![
                    (second_id, AccessKind::DirectRetrieval),
                    (d.parent, AccessKind::DirectRetrieval),
                    (d.parent, AccessKind::AssociativeRetrieval),
                ],
                None,
                86_400_000,
                NativePolicyInputs::default(),
                None,
                origin,
            )
            .await
            .unwrap_err()
    };
    assert_eq!(clock.0.load(Ordering::SeqCst), calls + usize::from(query));
    assert!(
        owner.complete().await.is_err(),
        "poisoned owner cannot heal itself"
    );
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .ensure_available()
            .is_err()
    );
    assert!(tracker.status().unresolved > 0);
    assert!(
        owner
            .readiness_barrier(
                before.generation_ref(),
                std::time::Instant::now() + std::time::Duration::from_millis(25),
            )
            .await
            .is_err(),
        "no false ready even for an older target"
    );
    drop((api, bridge, engine, owner));
    let root = f.close();
    // Strict production open validates the durable proof, not the poisoned handle.
    let reopened = open(&d, clock);
    let after = reopened.metadata().control_state().unwrap();
    assert_eq!(after.store_id, before.store_id);
    assert_eq!(
        after.committed_generation.get(),
        before.committed_generation.get() + nth as u64
    );
    for (index, id) in [d.parent, second_id].into_iter().enumerate() {
        assert_eq!(
            reopened
                .engine
                .get_record(id)
                .unwrap()
                .unwrap()
                .access_history
                .len(),
            initial[index] + usize::from(index < nth)
        );
    }
    let mut durable = {
        let read = reopened.metadata().db.begin_read().unwrap();
        let table = read.open_table(control::RECEIPTS).unwrap();
        table
            .iter()
            .unwrap()
            .map(|row| control::validate_receipt(row.unwrap().1.value()).unwrap())
            .filter(|stored| {
                stored.receipt.store_id == before.store_id
                    && stored.receipt.committed_generation > before.committed_generation
            })
            .collect::<Vec<_>>()
    };
    durable.sort_by_key(|stored| stored.receipt.committed_generation);
    assert_eq!(durable.len(), nth);
    for (index, stored) in durable.iter().enumerate() {
        let (namespace_id, namespace_name, id) = if index == 0 {
            (d.ns, "native", d.parent)
        } else {
            (second_ns, "second", second_id)
        };
        let scope = LifecycleScope {
            namespace_id,
            namespace_name: namespace_name.into(),
            source: None,
        };
        let semantic =
            control::encode(&(scope, &[(id, AccessKind::DirectRetrieval)], None::<u8>)).unwrap();
        let mut context = MutationContext::unkeyed(namespace_id);
        context.contract_version = origin;
        assert_eq!(
            stored.request_hash,
            control::request_hash(&context, "native_exposure", &[&semantic])
        );
        assert!(matches!(
            stored.receipt.outcome,
            MutationOutcome::LifecycleExposureRecorded { .. }
        ));
        assert_eq!(
            reopened
                .metadata()
                .lookup_operation(after.store_id, context.scope, stored.receipt.operation_id)
                .unwrap()
                .unwrap(),
            stored.receipt
        );
    }
    let expected: Vec<_> = durable.into_iter().map(|stored| stored.receipt).collect();
    drop((reopened, root));
    // These are the RED assertions: every commit above already survived strict reopen.
    assert_eq!(
        error.receipts, expected,
        "lost current postcommit receipt: {error:?}"
    );
    assert_nested_mapping(&expected);
    assert_failed_wire(error.clone(), &expected, "committed");
    assert_eq!(error.commit_state, "committed");
    assert!(!error.ready);
    assert!(error.unknown_operation_id.is_none());
    assert!(
        error
            .message
            .contains("injected vector validation publication failure")
    );
    assert!(
        !error.message.contains("receipt="),
        "structured receipts must not be repeated in message"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_first_group_api1() {
    postcommit_failure(1, 1, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_first_group_api2() {
    postcommit_failure(1, 2, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_later_group_api1() {
    postcommit_failure(2, 1, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_later_group_api2() {
    postcommit_failure(2, 2, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_whole_query_page_api1() {
    postcommit_failure(1, 1, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_whole_query_page_api2() {
    postcommit_failure(1, 2, true).await;
}

// Typed nested-error checks use the actual durable exposure receipts above.
// They do not claim the certification publisher emits a nested unknown failure.
fn assert_nested_mapping(receipts: &[WriteReceipt]) {
    let current = receipts.last().unwrap();
    let prior = &receipts[..receipts.len() - 1];
    let unknown_id = uuid::Uuid::new_v4();
    for unknown_worker in [false, true] {
        let leaf = if unknown_worker {
            StorageError::WorkerOutcomeUnknown("lost worker".into())
        } else {
            StorageError::CommitOutcomeUnknown {
                operation_id: unknown_id,
                source: redb::CommitError::Storage(redb::StorageError::Io(std::io::Error::other(
                    "unknown ACK",
                ))),
            }
        };
        let error = ObservationError::storage(
            StorageError::CommittedMutation {
                receipt: Box::new(current.clone()),
                source: Box::new(StorageError::CommittedMutation {
                    receipt: Box::new(current.clone()),
                    source: Box::new(leaf),
                }),
            },
            prior.to_vec(),
        );
        assert_eq!(
            error.receipts, receipts,
            "dedupe exact wrapper, preserve prior order"
        );
        assert_eq!(error.commit_state, "unknown");
        assert_eq!(
            error.unknown_operation_id,
            (!unknown_worker).then_some(unknown_id)
        );
        assert!(!error.ready);
        assert!(!error.message.contains("receipt="));
        assert_failed_wire(error, receipts, "unknown");
    }
    let plain =
        ObservationError::storage(StorageError::InvalidMutation("precommit".into()), vec![]);
    assert_eq!(plain.commit_state, "notCommitted");
    assert!(plain.receipts.is_empty());
    assert!(plain.unknown_operation_id.is_none());
    let worker = ObservationError::worker_failed("cancelled worker");
    assert_eq!(worker.commit_state, "unknown");
    assert!(worker.receipts.is_empty());
    assert!(worker.unknown_operation_id.is_none());
}

fn assert_failed_wire(error: ObservationError, receipts: &[WriteReceipt], state: &str) {
    let reply =
        crate::serialization::query_response::QueryReply::failure(SearchError::Observation(error));
    assert_eq!(reply.http_status, 503);
    assert!(reply.envelope.data.is_none());
    assert!(reply.envelope.readiness.is_none());
    assert_eq!(reply.observation_receipts, receipts);
    assert_eq!(
        reply.envelope.receipt.as_ref(),
        (receipts.len() == 1).then(|| &receipts[0])
    );
    let value = serde_json::to_value(&reply).unwrap();
    assert_eq!(value["error"]["commitState"], state);
    assert_eq!(value["error"]["retryable"], false);
    assert_eq!(reply.mcp().is_error, Some(true));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_keyed_core_replays_after_reopen_without_time_or_history() {
    for origin in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let d = certified_fixture(root.path());
        let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
        let session = open(&d, clock.clone());
        let intent = session
            .capture_exposure(scope(&d), &[(d.parent, AccessKind::DirectRetrieval)])
            .unwrap();
        let mut context = ctx(&session, &d, "postcommit-keyed-core");
        context.contract_version = origin;
        session
            .metadata()
            .vector_publication_failure_nth
            .store(1, Ordering::SeqCst);
        let error = ObservationError::storage(
            session.record_exposure(&context, &intent).unwrap_err(),
            vec![],
        );
        assert_eq!(error.commit_state, "committed");
        assert_eq!(error.receipts.len(), 1);
        drop(session);
        let reopened = open(&d, clock.clone());
        let generation = reopened
            .metadata()
            .control_state()
            .unwrap()
            .generation_ref();
        let record = reopened
            .engine
            .get_record(d.parent)
            .unwrap()
            .unwrap()
            .encode()
            .unwrap();
        let calls = clock.0.load(Ordering::SeqCst);
        let replay = reopened
            .record_exposure(&context, &intent)
            .unwrap()
            .unwrap();
        let mut expected = error.receipts[0].clone();
        expected.replayed = true;
        assert_eq!(replay, expected);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        assert_eq!(
            reopened
                .metadata()
                .control_state()
                .unwrap()
                .generation_ref(),
            generation
        );
        assert_eq!(
            reopened
                .engine
                .get_record(d.parent)
                .unwrap()
                .unwrap()
                .encode()
                .unwrap(),
            record
        );
        context.contract_version = 3 - origin;
        assert!(matches!(
            reopened.record_exposure(&context, &intent),
            Err(StorageError::IdempotencyConflict)
        ));
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        drop((reopened, root));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_exposure_postcommit_prepared_query_returns_only_failure_and_full_receipt() {
    let (f, d, clock, id) = setup();
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let (api, bridge, engine) = readers_with_exposure(&f, clock.clone(), true);
    let before = event_count(&f, id);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .vector_publication_failure_nth
        .store(1, Ordering::SeqCst);
    let request = crate::serialization::query::QueryRequest::from_mcp(
        &json!({
            "contractVersion":2, "namespace":"native", "queryMode":"metadata_only",
            "recordAccess":true, "minStrength":0.0,
        }),
        "native",
    )
    .unwrap();
    let reply = engine.query_contract(request).await;
    assert_eq!(reply.http_status, 503);
    assert!(reply.envelope.data.is_none());
    assert!(reply.envelope.readiness.is_none());
    assert_eq!(reply.observation_receipts.len(), 1);
    assert_eq!(
        reply.envelope.receipt.as_ref(),
        reply.observation_receipts.first()
    );
    assert_eq!(reply.mcp().is_error, Some(true));
    let value = serde_json::to_value(&reply).unwrap();
    assert_eq!(value["error"]["commitState"], "committed");
    assert_eq!(value["error"]["retryable"], false);
    assert!(owner.complete().await.is_err());
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .work_tracker()
            .status()
            .unresolved
            > 0
    );
    drop((api, bridge, engine, owner));
    let root = f.close();
    let reopened = open(&d, clock);
    let receipt = &reply.observation_receipts[0];
    assert_eq!(
        reopened
            .metadata()
            .lookup_operation(
                receipt.store_id,
                MutationScope::Namespace(d.ns),
                receipt.operation_id
            )
            .unwrap()
            .as_ref(),
        Some(receipt)
    );
    assert_eq!(
        reopened
            .engine
            .get_record(id)
            .unwrap()
            .unwrap()
            .access_history
            .len(),
        before + 1
    );
    drop((reopened, root));
}
