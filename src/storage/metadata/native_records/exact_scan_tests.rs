//! Actual migrated/native writer fixtures. Public contract2 stays closed; the
//! inner MCP/REST handlers exercise the real backend without activating routes.
use super::*;
use crate::search::exact_scan::{ExactScanError, scan_exact, scan_exact_after_capture};
use crate::serialization::scan::ExactScanRequest;
use crate::test_support::Fixture;
use serde_json::{Value, json};

async fn measured(f: &Fixture, body: &Value) -> crate::search::exact_scan::ExactScanPage {
    let request = ExactScanRequest::from_http(body, "native").unwrap();
    let page = scan_exact(f.storage.clone(), request).await.unwrap();
    eprintln!(
        "EXACT_NATIVE_CAPTURE {:?}; compared={} through={}",
        page.diagnostics, page.data.coverage.compared_this_page, page.data.coverage.compared_so_far
    );
    page
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_native_writer_205_pairs_transport_reopen_ghost_tombstone_and_stale_cursor() {
    let mut root = tempfile::tempdir().unwrap();
    // Certify a genuinely retained Full document, then lose content through
    // actual native maintenance. The old-Ghost remedy negative stays separate.
    let d = certified_fixture(root.path());
    {
        let mut session = NativeLifecycleSession::open(
            &d.path,
            NativePolicyInputs::default(),
            Arc::new(crate::time::ManualClock::new(10_000 * 86_400_000)),
        )
        .unwrap();
        let before = session.engine.get_record(d.parent).unwrap().unwrap();
        assert_eq!(before.phase, DecayPhase::Full);
        let vector = before.schema.as_ref().unwrap().vector.clone();
        assert!(vector.is_some());
        for (key, phase) in [
            ("scan-parent-summary", DecayPhase::Summary),
            ("scan-parent-ghost", DecayPhase::Ghost),
        ] {
            let result = session.maintain(&ctx(&session, &d, key), d.parent).unwrap();
            let crate::storage::native_maintenance::NativeDecayResult::Committed(receipt) = result
            else {
                panic!("parent phase loss did not commit");
            };
            assert!(matches!(
                receipt.outcome,
                MutationOutcome::NativePhaseMaterialized { .. }
            ));
            let row = session.engine.get_record(d.parent).unwrap().unwrap();
            assert_eq!(row.phase, phase);
            assert_eq!(row.schema.as_ref().unwrap().vector, vector);
        }
    }
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let mut eligible = vec![d.parent];
    // Native supplied-vector insertion, not raw schema rows. Avoid graph caps:
    // these records deliberately have no relationship edges.
    for i in 0..205 {
        let mut original = input(&d);
        original.memory.parent_id = None;
        let intent = session.capture_insert(scope(&d), &original).unwrap();
        let receipt = session
            .insert_prepared(
                &ctx(&session, &d, &format!("pair-{i}")),
                &intent,
                &generated(),
            )
            .unwrap();
        eligible.push(inserted(&receipt));
    }
    let tombstone = eligible.pop().unwrap();
    let intent = session.capture_tombstone(tombstone, Some(d.ns)).unwrap();
    session
        .tombstone(&ctx(&session, &d, "pair-forget"), &intent)
        .unwrap();
    eligible.sort_unstable_by_key(|id| *id.as_bytes());
    assert_eq!(eligible.len(), 205);
    let mut expected = Vec::new();
    for (i, left) in eligible.iter().enumerate() {
        for right in eligible.iter().skip(i + 1) {
            expected.push(json!({"leftId":left,"rightId":right,"similarity":1.0}));
        }
    }
    let mut engine = Some(session.engine);
    let mut cursor: Option<String> = None;
    let mut saved_cursor = None;
    let mut actual = Vec::new();
    let mut generation = None;
    for page_index in 0..3 {
        // Close/open through the actual native gate between pages. Private
        // views start empty and are rebuilt through the real coordinator.
        let f = Fixture::from_open_engine(root, engine.take().unwrap());
        let publisher = readers::completion(&f);
        if page_index == 0 {
            publisher.complete().await.unwrap();
        } else {
            publisher.rebuild_all().await.unwrap();
        }
        let (api, bridge, query) = readers::readers(&f, clock.clone());
        let before = readers::bytes(&d.path);
        if page_index == 0 {
            let error = scan_exact(
                f.storage.clone(),
                ExactScanRequest::from_http(
                    &json!({"threshold":1.0,"pairBudget":100000}),
                    "native",
                )
                .unwrap(),
            )
            .await
            .unwrap_err();
            assert!(matches!(error, ExactScanError::ResponseTooLarge));
            let public = crate::mcp::tools::dispatch_tool(
                &bridge,
                "find_similar_memories",
                json!({"contractVersion":2,"mode":"scan","namespace":"native"}),
            )
            .await;
            assert_eq!(public.is_error, Some(true));
            use tower::ServiceExt;
            let response = api
                .clone()
                .oneshot(
                    axum::http::Request::post("/namespaces/native/duplicates")
                        .header("Recalld-Contract-Version", "2")
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        }
        let mut body = json!({"threshold":1.0,"pairBudget":10000});
        if let Some(cursor) = &cursor {
            body["cursor"] = json!(cursor);
        }
        let page = measured(&f, &body).await;
        assert_eq!(page.data.coverage.eligible_count, 205);
        assert_eq!(page.data.coverage.excluded_count, 0);
        assert_eq!(page.data.coverage.total_pairs, 20910);
        assert_eq!(page.data.coverage.phase_counts["ghost"], 1);
        assert_eq!(page.data.coverage.phase_counts["tombstone"], 1);
        assert_eq!(page.data.coverage.complete, page_index == 2);
        if let Some(generation) = generation {
            assert_eq!(page.data.coverage.read_generation, generation);
        } else {
            generation = Some(page.data.coverage.read_generation);
        }
        cursor = page.data.next_cursor.clone();
        if page_index == 0 {
            saved_cursor = cursor.clone();
        }
        let expected_envelope = serde_json::to_value(page.into_envelope()).unwrap();
        let response = if page_index == 0 {
            let mut args = body.clone();
            args["contractVersion"] = json!(2);
            args["mode"] = json!("scan");
            args["namespace"] = json!("native");
            let result =
                crate::mcp::tools::dispatch_v2(&bridge, "find_similar_memories", args).await;
            assert_ne!(result.is_error, Some(true));
            let wire = serde_json::to_value(&result).unwrap();
            let text: Value =
                serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(text, wire["structuredContent"]);
            text
        } else {
            let adapter = crate::api::adapters::StorageEngineAdapter::new(
                f.storage.clone(),
                f.cache.clone(),
                f.vector_index.clone(),
                f.fts_index.clone(),
                f.entity_index.clone(),
                f.graph.clone(),
            );
            let response = crate::api::scan_duplicates_v2(&adapter, "native", &body).await;
            assert_eq!(response.status(), axum::http::StatusCode::OK);
            let bytes = axum::body::to_bytes(
                response.into_body(),
                crate::daemon::protocol::MAX_MESSAGE_SIZE,
            )
            .await
            .unwrap();
            serde_json::from_slice(&bytes).unwrap()
        };
        assert_eq!(response, expected_envelope);
        actual.extend(
            response["data"]["pairs"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
        assert_eq!(readers::bytes(&d.path), before);
        drop((api, bridge, query, publisher));
        root = f.close();
        let reopened = open(&d, clock.clone());
        if page_index == 2 {
            let intent = reopened
                .capture_feedback(
                    *eligible.iter().find(|id| **id != d.parent).unwrap(),
                    Some(d.ns),
                    3,
                )
                .unwrap();
            reopened
                .apply_feedback(&ctx(&reopened, &d, "invalidate-cursor"), &intent)
                .unwrap();
        }
        engine = Some(reopened.engine);
    }
    assert_eq!(actual, expected);
    assert!(cursor.is_none());
    let f = Fixture::from_open_engine(root, engine.take().unwrap());
    let request = ExactScanRequest::from_http(
        &json!({"threshold":1.0,"cursor":saved_cursor.unwrap()}),
        "native",
    )
    .unwrap();
    assert!(matches!(
        scan_exact(f.storage.clone(), request).await,
        Err(ExactScanError::Stale)
    ));
}

#[tokio::test]
async fn exact_native_old_ghost_cannot_manufacture_a_certification_document() {
    struct NoDocumentProvider(crate::model::schema::EmbeddingProfile);
    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for NoDocumentProvider {
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> &str {
            &self.0.model_id
        }
        fn profile(&self) -> Option<crate::model::schema::EmbeddingProfile> {
            Some(self.0.clone())
        }
        async fn embed(&self, _: &str) -> Result<Vec<f32>, crate::embedding::EmbeddingError> {
            panic!("a genuine old Ghost has no retained document for the provider");
        }
    }
    let root = tempfile::tempdir().unwrap();
    let d = fixture_with_ghost(root.path(), true);
    let before = readers::bytes(&d.path);
    let snapshot = crate::storage::offline_snapshot::OfflineSnapshot::open(&d.path).unwrap();
    let source = snapshot.control().generation_ref();
    let ghost = snapshot.record(d.parent).unwrap().unwrap();
    assert_eq!(ghost.record.phase, DecayPhase::Ghost);
    assert!(ghost.record.summary.is_empty() && ghost.full_text.is_none());
    assert!(ghost.embedding.is_some()); // Retained vectors are not provider input.
    drop(snapshot);
    let request = crate::storage::vector_certification::ReembedVectors {
        source,
        namespace_id: d.ns,
        namespace_name: "native".into(),
        profile_id: d.profile.profile_id().unwrap(),
        context: MutationContext {
            store_id: Some(source.store_id),
            expected_generation: Some(source.generation),
            scope: MutationScope::Namespace(d.ns),
            contract_version: 1,
            idempotency_key: Some("old-ghost-remedy".into()),
        },
    };
    let destination = root.path().join("must-not-certify-old-ghost");
    let error = crate::storage::vector_certification::reembed_fresh(
        &d.path,
        &destination,
        &request,
        &NoDocumentProvider(d.profile.clone()),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, StorageError::InvalidMutation(ref reason)
        if reason.contains("no complete retained document")),
        "{error:?}"
    );
    assert_eq!(readers::bytes(&d.path), before);
    assert!(!destination.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_native_generation_change_after_capture_discards_computed_page() {
    let root = tempfile::tempdir().unwrap();
    let d = certified_fixture(root.path());
    let clock = Arc::new(FixedClock(AtomicUsize::new(0)));
    let mut session = open(&d, clock.clone());
    let intent = session.capture_insert(scope(&d), &input(&d)).unwrap();
    session
        .insert_prepared(&ctx(&session, &d, "new"), &intent, &generated())
        .unwrap();
    let mutation_ctx = ctx(&session, &d, "during-scan");
    let f = Fixture::from_open_engine(root, session.engine);
    let publisher = readers::completion(&f);
    publisher.complete().await.unwrap();
    let (captured_tx, captured_rx) = tokio::sync::oneshot::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(0);
    let storage = f.storage.clone();
    let request = ExactScanRequest::from_http(&json!({"threshold":1.0}), "native").unwrap();
    let scan = tokio::spawn(scan_exact_after_capture(storage, request, move || {
        captured_tx.send(()).unwrap();
        resume_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
    }));
    captured_rx.await.unwrap();
    {
        let engine = f.storage.read().unwrap();
        let policy = NativePolicyInputs::default();
        let intent = engine
            .meta_store()
            .capture_feedback(d.parent, Some(d.ns), 3, &policy)
            .unwrap();
        engine
            .meta_store()
            .apply_native_feedback(&mutation_ctx, &intent, &policy, clock.as_ref())
            .unwrap();
    }
    resume_tx.send(()).unwrap();
    let after_write = readers::bytes(&d.path);
    assert!(matches!(scan.await.unwrap(), Err(ExactScanError::Stale)));
    assert_eq!(readers::bytes(&d.path), after_write);
}
