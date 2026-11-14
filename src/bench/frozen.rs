//! Same-core isolated control owner. No process-global clock or second registry.
use std::sync::Arc;
use std::time::Instant;

use crate::derived_completion::DerivedCompletion;
use crate::storage::{Readiness, RedbStorageEngine, StorageError, metadata::MetadataFreeze};
use crate::time::{Clock, ManualClock};

/// The actor receives `core()` only. Runtime sources stay private to controls.
/// Pins lifecycle time and canonical allocation seed 7 for this legacy-v1 track.
/// This is not control over every RNG, behavioral cache, or query tie.
pub struct OwnedEvaluator {
    pub(super) core: Arc<crate::Recalld>,
    pub(super) controls: FrozenControls,
    pub(super) fixture_ancestors: Vec<std::path::PathBuf>,
}

impl OwnedEvaluator {
    pub async fn new(config: crate::config::RecalldConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(
            config.decay.disable_sweep,
            "isolated controls require explicit sweeps"
        );
        let clock = Arc::new(ManualClock::new(1_767_225_600_000));
        let mut seed = [0; 32];
        seed[..8].copy_from_slice(&7_u64.to_be_bytes());
        let core = Arc::new(
            crate::Recalld::with_runtime_sources(
                config,
                clock.clone(),
                Box::new(crate::storage::ids::SeededIds::new(seed)),
            )
            .await?,
        );
        let controls = FrozenControls::new(core.derived_owner(), clock);
        Ok(Self {
            core,
            controls,
            fixture_ancestors: Vec::new(),
        })
    }

    pub fn core(&self) -> &Arc<crate::Recalld> {
        &self.core
    }

    pub(crate) async fn inspect(
        &self,
        deadline: Instant,
    ) -> Result<serde_json::Value, StorageError> {
        let clock = self.controls.clock.clone();
        self.controls
            .capture(deadline, move |engine, frozen, _| {
                let ids = engine.meta_store().frozen_id_state(frozen)?;
                Ok(
                    serde_json::json!({"committed":frozen.generation(), "logicalHash":null,
                "runtimeState":{"nowMillis":clock.now_millis(),"allocation":ids},
                "snapshot":false}),
                )
            })
            .await
    }

    pub(crate) async fn advance(
        &self,
        delta_ms: i64,
        deadline: Instant,
    ) -> Result<i64, StorageError> {
        self.controls.advance(delta_ms, deadline).await
    }
}

pub(super) struct FrozenControls {
    owner: DerivedCompletion,
    pub(super) clock: Arc<ManualClock>,
    // A controller permit, not a work ticket or a readiness registry. The join
    // task owns this until the blocking worker exits, even if its caller leaves.
    busy: Arc<tokio::sync::Mutex<()>>,
}

fn unavailable(message: impl ToString) -> StorageError {
    StorageError::RecoveryRequired(message.to_string())
}

