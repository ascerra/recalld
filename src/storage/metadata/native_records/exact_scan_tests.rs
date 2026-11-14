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
    let d = fixture_with_ghost(root.path(), true);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_native_generation_change_after_capture_discards_computed_page() {
    let root = tempfile::tempdir().unwrap();
    let d = fixture(root.path());
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
