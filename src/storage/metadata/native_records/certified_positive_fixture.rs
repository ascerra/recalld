//! Explicit positive-only setup. The common migrated/uncertified fixture and
//! genuine historical artifacts remain unchanged. Every vector is regenerated
//! from its actual retained document; prior vector bytes are never the provider.
use super::*;

pub(super) fn certified_fixture(root: &Path) -> Data {
    certify_positive(root, fixture(root))
}
pub(super) fn certified_fixture_with_seed(
    root: &Path,
    seed: impl FnOnce(&mut RedbStorageEngine),
) -> Data {
    certify_positive(root, fixture_with_seed(root, seed))
}
pub(super) fn certify_positive(root: &Path, mut data: Data) -> Data {
    struct RecipeSpy {
        profile: crate::model::schema::EmbeddingProfile,
        seen: std::sync::Mutex<Vec<String>>,
    }
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for RecipeSpy {
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            &self.profile.model_id
        }
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            Some(self.profile.clone())
        }
        async fn embed(
            &self,
            document: &str,
        ) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            self.seen.lock().unwrap().push(document.to_owned());
            // Synthetic fixture geometry: every canonical parent remains collinear
            // with the declared subsequent [2,0,0,0] supplied producer inputs.
            Ok(vec![1., 0., 0., 0.])
        }
    }
    let snapshot = crate::storage::offline_snapshot::OfflineSnapshot::open(&data.path).unwrap();
    let source = snapshot.control().generation_ref();
    let mut documents = Vec::new();
    snapshot
        .for_each_record(|record| {
            if record.record.namespace_id == data.ns.get() && record.embedding.is_some() {
                documents.push(
                    crate::storage::vector_certification::retained_document(
                        &record.record,
                        record.full_text.as_deref(),
                    )?
                    .text,
                );
            }
            Ok(())
        })
        .unwrap();
    drop(snapshot);
    let request = crate::storage::vector_certification::ReembedVectors {
        source,
        namespace_id: data.ns,
        namespace_name: "native".into(),
        profile_id: data.profile.profile_id().unwrap(),
        context: MutationContext {
            store_id: Some(source.store_id),
            expected_generation: Some(source.generation),
            scope: MutationScope::Namespace(data.ns),
            contract_version: 1,
            idempotency_key: Some("explicit-positive-fixture-reembedding".into()),
        },
    };
    let provider = RecipeSpy {
        profile: data.profile.clone(),
        seen: Default::default(),
    };
    let target = root.join("certified-positive");
    // The helper is shared by sync and async tests. The real operation executes
    // on its own runtime thread; no nested Tokio runtime or source writer escape.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(crate::storage::vector_certification::reembed_fresh(
                        &data.path, &target, &request, &provider,
                    ))
                    .unwrap();
            })
            .join()
            .unwrap();
    });
    assert_eq!(*provider.seen.lock().unwrap(), documents);
    crate::storage::offline_snapshot::OfflineSnapshot::open(&target).unwrap();
    data.path = target;
    data
}

/// Positive metadata-page setup: certify retained Full text before real phase loss.
/// The common uncertified/old-Ghost fixtures remain unchanged.
pub(super) fn certified_ghost_fixture(root: &Path) -> Data {
    let data = certified_fixture(root);
    let mut session = NativeLifecycleSession::open(
        &data.path,
        NativePolicyInputs::default(),
        Arc::new(crate::time::ManualClock::new(10_000 * 86_400_000)),
    )
    .unwrap();
    let before = session.engine.get_record(data.parent).unwrap().unwrap();
    assert_eq!(before.phase, DecayPhase::Full);
    let vector = before.schema.as_ref().unwrap().vector.clone();
    assert!(vector.is_some());
    for (key, phase) in [
        ("page-parent-summary", DecayPhase::Summary),
        ("page-parent-ghost", DecayPhase::Ghost),
    ] {
        let result = session
            .maintain(&ctx(&session, &data, key), data.parent)
            .unwrap();
        let crate::storage::native_maintenance::NativeDecayResult::Committed(receipt) = result
        else {
            panic!("parent phase loss did not commit");
        };
        assert!(matches!(
            receipt.outcome,
            MutationOutcome::NativePhaseMaterialized { .. }
        ));
        let row = session.engine.get_record(data.parent).unwrap().unwrap();
        assert_eq!(row.phase, phase);
        assert_eq!(row.schema.as_ref().unwrap().vector, vector);
    }
    drop(session);
    data
}
