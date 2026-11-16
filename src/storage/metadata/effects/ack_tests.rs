//! Test-only counting at actual batch entry, including nested guard/validator work.
use super::*;
use std::cell::Cell;
use std::sync::mpsc::Sender;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AckWork {
    pub validation_calls: usize,
    pub effect_rows: usize,
    pub commit_attempts: usize,
    pub commit_successes: usize,
}

thread_local! {
    static WORK: Cell<Option<AckWork>> = const { Cell::new(None) };
}

pub(super) fn count(update: impl FnOnce(&mut AckWork)) {
    WORK.with(|slot| {
        if let Some(mut work) = slot.get() {
            update(&mut work);
            slot.set(Some(work));
        }
    });
}

pub(super) struct Probe(Option<Sender<AckWork>>);
impl Probe {
    pub fn start(store: &MetadataStore) -> Self {
        let sender = store.ack_work_probe.lock().unwrap().take();
        if sender.is_some() {
            WORK.with(|slot| {
                assert!(slot.replace(Some(AckWork::default())).is_none());
            });
        }
        Self(sender)
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let work = WORK.with(|slot| slot.replace(None).unwrap());
            let _ = sender.send(work);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_ack_actual_rebuild_has_linear_validation_work() {
    use crate::storage::{StorageEngine, control::MutationContext, insertion::InsertMemory};
    let fixture = crate::test_support::Fixture::new();
    let n = 32;
    let (send, receive) = std::sync::mpsc::channel();
    let before = {
        let mut storage = fixture.storage.write().unwrap();
        for i in 0..n {
            let input = InsertMemory {
                id: None,
                summary: format!("ack fixture {i}"),
                full_text: None,
                tags: vec![],
                initial_stability: Some(1.),
                created_at: Some(123),
                embedding: None,
                parent_id: None,
                supersedes: None,
            };
            storage
                .insert_memory_with_receipt(
                    &MutationContext::unkeyed(NamespaceId::new(1)),
                    &input,
                    &[1., 0., 0., 0.],
                    123,
                )
                .unwrap();
        }
        let pending = storage.meta_store().pending_effects().unwrap();
        assert_eq!(pending.len(), n);
        assert!(pending.iter().all(|effect| effect.canonical_applied));
        *storage.meta_store().ack_work_probe.lock().unwrap() = Some(send);
        storage.control_state().unwrap().generation_ref()
    };
    crate::derived_completion::DerivedCompletion {
        storage: fixture.storage.clone(),
        cache: fixture.cache.clone(),
        graph: fixture.graph.clone(),
        vectors: fixture.vector_index.clone(),
        fts: fixture.fts_index.clone(),
        entities: fixture.entity_index.clone(),
    }
    .rebuild_all()
    .await
    .unwrap();
    let work = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    {
        let storage = fixture.storage.read().unwrap();
        assert_eq!(storage.control_state().unwrap().generation_ref(), before);
        assert!(storage.meta_store().pending_effects().unwrap().is_empty());
        assert!(
            storage
                .meta_store()
                .publication_state()
                .unwrap()
                .matches_canonical()
        );
        assert_eq!(storage.scan_all().unwrap().len(), n);
    }
    eprintln!("STARTUP_ACK_WORK backlog={n} {work:?}");
    assert!(
        work.effect_rows <= 2 * n,
        "actual ACK repeats full backlog validation: {work:?}"
    );
    assert!(work.validation_calls <= 2, "{work:?}");
    assert_eq!(work.commit_successes, 1, "{work:?}");
}
