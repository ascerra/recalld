//! Prepared API2 query -> actual public API1 feedback, not public2 activation.
use super::adopted_public::{no_pending, receipts};
use super::*;
use crate::storage::control::{self, GenerationRef, StoredReceipt};
use crate::time::ManualClock;
use tower::ServiceExt;

const T0: i64 = 1000;
const DAY: i64 = 86_400_000;
struct CountingClock {
    clock: ManualClock,
    calls: AtomicUsize,
}
impl Clock for CountingClock {
    fn now_millis(&self) -> i64 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.clock.now_millis()
    }
}
impl CountingClock {
    fn new() -> Self {
        Self {
            clock: ManualClock::new(T0),
            calls: AtomicUsize::new(0),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

async fn imported(export: &Path) -> (Fixture, Arc<CountingClock>, McpBridge, crate::api::AppState) {
    let root = tempfile::tempdir().unwrap();
    std::fs::remove_dir(root.path()).unwrap();
    crate::storage::logical_export::import_store(export, root.path()).unwrap();
    inspect_schema(root.path()).unwrap();
    let clock = Arc::new(CountingClock::new());
    let session =
        NativeLifecycleSession::open(root.path(), NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    let owner = completion(&f);
    owner.rebuild_all().await.unwrap();
    let at = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .generation_ref();
    let ready = owner
        .readiness_barrier(
            at,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        )
        .await
        .unwrap();
    assert!(ready.ready);
    no_pending(&f);
    let (bridge, state) =
        public_adapters_with_query(&f, clock.clone(), NativePolicyInputs::default(), true);
    (f, clock, bridge, state)
}

fn canonical_bytes(f: &Fixture) -> Vec<(PathBuf, Vec<u8>)> {
    // Same owned-fixture file selection as coherent read-view tests. No offline
    // snapshot/open is attempted while this engine owns the store.
    let storage = f.storage.read().unwrap();
    let mut paths: Vec<_> = ["meta.db", "edges.db", "fulltext.dat"]
        .into_iter()
        .map(|n| f.path().join(n))
        .collect();
    for ns in storage.list_namespaces().unwrap() {
        paths.push(f.path().join(ns.name).join("vectors.dat"));
    }
    if let Some(selected) = storage.meta_store().text_selection().unwrap() {
        paths.push(f.path().join(selected.filename));
    }
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|p| {
            let b = std::fs::read(&p).unwrap();
            (p, b)
        })
        .collect()
}

fn retained_payload(f: &Fixture, id: MemoryId) -> Vec<u8> {
    let storage = f.storage.read().unwrap();
    let row = storage.get_record(id).unwrap().unwrap();
    control::encode(&(
        &row.summary,
        &row.tags,
        &row.schema.as_ref().unwrap().labels,
        storage
            .get_text(crate::storage::text::TextRef {
                file_offset: row.text_offset,
                length: row.text_length,
            })
            .unwrap(),
        storage
            .get_vector(NamespaceId::new(row.namespace_id), row.vector_slot)
            .unwrap(),
    ))
    .unwrap()
}

async fn query(
    mcp: bool,
    bridge: &McpBridge,
    state: &crate::api::AppState,
    access: bool,
    empty: bool,
) -> Value {
    let mut body = json!({"namespace":"native","queryMode":"metadata_only","minStrength":0.,"limit":1,"depth":0,"recordAccess":access});
    if empty {
        body["tags"] = json!(["absent-query-tag"]);
    }
    if mcp {
        body["contractVersion"] = json!(2);
        let result =
            crate::mcp::tools::prepared_query(bridge.search.as_ref(), "wrong-default", &body).await;
        assert_eq!(result.is_error, Some(false), "{result:?}");
        let value = Value::Object(result.structured_content.unwrap());
        let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
        assert_eq!(serde_json::from_str::<Value>(text).unwrap(), value);
        value
    } else {
        let response = crate::api::search_v2(state.search.as_ref(), "wrong-default", &body).await;
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(status.as_u16(), 200, "{value}");
        value
    }
}

async fn feedback(mcp: bool, bridge: &McpBridge, state: &crate::api::AppState, id: MemoryId) {
    if mcp {
        let result = crate::mcp::tools::dispatch_tool(
            bridge,
            "reinforce_memory",
            json!({"id":id,"quality":3}),
        )
        .await;
        assert_ne!(result.is_error, Some(true), "{result:?}");
    } else {
        let response = crate::api::router(state.clone(), &Default::default())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/memories/{id}/reinforce"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"quality":3}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(status.as_u16(), 200, "{value}");
        assert!(value.get("data").is_some());
        assert!(
            value.get("receipt").is_none(),
            "API1 retains its old success shape"
        );
    }
}

fn original_context(
    f: &Fixture,
    ns: NamespaceId,
    id: MemoryId,
    receipt: &WriteReceipt,
    api: u32,
    quality: Option<u8>,
) {
    let stored: StoredReceipt = receipts(f)
        .values()
        .map(|bytes| control::validate_receipt(bytes).unwrap())
        .find(|r| r.receipt.operation_id == receipt.operation_id)
        .unwrap();
    assert_eq!(stored.receipt, *receipt);
    let mut ctx = MutationContext::unkeyed(ns);
    ctx.contract_version = api;
    let scope = crate::storage::native_lifecycle::LifecycleScope {
        namespace_id: ns,
        namespace_name: "native".into(),
        source: None,
    };
    let kind = if quality.is_some() {
        AccessKind::ManualReinforcement
    } else {
        AccessKind::DirectRetrieval
    };
    let fields = control::encode(&(scope, vec![(id, kind)], quality)).unwrap();
    let operation = if quality.is_some() {
        "native_feedback"
    } else {
        "native_exposure"
    };
    assert_eq!(
        stored.request_hash,
        control::request_hash(&ctx, operation, &[&fields])
    );
    ctx.contract_version = if api == 1 { 2 } else { 1 };
    assert_ne!(
        stored.request_hash,
        control::request_hash(&ctx, operation, &[&fields])
    );
}

fn reopen(
    f: Fixture,
    bridge: McpBridge,
    state: crate::api::AppState,
    clock: Arc<CountingClock>,
    ids: &[MemoryId],
) {
    let expected: Vec<_> = ids
        .iter()
        .map(|id| f.record(*id).unwrap().encode().unwrap())
        .collect();
    let history = receipts(&f);
    let payloads: Vec<_> = ids.iter().map(|id| retained_payload(&f, *id)).collect();
    drop((bridge, state));
    let root = f.close();
    inspect_schema(root.path()).unwrap();
    let session =
        NativeLifecycleSession::open(root.path(), NativePolicyInputs::default(), clock).unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    assert_eq!(receipts(&f), history);
    for (id, expected) in ids.iter().zip(expected) {
        assert_eq!(f.record(*id).unwrap().encode().unwrap(), expected);
    }
    for (id, expected) in ids.iter().zip(payloads) {
        assert_eq!(retained_payload(&f, *id), expected);
    }
    f.close();
}

async fn chronology(version: LifecycleVersion, mcp: bool) {
    let root = tempfile::tempdir().unwrap();
    let mut second = None;
    let mut d = fixture_with_policy(
        root.path(),
        |engine| {
            let ns = engine
                .list_namespaces()
                .unwrap()
                .into_iter()
                .find(|n| n.name == "native")
                .unwrap()
                .id;
            let inserted = engine
                .insert_memory_with_receipt(
                    &MutationContext::unkeyed(ns),
                    &crate::storage::insertion::InsertMemory {
                        id: None,
                        summary: "second eligible".into(),
                        full_text: Some("retained second payload".into()),
                        tags: vec![],
                        initial_stability: None,
                        created_at: Some(T0),
                        embedding: Some(vec![1., 0., 0., 0.]),
                        parent_id: None,
                        supersedes: None,
                    },
                    &[],
                    T0,
                )
                .unwrap();
            second = Some(crate::storage::insertion::inserted_id(&inserted).unwrap());
        },
        false,
        None,
        false,
    );
    if version == LifecycleVersion::LegacyAccessV1 {
        d.path = root.path().join("schema");
    }
    crate::test_support::explicitly_adopt_inherited_policy(&d.path, d.ns, 1., 2000);
    let ids = [d.parent, second.unwrap()];
    let export = root.path().join("day0.jsonl");
    crate::storage::logical_export::export_store(&d.path, &export).unwrap();
    let (f, clock, bridge, state) = imported(&export).await;
    let (control, control_clock, control_bridge, control_state) = imported(&export).await;
    for id in ids {
        assert_eq!(
            f.record(id).unwrap().encode().unwrap(),
            control.record(id).unwrap().encode().unwrap()
        );
    }
    let initial_receipts = receipts(&f);
    assert_eq!(receipts(&control), initial_receipts);
    clock.clock.advance_to(T0 + 7 * DAY).unwrap();
    f.storage.write().unwrap().sync().unwrap();
    let before = canonical_bytes(&f);
    let calls = clock.calls();
    let read = query(mcp, &bridge, &state, false, false).await;
    assert_eq!(clock.calls(), calls + 1);
    assert_eq!(canonical_bytes(&f), before);
    assert!(read.get("receipt").is_none());
    assert!(read.get("observationReceipts").is_none());
    no_pending(&f);
    let hits = read["data"]["results"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    let id: MemoryId = serde_json::from_value(hits[0]["memoryId"].clone()).unwrap();
    assert!(ids.contains(&id));
    let omitted = ids.into_iter().find(|other| *other != id).unwrap();
    let omitted_before = f.record(omitted).unwrap().encode().unwrap();
    let payload = retained_payload(&f, id);
    let old = f.record(id).unwrap();
    let old_lifecycle = &old.schema.as_ref().unwrap().lifecycle;
    assert_eq!(old_lifecycle.lifecycle_version, version);
    assert_eq!(old_lifecycle.observed_at, T0);
    assert_eq!(old_lifecycle.reinforced_at, None);
    let calls = clock.calls();
    let seen = query(mcp, &bridge, &state, true, false).await;
    assert_eq!(
        clock.calls(),
        calls + 1,
        "recorder uses the captured query clock"
    );
    assert_eq!(seen["data"]["results"][0]["memoryId"], hits[0]["memoryId"]);
    assert_eq!(seen["readGeneration"], read["readGeneration"]);
    let at: GenerationRef = serde_json::from_value(seen["readGeneration"].clone()).unwrap();
    let receipt: WriteReceipt = serde_json::from_value(seen["receipt"].clone()).unwrap();
    assert_eq!(seen["observationReceipts"].as_array().unwrap().len(), 1);
    assert_eq!(seen["observationReceipts"][0], seen["receipt"]);
    assert_eq!(at.store_id, receipt.store_id);
    assert!(receipt.committed_generation > at.generation);
    assert_eq!(seen["readiness"]["ready"], true);
    assert_eq!(seen["readiness"]["pendingWork"], 0);
    no_pending(&f);
    original_context(&f, d.ns, id, &receipt, 2, None);
    let MutationOutcome::LifecycleExposureRecorded { transition } = &receipt.outcome else {
        panic!("{receipt:?}")
    };
    assert_eq!(transition.sampled_at, T0 + 7 * DAY);
    assert_eq!(transition.capture_generation, at);
    assert_eq!(transition.records.len(), 1);
    assert_eq!(transition.records[0].memory_id, id);
    let observed = f.record(id).unwrap();
    let t = &observed.schema.as_ref().unwrap().lifecycle;
    assert_eq!(t.retrieved_at, Some(T0 + 7 * DAY));
    assert_eq!(t.reinforced_at, old_lifecycle.reinforced_at);
    assert_eq!(t.observed_at, old_lifecycle.observed_at);
    assert_eq!(t.phase_changed_at, old_lifecycle.phase_changed_at);
    assert_eq!(
        t.record_generation,
        old_lifecycle.record_generation.next().unwrap()
    );
    assert_eq!(
        t.decay_anchor_at,
        if version == LifecycleVersion::LegacyAccessV1 {
            T0 + 7 * DAY
        } else {
            old_lifecycle.decay_anchor_at
        }
    );
    assert_eq!(
        (observed.stability, observed.phase),
        (old.stability, old.phase)
    );
    assert_eq!(retained_payload(&f, id), payload);
    assert_eq!(observed.access_history.len(), old.access_history.len() + 1);
    assert_eq!(f.record(omitted).unwrap().encode().unwrap(), omitted_before);
    let empty_before = canonical_bytes(&f);
    let empty = query(mcp, &bridge, &state, true, true).await;
    assert!(empty["data"]["results"].as_array().unwrap().is_empty());
    assert!(empty.get("receipt").is_none());
    assert!(empty.get("observationReceipts").is_none());
    assert_eq!(canonical_bytes(&f), empty_before);
    no_pending(&f);
    for (k, v) in &initial_receipts {
        assert_eq!(receipts(&f).get(k), Some(v));
    }

    let mut after = Vec::new();
    for (fixture, c, b, s) in [
        (&f, &clock, &bridge, &state),
        (&control, &control_clock, &control_bridge, &control_state),
    ] {
        c.clock.advance_to(T0 + 8 * DAY).unwrap();
        let before = fixture.record(id).unwrap();
        let ns = fixture
            .storage
            .read()
            .unwrap()
            .get_namespace(d.ns)
            .unwrap()
            .unwrap();
        let policy = crate::decay::lifecycle::resolve_policy(&ns, &Default::default(), 1.).unwrap();
        let expected =
            crate::decay::lifecycle::plan_feedback(&before, &policy, T0 + 8 * DAY, 3).unwrap();
        let calls = c.calls();
        feedback(mcp, b, s, id).await;
        assert_eq!(c.calls(), calls + 1);
        let row = fixture.record(id).unwrap();
        assert_eq!(row.stability, expected.stability);
        let lifecycle = &row.schema.as_ref().unwrap().lifecycle;
        assert_eq!(lifecycle.reinforced_at, Some(T0 + 8 * DAY));
        assert_eq!(lifecycle.decay_anchor_at, T0 + 8 * DAY);
        assert_eq!(lifecycle.observed_at, T0);
        assert_eq!(row.phase, before.phase);
        assert_eq!(retained_payload(fixture, id), payload);
        assert_eq!(
            row.schema.as_ref().unwrap().labels,
            before.schema.as_ref().unwrap().labels
        );
        no_pending(fixture);
        let stored: StoredReceipt = receipts(fixture)
            .values()
            .map(|v| control::validate_receipt(v).unwrap())
            .filter(|r| {
                matches!(
                    r.receipt.outcome,
                    MutationOutcome::LifecycleFeedbackApplied { .. }
                )
            })
            .max_by_key(|r| r.receipt.committed_generation)
            .unwrap();
        original_context(fixture, d.ns, id, &stored.receipt, 1, Some(3));
        for (k, v) in &initial_receipts {
            assert_eq!(receipts(fixture).get(k), Some(v));
        }
        after.push(row);
    }
    if version == LifecycleVersion::ExplicitFeedbackV2 {
        assert_eq!(after[0].stability, after[1].stability);
        assert_eq!(after[0].difficulty, after[1].difficulty);
    } else {
        assert_ne!(
            after[0].stability, after[1].stability,
            "v1 retains the day7 reset"
        );
    }
    assert_eq!(
        after[0].schema.as_ref().unwrap().lifecycle.retrieved_at,
        Some(T0 + 7 * DAY)
    );
    assert_eq!(
        after[1].schema.as_ref().unwrap().lifecycle.retrieved_at,
        None
    );
    assert_eq!(
        control.record(omitted).unwrap().encode().unwrap(),
        omitted_before
    );
    reopen(f, bridge, state, clock, &ids);
    reopen(control, control_bridge, control_state, control_clock, &ids);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_feedback_chronology_prepared_mcp_v2() {
    chronology(LifecycleVersion::ExplicitFeedbackV2, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_feedback_chronology_prepared_rest_v2() {
    chronology(LifecycleVersion::ExplicitFeedbackV2, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_feedback_chronology_prepared_mcp_v1() {
    chronology(LifecycleVersion::LegacyAccessV1, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_feedback_chronology_prepared_rest_v1() {
    chronology(LifecycleVersion::LegacyAccessV1, false).await;
}
