//! Actual HTTP compatibility restriction: supplied clock overrides reject
//! before work; ordinary manual sweeps retain their shared configured clock.
use super::*;
use crate::model::{DecayPhase, NamespaceId};
use crate::storage::control::MutationContext;
use crate::storage::native_lifecycle::{NativeLifecycleSession, NativePolicyInputs};
use crate::storage::schema_migration::{
    MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountClock(AtomicUsize);
impl crate::time::Clock for CountClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        1000
    }
}
fn completion(f: &Fixture) -> crate::derived_completion::DerivedCompletion {
    crate::derived_completion::DerivedCompletion {
        storage: f.storage.clone(),
        cache: f.cache.clone(),
        graph: f.graph.clone(),
        vectors: f.vector_index.clone(),
        fts: f.fts_index.clone(),
        entities: f.entity_index.clone(),
    }
}
async fn fixture(native: bool) -> (Fixture, crate::model::MemoryId, Arc<CountClock>) {
    let f = Fixture::new();
    let input = crate::storage::insertion::InsertMemory {
        id: None,
        summary: "public sweep clock probe".into(),
        full_text: Some("retained source text".into()),
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
    let id = crate::storage::insertion::inserted_id(&receipt).unwrap();
    completion(&f).rebuild_all().await.unwrap();
    let clock = Arc::new(CountClock(AtomicUsize::new(0)));
    if !native {
        return (f, id, clock);
    }
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
    let session =
        NativeLifecycleSession::open(&destination, NativePolicyInputs::default(), clock.clone())
            .unwrap();
    let f = Fixture::from_open_engine(root, session.engine);
    completion(&f).rebuild_all().await.unwrap();
    (f, id, clock)
}
fn sweep_app(f: &Fixture, clock: Arc<CountClock>) -> Router {
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
        .with_clock(clock.clone())
        .with_native_lifecycle_policy(NativePolicyInputs::default()),
    );
    crate::api::routes::router(state, &Default::default())
}
async fn request(native: bool, override_time: bool) {
    let (f, id, clock) = fixture(native).await;
    let app = sweep_app(&f, clock.clone());
    let before = f.record(id).unwrap();
    let generation = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    let body = if override_time {
        json!({"asOf":1000+500_i64*86_400_000})
    } else {
        json!({})
    };
    let (status, reply) = post(&app, "/decay/sweep", body).await;
    let after = f.record(id).unwrap();
    let after_g = f
        .storage
        .read()
        .unwrap()
        .control_state()
        .unwrap()
        .committed_generation;
    println!(
        "native={native} override={override_time} status={status} calls={} phase={:?}->{:?} G={generation:?}->{after_g:?} text={}->{} response={reply}",
        clock.0.load(Ordering::SeqCst),
        before.phase,
        after.phase,
        before.text_length,
        after.text_length
    );
    if override_time {
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "public override must reject before work: {reply}"
        );
        assert_eq!(reply["error"], "BAD_REQUEST");
        assert_eq!(reply["field"], "asOf");
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
        assert_eq!(before.encode().unwrap(), after.encode().unwrap());
        assert_eq!(generation, after_g);
    } else {
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        assert_eq!(after.phase, DecayPhase::Full);
        assert_eq!(before.text_length, after.text_length);
        assert_eq!(before.last_accessed_at, after.last_accessed_at);
    }
    let work = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .status();
    assert_eq!(work.pending, 0);
    assert_eq!(work.unresolved, 0);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_sweep_clock_override_legacy_rejects_before_work() {
    request(false, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_sweep_clock_override_native_rejects_before_work() {
    request(true, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_sweep_clock_without_override_uses_actual_clock_once() {
    request(false, false).await;
    request(true, false).await;
}

fn canonical_snapshot(f: &Fixture) -> Vec<u8> {
    use redb::ReadableTable;
    let engine = f.storage.read().unwrap();
    let (_copy, metadata) =
        crate::storage::validation::inspect_database_copy(&engine.db_path().join("meta.db"), false)
            .unwrap();
    let read = metadata.begin_read().unwrap();
    let mut tables = Vec::new();
    for definition in [
        crate::storage::control::RECEIPTS,
        crate::storage::metadata::removal_ledger::LEDGER,
    ] {
        tables.push(
            read.open_table(definition)
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
    let rows = engine
        .scan_all()
        .unwrap()
        .into_iter()
        .map(|(id, r)| (id, r.encode().unwrap()))
        .collect::<Vec<_>>();
    serde_json::to_vec(&(
        engine.control_state().unwrap(),
        engine.list_namespaces().unwrap(),
        rows,
        engine.load_all_edges().unwrap(),
        tables,
    ))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_sweep_clock_presence_matrix_rejects_without_accepting_work() {
    for native in [false, true] {
        let (f, _id, clock) = fixture(native).await;
        let app = sweep_app(&f, clock.clone());
        let tracker = f.storage.read().unwrap().meta_store().work_tracker();
        let marker = tracker.begin().unwrap();
        let previous = marker.id();
        marker.finish();
        let before = canonical_snapshot(&f);
        let status = tracker.status();
        let min = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
        let max = chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis();
        for field in ["asOf", "as_of"] {
            for value in [
                Value::Null,
                json!(1000),
                json!(0),
                json!(-1),
                json!(min),
                json!(max),
                json!(u64::MAX),
                json!(true),
                json!(false),
                json!("1000"),
                json!(1.5),
                json!([]),
                json!({}),
            ] {
                let mut body = json!({"unrelatedLegacyField":true});
                body[field] = value;
                let (status, reply) = post(&app, "/decay/sweep", body).await;
                assert_eq!(
                    status,
                    StatusCode::BAD_REQUEST,
                    "native={native} {field}: {reply}"
                );
                assert_eq!(reply["error"], "BAD_REQUEST");
                assert_eq!(reply["field"], "asOf");
                assert_eq!(
                    reply["message"],
                    "Public sweeps use the configured clock; remove asOf/as_of"
                );
                assert_eq!(canonical_snapshot(&f), before);
            }
        }
        let (status_both, _) = post(&app, "/decay/sweep", json!({"asOf":null,"as_of":1000})).await;
        assert_eq!(status_both, StatusCode::BAD_REQUEST);
        // Send literal JSON so duplicate keys are not collapsed by the fixture.
        for raw in [
            r#"{"asOf":1000,"asOf":null}"#,
            r#"{"asOf":null,"asOf":1000}"#,
            r#"{"as_of":{},"as_of":false}"#,
            r#"{"as_of":null,"asOf":1000}"#,
        ] {
            let (code, reply) = raw_sweep(&app, raw).await;
            assert_eq!(code, StatusCode::BAD_REQUEST, "{raw}: {reply}");
            assert_eq!(reply["error"], "BAD_REQUEST");
            assert_eq!(reply["field"], "asOf");
            assert_eq!(canonical_snapshot(&f), before);
        }
        for raw in [
            "",
            "null",
            "[]",
            "42",
            "true",
            r#""not an object""#,
            r#"{"asOf":"#,
        ] {
            let (code, reply) = raw_sweep(&app, raw).await;
            assert!(code.is_client_error(), "{raw}: {reply}");
            assert_eq!(canonical_snapshot(&f), before);
        }
        assert_eq!(tracker.status(), status);
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
        let next = tracker.begin().unwrap();
        assert_eq!(next.id(), previous + 1);
        next.finish();
        // Closed work admission must not turn input rejection into unavailable.
        let freeze = tracker
            .close_and_drain(std::time::Instant::now() + std::time::Duration::from_secs(5))
            .unwrap();
        let (closed, _) = post(&app, "/decay/sweep", json!({"asOf":null})).await;
        assert_eq!(closed, StatusCode::BAD_REQUEST);
        assert_eq!(canonical_snapshot(&f), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
        drop(freeze);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_sweep_clock_unrelated_api_one_fields_keep_ordinary_clock() {
    for native in [false, true] {
        let (f, id, clock) = fixture(native).await;
        let app = sweep_app(&f, clock.clone());
        let before = canonical_snapshot(&f);
        for body in [
            json!({}),
            json!({"unrelated":null}),
            json!({"unrelated":{"nested":"value"}}),
        ] {
            let calls = clock.0.load(Ordering::SeqCst);
            let (status, reply) = post(&app, "/decay/sweep", body).await;
            assert_eq!(status, StatusCode::OK, "{reply}");
            assert_eq!(clock.0.load(Ordering::SeqCst), calls + 1);
            assert_eq!(f.record(id).unwrap().phase, DecayPhase::Full);
            assert_eq!(canonical_snapshot(&f), before);
        }
    }
}

async fn raw_sweep(app: &Router, body: &str) -> (StatusCode, Value) {
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
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    // Axum's malformed-JSON extraction error is existing plain text.
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, value)
}
