//! Actual HTTP probe of intentionally supported old API1 asOf behavior versus
//! the agreed A3 production-clock boundary. No runtime correction in this probe.
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
async fn request(native: bool, override_time: bool) {
    let (f, id, clock) = fixture(native).await;
    let (mut state, _bridge) = fixture_state(&f);
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
    let app = crate::api::routes::router(state, &Default::default());
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