impl FrozenControls {
    pub(super) fn new(owner: DerivedCompletion, clock: Arc<ManualClock>) -> Self {
        Self {
            owner,
            clock,
            busy: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub(super) async fn capture<T: Send + 'static>(
        &self,
        deadline: Instant,
        capture: impl FnOnce(
            &RedbStorageEngine,
            &MetadataFreeze<'_>,
            Readiness,
        ) -> Result<T, StorageError>
        + Send
        + 'static,
    ) -> Result<T, StorageError> {
        let permit = self
            .busy
            .clone()
            .try_lock_owned()
            .map_err(|_| unavailable("prior frozen control worker has not joined"))?;
        let owner = self.owner.clone();
        let join = tokio::spawn(async move {
            let _permit = permit;
            owner.with_frozen_core(None, deadline, capture).await
        });
        // Dropping this JoinHandle detaches the join task, not its busy permit.
        // New clock/ID/cache controls cannot race a timed-out blocking capture.
        tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), join)
            .await
            .map_err(|_| unavailable("control deadline expired; worker may still be joining"))?
            .map_err(unavailable)?
    }

    async fn advance(&self, delta_ms: i64, deadline: Instant) -> Result<i64, StorageError> {
        if delta_ms < 0 {
            return Err(unavailable("negative clock advance"));
        }
        let clock = self.clock.clone();
        self.capture(deadline, move |_, _, _| {
            let next = clock
                .now_millis()
                .checked_add(delta_ms)
                .ok_or_else(|| unavailable("clock advance overflow"))?;
            clock.advance_to(next).map_err(unavailable)?;
            Ok(next)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageEngine;
    use crate::test_support::Fixture;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    async fn controls() -> (Fixture, FrozenControls) {
        let root = tempfile::tempdir().unwrap();
        let mut engine = RedbStorageEngine::open_with_ids(
            root.path(),
            false,
            Box::new(crate::storage::ids::SeededIds::new([7; 32])),
        )
        .unwrap();
        let mut ns = crate::model::NamespaceConfig::default_namespace(0);
        ns.embedding_dim = 4;
        engine.create_namespace(&ns).unwrap();
        let fx = Fixture::from_open_engine(root, engine);
        let owner = DerivedCompletion {
            storage: fx.storage.clone(),
            cache: fx.cache.clone(),
            graph: fx.graph.clone(),
            vectors: fx.vector_index.clone(),
            fts: fx.fts_index.clone(),
            entities: fx.entity_index.clone(),
        };
        owner.complete().await.unwrap();
        (
            fx,
            FrozenControls::new(owner, Arc::new(ManualClock::new(1000))),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn capture_timeout_keeps_clock_and_all_leases_frozen_until_worker_joins() {
        let (fx, controls) = controls().await;
        let controls = Arc::new(controls);
        let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let published = Arc::new(AtomicBool::new(false));
        let seen = published.clone();
        let capturing = controls.clone();
        let clock = controls.clock.clone();
        let task = tokio::spawn(async move {
            let result = capturing
                .capture(
                    Instant::now() + Duration::from_millis(500),
                    move |_, _, _| {
                        started.send(()).unwrap();
                        released.recv_timeout(Duration::from_secs(10)).unwrap();
                        assert_eq!(clock.now_millis(), 1000);
                        Ok("staged, never yet published")
                    },
                )
                .await;
            if result.is_ok() {
                seen.store(true, Ordering::SeqCst);
            }
            result
        });
        entered.await.unwrap();
        assert!(task.await.unwrap().is_err());
        assert!(!tracker.status().admission_open);
        assert_eq!(
            tracker.status().pending,
            0,
            "controller must not drain itself"
        );
        assert!(tracker.begin().is_err());
        assert!(fx.graph.try_write().is_err());
        assert!(fx.vector_index.try_write().is_err());
        assert!(fx.fts_index.try_lock().is_err());
        assert!(fx.entity_index.try_write().is_err());
        assert!(fx.storage.try_write().is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        assert!(
            controls
                .advance(50, deadline)
                .await
                .unwrap_err()
                .to_string()
                .contains("not joined")
        );
        assert!(
            controls
                .capture(deadline, |_, _, _| Ok(()))
                .await
                .unwrap_err()
                .to_string()
                .contains("not joined")
        );
        assert_eq!(controls.clock.now_millis(), 1000);
        assert!(!published.load(Ordering::SeqCst));
        release.send(()).unwrap();
        // Acquiring this exact permit proves the detached coordinator has joined
        // the worker, rather than observing a speculative zero pending count.
        let joined = tokio::time::timeout(Duration::from_secs(5), controls.busy.lock())
            .await
            .unwrap();
        assert!(tracker.status().admission_open);
        assert!(!published.load(Ordering::SeqCst));
        drop(joined);
        assert_eq!(controls.advance(50, deadline).await.unwrap(), 1050);
        assert_eq!(controls.clock.now_millis(), 1050);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn advance_drains_accepted_descendants_and_rejects_bad_time_without_change() {
        let (fx, controls) = controls().await;
        let controls = Arc::new(controls);
        let tracker = fx.storage.read().unwrap().meta_store().work_tracker();
        let parent = tracker.begin().unwrap();
        let advancing = controls.clone();
        let task = tokio::spawn(async move {
            advancing
                .advance(10, Instant::now() + Duration::from_secs(5))
                .await
        });
        let closing = tracker.clone();
        tokio::task::spawn_blocking(move || closing.wait_until_closed())
            .await
            .unwrap();
        assert_eq!(controls.clock.now_millis(), 1000);
        let child = parent.child().unwrap();
        parent.finish();
        assert!(!task.is_finished());
        child.finish();
        assert_eq!(task.await.unwrap().unwrap(), 1010);
        assert!(
            controls
                .advance(-1, Instant::now() + Duration::from_secs(5))
                .await
                .is_err()
        );
        assert!(
            controls
                .advance(i64::MAX, Instant::now() + Duration::from_secs(5))
                .await
                .is_err()
        );
        assert_eq!(controls.clock.now_millis(), 1010);
    }
    #[tokio::test]
    async fn owned_ids_batch_reservation_replay_uses_owner_cursor_and_frozen_inspection() {
        use crate::storage::batch::{BatchError, BatchItemPlan, BatchKind};
        use crate::storage::control::{MutationContext, MutationScope};
        use crate::storage::ids::{IdKind, IdSource, SeededIds};
        let (fx, controls) = controls().await;
        let state = {
            let engine = fx.storage.read().unwrap();
            let meta = engine.meta_store();
            let expected =
                SeededIds::from_state(meta.id_source().checkpoint().unwrap().unwrap()).unwrap();
            let first = expected.uuid(IdKind::Operation).unwrap();
            let second = expected.uuid(IdKind::Operation).unwrap();
            let parent = expected.uuid(IdKind::Operation).unwrap();
            let plans = [
                BatchItemPlan::Invalid {
                    request_hash: [1; 32],
                    error: BatchError::new("INVALID_INPUT"),
                },
                BatchItemPlan::Invalid {
                    request_hash: [2; 32],
                    error: BatchError::new("INVALID_INPUT"),
                },
            ];
            let ctx = MutationContext {
                store_id: Some(engine.control_state().unwrap().store_id),
                scope: MutationScope::Catalog,
                idempotency_key: Some("scoped-batch".into()),
                expected_generation: None,
                contract_version: 1,
            };
            let result = meta.reserve_batch(&ctx, BatchKind::Insert, &plans).unwrap();
            assert_eq!(result.items[0].operation_id, first);
            assert_eq!(result.items[1].operation_id, second);
            assert_eq!(result.reservation_receipt.operation_id, parent);
            let replay = meta.reserve_batch(&ctx, BatchKind::Insert, &plans).unwrap();
            assert!(replay.reservation_receipt.replayed);
            assert_eq!(replay.items, result.items);
            assert_eq!(
                meta.id_source().checkpoint().unwrap(),
                expected.checkpoint().unwrap()
            );
            expected.checkpoint().unwrap()
        };
        controls.owner.complete().await.unwrap();
        let captured = controls
            .capture(
                Instant::now() + Duration::from_secs(5),
                |engine, frozen, _| engine.meta_store().frozen_id_state(frozen),
            )
            .await
            .unwrap();
        assert_eq!(captured, state);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actual_frozen_canonical_stage_is_validated_and_late_capture_is_discarded() {
        let (fx, controls) = controls().await;
        let controls = Arc::new(controls);
        let id = fx
            .insert_memory_in(crate::model::NamespaceId::new(1), &["entity/frozen"])
            .await;
        controls.owner.complete().await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let source_files = {
            let engine = fx.storage.read().unwrap();
            let mut names = vec![
                "meta.db".to_owned(),
                "edges.db".into(),
                "fulltext.dat".into(),
                "default/vectors.dat".into(),
            ];
            if let Some(selected) = engine.meta_store().text_selection().unwrap() {
                names.push(selected.filename);
            }
            names
                .into_iter()
                .map(|name| {
                    let path = engine.db_path().join(name);
                    let bytes = std::fs::read(&path).unwrap();
                    (path, bytes)
                })
                .collect::<Vec<_>>()
        };
        let first = controls
            .capture(deadline, move |engine, frozen, _| {
                engine.stage_frozen_canonical(frozen, deadline)
            })
            .await
            .unwrap();
        let original_generation = first.generation;
        let original_hash = first.logical_sha256;
        let snapshot =
            crate::storage::offline_snapshot::OfflineSnapshot::open(&first.path()).unwrap();
        assert_eq!(snapshot.control().generation_ref(), original_generation);
        assert!(snapshot.record(id).unwrap().is_some());
        drop(snapshot);
        let second = controls
            .capture(deadline, move |engine, frozen, _| {
                engine.stage_frozen_canonical(frozen, deadline)
            })
            .await
            .unwrap();
        assert_eq!(second.generation, original_generation);
        assert_eq!(second.logical_sha256, original_hash);
        {
            let database = redb::Database::open(second.path().join("meta.db")).unwrap();
            let tx = database.begin_write().unwrap();
            tx.open_table(redb::TableDefinition::<&str, &str>::new(
                "unknown_required_state",
            ))
            .unwrap()
            .insert("key", "value")
            .unwrap();
            tx.commit().unwrap();
        }
        let snapshot =
            crate::storage::offline_snapshot::OfflineSnapshot::open(&second.path()).unwrap();
        assert!(crate::storage::logical_export::isolated_legacy_hash(&snapshot, deadline).is_err());
        drop(snapshot);
        // A source-specific corruption of the disposable stage never changes
        // the live owner or makes a second capture inherit unvalidated bytes.
        std::fs::write(second.path().join("meta.db"), b"not a database").unwrap();
        assert!(crate::storage::offline_snapshot::OfflineSnapshot::open(&second.path()).is_err());
        assert!(fx.storage.read().unwrap().get_record(id).unwrap().is_some());
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let capturing = controls.clone();
        let task = tokio::spawn(async move {
            let deadline = Instant::now() + Duration::from_secs(5);
            capturing
                .capture(deadline, move |engine, frozen, _| {
                    let stage = engine.stage_frozen_canonical(frozen, deadline)?;
                    started.send(stage.directory.path().to_owned()).unwrap();
                    released.recv_timeout(Duration::from_secs(10)).unwrap();
                    Ok(stage)
                })
                .await
        });
        let path = entered.await.unwrap();
        assert!(path.exists());
        assert!(task.await.unwrap().is_err());
        assert!(
            controls
                .advance(1, Instant::now() + Duration::from_secs(5))
                .await
                .is_err()
        );
        release.send(()).unwrap();
        let _joined = tokio::time::timeout(Duration::from_secs(5), controls.busy.lock())
            .await
            .unwrap();
        assert!(!path.exists(), "timed-out canonical stage must not escape");
        for (path, expected) in source_files {
            assert_eq!(std::fs::read(path).unwrap(), expected);
        }
        eprintln!(
            "validated canonical stage at {:?}, logical hash {:?}; late stage discarded",
            original_generation, original_hash
        );
    }
}
