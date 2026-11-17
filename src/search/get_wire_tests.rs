// Actual point consumers using the owned storage/completion fixture.
fn point_request(id: MemoryId, access: bool) -> crate::serialization::get::GetRequest {
    crate::serialization::get::GetRequest::from_mcp(&serde_json::json!({
        "contractVersion": 2, "id": id.to_string(), "recordAccess": access,
    }))
    .unwrap()
}

/// Genuine old-consumer failure retained as an opt-in experiment. This calls
/// the unchanged all-view getter, not the new prepared coordinator. Its binary
/// is attributed to the draft source that includes this probe, not to main19bd.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "BEFORE probe: old point getter still requires unrelated derived views"]
async fn prepared_get_before_old_consumer_accepts_canonical_point_with_stale_vector() {
    let f = Fixture::new();
    let owner = owner(&f);
    let id = seed(&owner).await;
    let engine = payload_engine(&f, &owner);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .invalidate_derived_view(crate::storage::publication::DerivedView::Vectors);
    let result = engine.prepare_coherent_get(id, None, false, None).await;
    assert!(
        result.is_ok(),
        "canonical point read is overrestricted: {:?}",
        result.err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_false_ignores_unused_derived_stamps_and_preserves_bytes() {
    let f = Fixture::new();
    let owner = owner(&f);
    let id = seed(&owner).await;
    let engine = payload_engine(&f, &owner);
    {
        let storage = f.storage.read().unwrap();
        for view in crate::storage::publication::REQUIRED_VIEWS {
            storage.meta_store().invalidate_derived_view(view);
        }
    }
    let before = bytes(&f);
    let old = engine.prepare_coherent_get(id, None, false, None).await;
    assert!(old.is_err(), "legacy all-view wrapper remains unchanged");
    let reply = engine
        .get_contract(
            point_request(id, false),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    let data = reply.envelope.data.unwrap();
    assert_eq!(data.memory.id, id);
    assert_eq!(data.memory.full_text.as_deref(), Some("fixture full text"));
    assert_eq!(data.evaluated_at, 123);
    assert!(reply.observation_receipts.is_empty());
    assert!(reply.envelope.readiness.is_none());
    assert_eq!(bytes(&f), before);
    let storage = f.storage.read().unwrap();
    assert!(
        !storage
            .meta_store()
            .publication_state()
            .unwrap()
            .matches_canonical()
    );
    assert_eq!(storage.meta_store().work_tracker().status().pending, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_canonical_pending_rejects_before_clock_or_observation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct CountClock(Arc<AtomicUsize>);
    impl crate::time::Clock for CountClock {
        fn now_millis(&self) -> i64 {
            self.0.fetch_add(1, Ordering::SeqCst);
            123
        }
    }
    let f = Fixture::new();
    let owner = owner(&f);
    let id = seed(&owner).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut engine = payload_engine(&f, &owner);
    engine.clock = Arc::new(CountClock(calls.clone()));
    let before = bytes(&f);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .canonical_pending
        .store(true, Ordering::SeqCst);
    for access in [false, true] {
        let reply = engine
            .get_contract(
                point_request(id, access),
                crate::daemon::protocol::MAX_MESSAGE_SIZE,
            )
            .await;
        assert_eq!(reply.http_status, 503, "{reply:?}");
        assert!(reply.envelope.data.is_none());
        assert_eq!(
            reply.envelope.error.unwrap().commit_state,
            crate::serialization::contract::CommitState::NotCommitted
        );
        assert!(reply.observation_receipts.is_empty());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // Re-enable only this synthetic admission control before byte inventory.
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .canonical_pending
        .store(false, Ordering::SeqCst);
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_canonical_applied_outbox_does_not_block_plain_copy() {
    let f = Fixture::new();
    let owner = owner(&f);
    let engine = payload_engine(&f, &owner);
    let id = fixed_id(9822);
    // Use the real commit-first writer, stopping before derived completion.
    let receipt = f
        .storage
        .write()
        .unwrap()
        .insert_memory_with_receipt(
            &MutationContext::unkeyed(NamespaceId::new(1)),
            &InsertMemory {
                id: Some(id),
                summary: "pending publication".into(),
                full_text: Some("canonical text while derived views are dirty".into()),
                tags: vec![],
                initial_stability: None,
                created_at: Some(100),
                embedding: Some(vec![1.0, 0.0, 0.0, 0.0]),
                parent_id: None,
                supersedes: None,
            },
            &[],
            100,
        )
        .unwrap();
    {
        let storage = f.storage.read().unwrap();
        let pending = storage.meta_store().pending_effects().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].receipt, receipt);
        assert!(pending[0].canonical_applied);
        assert!(storage.meta_store().needs_effects_rebuild());
        assert!(storage.meta_store().effects_stamp().is_err());
    }
    let before = bytes(&f);
    let reply = engine
        .get_contract(
            point_request(id, false),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(
        reply.envelope.data.unwrap().memory.full_text.as_deref(),
        Some("canonical text while derived views are dirty")
    );
    assert_eq!(
        reply.envelope.read_generation.unwrap().generation,
        receipt.committed_generation
    );
    assert!(reply.envelope.readiness.is_none() && reply.observation_receipts.is_empty());
    assert_eq!(bytes(&f), before);
    assert_eq!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .len(),
        1
    );
    owner.complete().await.unwrap();
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .pending_effects()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_direct_mcp_rest_share_captured_payload_and_scope() {
    let f = Fixture::new();
    let owner = owner(&f);
    let id = seed(&owner).await;
    let engine = Arc::new(payload_engine(&f, &owner));
    let provider = Arc::new(crate::embedding::PassthroughProvider::new(4));
    let mcp = crate::mcp::bridge_adapters::McpSearchAdapter::new(
        engine.clone(),
        provider.clone(),
        f.storage.clone(),
        f.graph.clone(),
        chrono_tz::UTC,
    );
    let http = crate::api::adapters::SearchPipelineAdapter::new(
        engine,
        provider,
        f.vector_index.clone(),
        f.fts_index.clone(),
        f.entity_index.clone(),
    );
    let before = bytes(&f);
    let args = serde_json::json!({"contractVersion":2,"id":id.to_string(),"namespace":"DEFAULT","recordAccess":false});
    let reply = crate::mcp::tools::prepared_get(&mcp, &args).await;
    assert_eq!(reply.is_error, Some(false), "{reply:?}");
    let value = serde_json::Value::Object(reply.structured_content.unwrap());
    let (status, axum::Json(http_reply)) = crate::api::prepared_get(
        &http,
        id,
        &serde_json::json!({"namespace":"DEFAULT","recordAccess":false}),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(serde_json::to_value(http_reply).unwrap(), value);
    assert_eq!(value["scope"]["namespace"], "default");
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_true_one_event_one_clock_and_full_receipt() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct CountClock(Arc<AtomicUsize>);
    impl crate::time::Clock for CountClock {
        fn now_millis(&self) -> i64 {
            self.0.fetch_add(1, Ordering::SeqCst);
            123
        }
    }
    for origin in [1, 2] {
        let f = Fixture::new();
        let owner = owner(&f);
        let id = seed(&owner).await;
        let mut engine = payload_engine(&f, &owner);
        let calls = Arc::new(AtomicUsize::new(0));
        engine.clock = Arc::new(CountClock(calls.clone()));
        engine.access_recorder = Arc::new(
            crate::search::adapters::StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
                .with_completion(owner.clone()),
        );
        let before = f.record(id).unwrap().access_history.len();
        let mut request = point_request(id, true);
        request.original_contract_version = origin;
        let reply = engine
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await;
        assert_eq!(reply.http_status, 200, "{reply:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(reply.observation_receipts.len(), 1);
        assert_eq!(
            reply.envelope.receipt.as_ref(),
            reply.observation_receipts.first()
        );
        let receipt = reply.envelope.receipt.unwrap();
        assert_eq!(
            receipt.committed_generation,
            reply
                .envelope
                .read_generation
                .unwrap()
                .generation
                .next()
                .unwrap()
        );
        assert_eq!(f.record(id).unwrap().access_history.len(), before + 1);
        assert_eq!(
            f.record(id)
                .unwrap()
                .access_history
                .last()
                .unwrap()
                .timestamp,
            123
        );
        assert!(reply.envelope.readiness.unwrap().ready);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_budget_rejects_before_exposure_and_missing_scope_is_not_defaulted() {
    let f = Fixture::new();
    let owner = owner(&f);
    let id = seed(&owner).await;
    // RejectExposure panics on accidental recording; preparation retains true.
    let engine = payload_engine(&f, &owner);
    let before = bytes(&f);
    let reply = engine.get_contract(point_request(id, true), 128).await;
    assert_eq!(reply.envelope.error.unwrap().code, "RESPONSE_TOO_LARGE");
    assert!(reply.envelope.data.is_none());
    assert!(reply.observation_receipts.is_empty());
    let mut wrong = point_request(id, true);
    wrong.namespace = Some("elsewhere".into());
    let reply = engine
        .get_contract(wrong, crate::daemon::protocol::MAX_MESSAGE_SIZE)
        .await;
    assert_eq!(reply.envelope.error.unwrap().code, "NAMESPACE_MISMATCH");
    let reply = engine
        .get_contract(
            point_request(fixed_id(9999), true),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.envelope.error.unwrap().code, "MEMORY_NOT_FOUND");
    assert!(reply.envelope.scope.is_none());
    assert!(reply.observation_receipts.is_empty());
    assert_eq!(bytes(&f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_native_only_projection_graph_is_required() {
    let f = similar_fixture().await;
    // 'other' is ExplicitFeedbackV2; default remains native LegacyAccessV1.
    {
        let storage = f.f.storage.read().unwrap();
        for view in [
            crate::storage::publication::DerivedView::Vectors,
            crate::storage::publication::DerivedView::Fts,
            crate::storage::publication::DerivedView::Entities,
        ] {
            storage.meta_store().invalidate_derived_view(view);
        }
    }
    let before = bytes(&f.f);
    let calls = f.clock.0.load(Ordering::SeqCst);
    let reply = f
        .engine
        .get_contract(
            point_request(f.foreign, false),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(f.clock.0.load(Ordering::SeqCst), calls + 1);
    assert_eq!(reply.envelope.data.unwrap().memory.namespace, "other");
    assert!(reply.envelope.readiness.is_none());
    f.f.storage
        .read()
        .unwrap()
        .meta_store()
        .invalidate_derived_view(crate::storage::publication::DerivedView::Graph);
    let calls = f.clock.0.load(Ordering::SeqCst);
    let reply = f
        .engine
        .get_contract(
            point_request(f.foreign, false),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.http_status, 503, "{reply:?}");
    assert_eq!(f.clock.0.load(Ordering::SeqCst), calls);
    assert!(reply.envelope.data.is_none());
    let reply = f
        .engine
        .get_contract(
            point_request(f.source, false),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(
        reply.http_status, 200,
        "native materialized point should not use graph: {reply:?}"
    );
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_native_origins_one_exposure_and_terminal_no_policy_projection() {
    let f = similar_fixture().await;
    for origin in [1, 2] {
        let before = f.f.record(f.foreign).unwrap();
        let calls = f.clock.0.load(Ordering::SeqCst);
        let mut request = point_request(f.foreign, true);
        request.original_contract_version = origin;
        let reply = f
            .engine
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await;
        assert_eq!(reply.http_status, 200, "{reply:?}");
        assert_eq!(f.clock.0.load(Ordering::SeqCst), calls + 1);
        let receipt = reply.envelope.receipt.as_ref().unwrap();
        let crate::storage::control::MutationOutcome::LifecycleExposureRecorded { transition } =
            &receipt.outcome
        else {
            panic!("wrong point outcome: {receipt:?}")
        };
        assert_eq!(
            transition.capture_generation,
            reply.envelope.read_generation.unwrap()
        );
        assert_eq!(transition.records.len(), 1);
        assert_eq!(transition.records[0].memory_id, f.foreign);
        assert_eq!(transition.records[0].kind, AccessKind::DirectRetrieval);
        assert_eq!(
            f.f.record(f.foreign).unwrap().access_history.len(),
            before.access_history.len() + 1
        );
        assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
        assert!(reply.envelope.readiness.unwrap().ready);
    }
    f.owner
        .forget_native_with_work(
            MutationContext::unkeyed(f.other),
            f.foreign,
            Some("other".into()),
            f.engine.native_policy_inputs(),
            Arc::new(crate::time::ManualClock::new(20_000)),
            None,
        )
        .await
        .unwrap();
    f.owner.complete().await.unwrap();
    f.f.storage
        .read()
        .unwrap()
        .meta_store()
        .invalidate_derived_view(crate::storage::publication::DerivedView::Graph);
    let before = bytes(&f.f);
    let reply = f
        .engine
        .get_contract(
            point_request(f.foreign, true),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.http_status, 200, "{reply:?}");
    let memory = reply.envelope.data.unwrap().memory;
    assert_eq!(memory.phase, DecayPhase::Tombstone);
    assert!(memory.summary.is_none() && memory.full_text.is_none());
    assert!(memory.tags.is_empty());
    assert!(
        memory
            .labels
            .as_ref()
            .is_none_or(|labels| labels.is_empty())
    );
    assert!(reply.observation_receipts.is_empty());
    assert_eq!(bytes(&f.f), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_unrelated_generation_after_capture_does_not_recapture() {
    let f = similar_fixture().await;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut request = point_request(f.foreign, true);
    request.before_observation = Some((entered.clone(), release.clone()));
    let engine = Arc::new(f.engine);
    let reader = engine.clone();
    let task = tokio::spawn(async move {
        reader
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await
    });
    entered.notified().await;
    let before =
        f.f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
    // Real other-record mutation after authoritative capture, not a raw G bump.
    f.owner
        .forget_native_with_work(
            MutationContext::unkeyed(NamespaceId::new(1)),
            f.live,
            None,
            engine.native_policy_inputs(),
            Arc::new(crate::time::ManualClock::new(20_000)),
            None,
        )
        .await
        .unwrap();
    release.notify_one();
    let reply = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply.http_status, 200, "{reply:?}");
    assert_eq!(reply.envelope.read_generation, Some(before));
    let receipt = reply.envelope.receipt.unwrap();
    assert!(receipt.committed_generation > before.generation.next().unwrap());
    let crate::storage::control::MutationOutcome::LifecycleExposureRecorded { transition } =
        receipt.outcome
    else {
        panic!("expected native exposure")
    };
    assert_eq!(transition.capture_generation, before);
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_terminal_transition_after_capture_rejects_without_payload() {
    let f = similar_fixture().await;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut request = point_request(f.foreign, true);
    request.before_observation = Some((entered.clone(), release.clone()));
    let engine = Arc::new(f.engine);
    let reader = engine.clone();
    let task = tokio::spawn(async move {
        reader
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await
    });
    entered.notified().await;
    f.owner
        .forget_native_with_work(
            MutationContext::unkeyed(f.other),
            f.foreign,
            None,
            engine.native_policy_inputs(),
            Arc::new(crate::time::ManualClock::new(20_000)),
            None,
        )
        .await
        .unwrap();
    let after_forget = f.f.storage.read().unwrap().control_state().unwrap();
    release.notify_one();
    let reply = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert!(reply.http_status >= 400, "{reply:?}");
    assert!(reply.envelope.data.is_none() && reply.observation_receipts.is_empty());
    assert_eq!(
        reply.envelope.error.unwrap().commit_state,
        crate::serialization::contract::CommitState::NotCommitted
    );
    assert_eq!(
        f.f.storage.read().unwrap().control_state().unwrap(),
        after_forget
    );
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_policy_change_keeps_real_history_and_typed_conflict() {
    let f = similar_fixture().await;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut request = point_request(f.foreign, true);
    request.before_observation = Some((entered.clone(), release.clone()));
    let engine = Arc::new(f.engine);
    let reader = engine.clone();
    let task = tokio::spawn(async move {
        reader
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await
    });
    entered.notified().await;
    let (policy_receipt, after_policy) = {
        let mut storage = f.f.storage.write().unwrap();
        let before = storage.get_record(f.foreign).unwrap().unwrap();
        let namespace = storage.get_namespace(f.other).unwrap().unwrap();
        let mut replacement = crate::storage::catalog::NamespacePolicy::from(&namespace);
        replacement.decay_rate_multiplier = Some(1.5);
        let inputs = engine.native_policy_inputs();
        let intent = storage
            .meta_store()
            .capture_native_policy(
                crate::storage::native_lifecycle::LifecycleScope {
                    namespace_id: f.other,
                    namespace_name: namespace.name,
                    source: None,
                },
                &replacement,
                &inputs,
                None,
            )
            .unwrap();
        let receipt = storage
            .apply_native_policy(&MutationContext::unkeyed(f.other), &intent, &inputs, None)
            .unwrap()
            .unwrap();
        let after = storage.get_record(f.foreign).unwrap().unwrap();
        let old_fields = &before.schema.as_ref().unwrap().lifecycle;
        let new_fields = &after.schema.as_ref().unwrap().lifecycle;
        // This actual writer changes both tokens. Do not manufacture a
        // policy-only history by restoring a target token behind its receipt.
        assert!(new_fields.policy_revision > old_fields.policy_revision);
        assert!(new_fields.record_generation > old_fields.record_generation);
        assert_eq!(after.access_history, before.access_history);
        (receipt, storage.control_state().unwrap())
    };
    release.notify_one();
    let reply = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert!(reply.envelope.data.is_none());
    assert!(reply.observation_receipts.is_empty());
    assert_eq!(
        f.f.storage.read().unwrap().control_state().unwrap(),
        after_policy
    );
    assert_eq!(
        f.f.storage
            .read()
            .unwrap()
            .lookup_operation(
                policy_receipt.store_id,
                crate::storage::control::MutationScope::Namespace(f.other),
                policy_receipt.operation_id,
            )
            .unwrap(),
        Some(policy_receipt)
    );
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(reply.http_status, 409, "{reply:?}");
    let error = reply.envelope.error.unwrap();
    assert_eq!(error.code, "LIFECYCLE_CONFLICT");
    assert_eq!(
        error.commit_state,
        crate::serialization::contract::CommitState::NotCommitted
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_publication_failure_keeps_exact_receipt_and_allows_rebuild() {
    let f = similar_fixture().await;
    let mut request = point_request(f.foreign, true);
    request.before_barrier = Some(|owner| {
        owner
            .storage
            .read()
            .unwrap()
            .meta_store()
            .invalidate_derived_view(crate::storage::publication::DerivedView::Fts)
    });
    let reply = f
        .engine
        .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
        .await;
    assert_eq!(reply.http_status, 503, "{reply:?}");
    assert!(reply.envelope.data.is_none());
    assert_eq!(
        reply.envelope.error.unwrap().commit_state,
        crate::serialization::contract::CommitState::Committed
    );
    let receipt = reply.envelope.receipt.unwrap();
    assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
    assert!(reply.envelope.read_generation.is_some());
    f.owner.complete().await.unwrap();
    assert_eq!(
        f.f.storage
            .read()
            .unwrap()
            .lookup_operation(
                receipt.store_id,
                crate::storage::control::MutationScope::Namespace(f.other),
                receipt.operation_id,
            )
            .unwrap(),
        Some(receipt.clone())
    );
    // Error-envelope controls reuse the actual committed receipt above. These
    // are mapper tests, not additional induced storage failures.
    use crate::decay::observation::{ObservationCause, ObservationError};
    use crate::serialization::get::GetReply;
    use crate::storage::StorageError;
    let conflict =
        ObservationError::storage(StorageError::LifecycleConflict("changed".into()), vec![]);
    assert_eq!(conflict.cause, Some(ObservationCause::LifecycleConflict));
    let mut without_cause = conflict.clone();
    without_cause.cause = None;
    assert_eq!(
        serde_json::to_vec(&conflict).unwrap(),
        serde_json::to_vec(&without_cause).unwrap()
    );
    let old_query = crate::serialization::query_response::QueryReply::failure(
        SearchError::Observation(conflict.clone()),
    );
    assert_eq!(
        old_query.http_status, 503,
        "global query mapping was not changed"
    );
    let pure = GetReply::failure(SearchError::Observation(conflict));
    assert_eq!(pure.http_status, 409);
    assert_eq!(pure.envelope.error.unwrap().code, "LIFECYCLE_CONFLICT");
    for (cause, expected_state) in [
        (
            StorageError::LifecycleConflict("after prior commit".into()),
            crate::serialization::contract::CommitState::Committed,
        ),
        (
            StorageError::WorkerOutcomeUnknown("worker lost".into()),
            crate::serialization::contract::CommitState::Unknown,
        ),
    ] {
        let error = ObservationError::storage(
            StorageError::CommittedMutation {
                receipt: Box::new(receipt.clone()),
                source: Box::new(cause),
            },
            Vec::new(),
        );
        let mapped = GetReply::failure(SearchError::Observation(error));
        assert_eq!(mapped.http_status, 503);
        assert_eq!(mapped.envelope.error.unwrap().commit_state, expected_state);
        assert_eq!(mapped.envelope.receipt, Some(receipt.clone()));
        assert_eq!(mapped.observation_receipts, vec![receipt.clone()]);
        assert!(mapped.envelope.data.is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_unknown_commit_retains_identity_and_reopens_committed_event() {
    let f = similar_fixture().await;
    let initial = f.f.record(f.foreign).unwrap().access_history.len();
    let store_id =
        f.f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .store_id;
    f.f.storage
        .read()
        .unwrap()
        .meta_store()
        .observation_fault
        .store(3, Ordering::SeqCst);
    let reply = f
        .engine
        .get_contract(
            point_request(f.foreign, true),
            crate::daemon::protocol::MAX_MESSAGE_SIZE,
        )
        .await;
    assert_eq!(reply.http_status, 503, "{reply:?}");
    assert!(reply.envelope.data.is_none());
    let error = reply.envelope.error.unwrap();
    assert_eq!(
        error.commit_state,
        crate::serialization::contract::CommitState::Unknown
    );
    assert_eq!(error.code, "COMMIT_OUTCOME_UNKNOWN");
    let operation_id = error
        .operation_id
        .expect("actual unknown operation identity");
    assert!(reply.observation_receipts.is_empty());
    let id = f.foreign;
    let namespace = f.other;
    drop(f.engine);
    drop(f.owner);
    let root = f.f.close();
    let storage = crate::storage::RedbStorageEngine::open_native(root.path()).unwrap();
    let receipt = storage
        .lookup_operation(
            store_id,
            crate::storage::control::MutationScope::Namespace(namespace),
            operation_id,
        )
        .unwrap()
        .unwrap();
    assert_eq!(receipt.operation_id, operation_id);
    let crate::storage::control::MutationOutcome::LifecycleExposureRecorded { transition } =
        receipt.outcome
    else {
        panic!("unknown outcome must resolve to original exposure")
    };
    assert_eq!(transition.records.len(), 1);
    assert_eq!(transition.records[0].memory_id, id);
    assert_eq!(
        storage
            .get_record(id)
            .unwrap()
            .unwrap()
            .access_history
            .len(),
        initial + 1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_cancel_keeps_copy_leases_until_worker_exit() {
    use crate::serialization::get::PointCopyPause;
    // Always release on an assertion failure as well as the successful path.
    struct Release(Arc<PointCopyPause>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.release();
        }
    }
    let f = similar_fixture().await;
    let engine = Arc::new(f.engine);
    let before = bytes(&f.f);
    let before_record = f.f.record(f.foreign).unwrap();
    let at =
        f.f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
    let tracker = f.f.storage.read().unwrap().meta_store().work_tracker();
    let publication =
        f.f.storage
            .read()
            .unwrap()
            .meta_store()
            .derived_publication
            .clone();
    let storage_refs = Arc::strong_count(&f.f.storage);
    let pause = Arc::new(PointCopyPause::default());
    let release = Release(pause.clone());
    let mut request = point_request(f.foreign, true);
    request.copy_pause = Some(pause.clone());
    let reader = engine.clone();
    let task = tokio::spawn(async move {
        reader
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), pause.entered.notified())
        .await
        .unwrap();
    assert!(f.f.storage.try_write().is_err());
    assert!(f.f.graph.try_write().is_err());
    assert!(publication.try_write().is_err());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    // Cancelling the await must not release leases owned by spawn_blocking.
    assert!(f.f.storage.try_write().is_err());
    assert!(f.f.graph.try_write().is_err());
    assert!(publication.try_write().is_err());
    assert_eq!(tracker.status().unresolved, 1);
    drop(release);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if Arc::strong_count(&f.f.storage) == storage_refs
                && f.f.storage.try_write().is_ok()
                && f.f.graph.try_write().is_ok()
                && publication.try_write().is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!pause.timed_out.load(Ordering::SeqCst));
    assert_eq!(tracker.status().pending, 1);
    assert_eq!(tracker.status().unresolved, 1);
    assert_eq!(bytes(&f.f), before);
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
    assert!(
        f.owner
            .readiness_barrier(at, Instant::now() + Duration::from_millis(50))
            .await
            .is_err()
    );
    // The timeout may return before the closer's worker drops its capture.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !tracker.status().admission_open {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let id = f.foreign;
    drop(engine);
    drop(f.owner);
    let root = f.f.close();
    let storage = crate::storage::RedbStorageEngine::open_native(root.path()).unwrap();
    assert_eq!(storage.control_state().unwrap().generation_ref(), at);
    assert_eq!(
        storage.get_record(id).unwrap().unwrap().to_bytes(),
        before_record.to_bytes()
    );
    assert_eq!(storage.meta_store().work_tracker().status().pending, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_get_actual_close_accepts_existing_root_descendants_and_keeps_receipt() {
    let f = similar_fixture().await;
    let at =
        f.f.storage
            .read()
            .unwrap()
            .control_state()
            .unwrap()
            .generation_ref();
    let initial_events = f.f.record(f.foreign).unwrap().access_history.len();
    let tracker = f.f.storage.read().unwrap().meta_store().work_tracker();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut request = point_request(f.foreign, true);
    request.before_observation = Some((entered.clone(), release.clone()));
    let engine = Arc::new(f.engine);
    let reader = engine.clone();
    let task = tokio::spawn(async move {
        reader
            .get_contract(request, crate::daemon::protocol::MAX_MESSAGE_SIZE)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    let owner = f.owner.clone();
    let closing = tokio::spawn(async move { owner.readiness_barrier(at, deadline()).await });
    let watching = tracker.clone();
    tokio::task::spawn_blocking(move || watching.wait_until_closed())
        .await
        .unwrap();
    // Release before assertions so an assertion failure cannot strand the task.
    let rejected_root = f.owner.accept_work(None).is_err();
    let still_draining = !closing.is_finished();
    release.notify_one();
    assert!(rejected_root && still_draining);
    let reply = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(10), closing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let receipt = reply
        .envelope
        .receipt
        .as_ref()
        .expect("full known exposure receipt");
    assert_eq!(reply.observation_receipts, vec![receipt.clone()]);
    assert_eq!(reply.envelope.read_generation, Some(at));
    match reply.http_status {
        200 => assert!(reply.envelope.readiness.as_ref().unwrap().ready),
        503 => {
            // Two barriers may race after the accepted root finishes. A losing
            // controller must keep the known receipt and withhold the payload.
            assert!(reply.envelope.data.is_none());
            assert_eq!(
                reply.envelope.error.as_ref().unwrap().commit_state,
                crate::serialization::contract::CommitState::Committed
            );
        }
        _ => panic!("unexpected point outcome: {reply:?}"),
    }
    assert!(ready.ready && ready.committed_generation >= receipt.committed_generation);
    assert_eq!(
        f.f.storage
            .read()
            .unwrap()
            .lookup_operation(
                receipt.store_id,
                crate::storage::control::MutationScope::Namespace(f.other),
                receipt.operation_id
            )
            .unwrap(),
        Some(receipt.clone())
    );
    assert_eq!(
        f.f.record(f.foreign).unwrap().access_history.len(),
        initial_events + 1
    );
    assert_eq!(f.clock.0.load(Ordering::SeqCst), 1);
    assert_eq!(tracker.status().pending, 0);
    assert_eq!(tracker.status().unresolved, 0);
    assert!(tracker.status().admission_open);
}

#[test]
fn prepared_get_strict_parser_preserves_omitted_scope_and_access_intent() {
    let id = fixed_id(9821);
    let base = serde_json::json!({"contractVersion":2,"id":id.to_string()});
    let request = crate::serialization::get::GetRequest::from_mcp(&base).unwrap();
    assert!(request.record_access);
    assert!(request.namespace.is_none());
    let mut null_scope = base.clone();
    null_scope["namespace"] = serde_json::Value::Null;
    assert!(
        crate::serialization::get::GetRequest::from_mcp(&null_scope)
            .unwrap()
            .namespace
            .is_none()
    );
    for (field, invalid) in [
        ("recordAccess", serde_json::Value::Null),
        ("recordAccess", serde_json::json!("false")),
        ("asOf", serde_json::json!(123)),
        ("namespace", serde_json::json!(true)),
        ("id", serde_json::json!(uuid::Uuid::nil().to_string())),
    ] {
        let mut input = base.clone();
        input[field] = invalid;
        assert!(
            crate::serialization::get::GetRequest::from_mcp(&input).is_err(),
            "{input}"
        );
    }
}
