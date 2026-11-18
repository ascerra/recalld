//! Actual owned adapters and deterministic clock latches, synthetic tempstores only.
use super::*;
use crate::time::Clock;

async fn call(
    h: &Harness,
    mcp: bool,
    similar: bool,
    access: bool,
    scope: Option<&str>,
) -> (bool, String) {
    call_with_clock(h, mcp, similar, access, scope, None).await
}

async fn call_with_clock(
    h: &Harness,
    mcp: bool,
    similar: bool,
    access: bool,
    scope: Option<&str>,
    clock: Option<Arc<dyn Clock>>,
) -> (bool, String) {
    let id = if similar {
        h.ids[2]
    } else {
        *h.ids.last().unwrap()
    };
    if mcp {
        let mut bridge = crate::test_support::StubBridge::default().into_bridge();
        bridge.search = Arc::new(crate::mcp::bridge_adapters::McpSearchAdapter::new(
            h.engine.clone(),
            h.embedding.clone(),
            h.f.storage.clone(),
            h.f.graph.clone(),
            chrono_tz::UTC,
        ));
        let ordered_close = clock.is_some() && !similar;
        let mut storage = crate::mcp::bridge_adapters::McpStorageAdapter::new(
            h.f.storage.clone(),
            h.f.cache.clone(),
            h.embedding.clone(),
            h.f.vector_index.clone(),
            h.f.fts_index.clone(),
            h.f.entity_index.clone(),
            h.f.graph.clone(),
            Arc::new(crate::config::RecalldConfig::default()),
            chrono_tz::UTC,
        )
        .with_clock(clock.unwrap_or_else(|| Arc::new(crate::time::SystemClock)));
        if ordered_close {
            // This fixture expects successful get readiness after its external
            // barrier drains the accepted root. Order the two controllers;
            // concurrent close remains refused by the tracker regression.
            storage.api1_before_barrier = Some(|owner| {
                let tracker = owner.storage.read().unwrap().meta_store().work_tracker();
                tracker.wait_until_open();
            });
        }
        bridge.storage = Arc::new(storage);
        let mut input = json!({"id":id,"recordAccess":access,"limit":1});
        if let Some(scope) = scope {
            input["namespace"] = json!(scope);
        }
        let result = crate::mcp::tools::dispatch_tool(
            &bridge,
            if similar {
                "find_similar_memories"
            } else {
                "get_memory"
            },
            input,
        )
        .await;
        let crate::mcp::protocol::ContentBlock::Text { text } = &result.content[0];
        (result.is_error != Some(true), text.clone())
    } else if similar {
        let mut input = json!({"recordAccess":access,"limit":1,"includeEmbeddings":true});
        if let Some(scope) = scope {
            input["namespace"] = json!(scope);
        }
        let (status, body) = request(h, &format!("/similar/{id}"), input.to_string()).await;
        (status.is_success(), body.to_string())
    } else {
        use tower::ServiceExt;
        let mut uri = format!("/memories/{id}?recordAccess={access}");
        if let Some(scope) = scope {
            uri.push_str(&format!("&namespace={scope}"));
        }
        let response = h
            .http()
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let ok = response.status().is_success();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (ok, String::from_utf8(bytes.to_vec()).unwrap())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coherent_by_id_actual_payload_scope_and_returned_only_exposure() {
    for mcp in [false, true] {
        for similar in [false, true] {
            let h = coherent_harness().await;
            let before = h.snapshot();
            let (ok, body) = call(&h, mcp, similar, false, Some("default")).await;
            assert!(ok, "{mcp}/{similar}: {body}");
            assert!(body.contains("owned résumé"), "{body}");
            assert_eq!(h.embedding.calls.load(Ordering::SeqCst), 0);
            h.assert_quiet(&before).await;
            let (ok, body) = call(&h, mcp, similar, true, Some("missing")).await;
            assert!(!ok, "{body}");
            assert!(!body.contains("owned résumé"));
            h.assert_quiet(&before).await;
            let (ok, body) = call(&h, mcp, similar, true, Some("default")).await;
            assert!(ok, "{mcp}/{similar}: {body}");
            for id in &h.ids {
                let row = h.f.record(*id).unwrap();
                assert_eq!(
                    row.access_history.len(),
                    usize::from(id == h.ids.last().unwrap())
                );
                if let Some(event) = row.access_history.first() {
                    assert_eq!(
                        event.kind,
                        if similar {
                            AccessKind::AssociativeRetrieval
                        } else {
                            AccessKind::DirectRetrieval
                        }
                    );
                }
            }
            let tracker = h.f.storage.read().unwrap().meta_store().work_tracker();
            assert_eq!(tracker.status().pending, 0);
            assert_eq!(tracker.status().unresolved, 0);
        }
    }
}

struct LatchClock {
    calls: AtomicUsize,
    entered: tokio::sync::Notify,
    resume: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}
impl Clock for LatchClock {
    fn now_millis(&self) -> i64 {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.resume
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(15))
                .unwrap();
        }
        2_000_000_000_000
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coherent_by_id_actual_capture_rejects_forget_and_compaction_before_exposure() {
    // MCP get copies under one engine guard; its serialization test is separate.
    for (mcp, similar) in [(false, false), (false, true), (true, true)] {
        for compact in [false, true] {
            for access in [false, true] {
                let mut h = coherent_harness().await;
                let (resume_tx, resume_rx) = std::sync::mpsc::channel();
                let clock = Arc::new(LatchClock {
                    calls: AtomicUsize::new(0),
                    entered: Default::default(),
                    resume: std::sync::Mutex::new(resume_rx),
                });
                h.engine = Arc::new(
                    Arc::try_unwrap(h.engine)
                        .ok()
                        .unwrap()
                        .with_lifecycle_inputs(clock.clone(), Default::default(), 0.0),
                );
                let h = Arc::new(h);
                let child = h.clone();
                let at =
                    h.f.storage
                        .read()
                        .unwrap()
                        .meta_store()
                        .publication_state()
                        .unwrap()
                        .canonical;
                let query =
                    tokio::spawn(async move { call(&child, mcp, similar, access, None).await });
                tokio::time::timeout(Duration::from_secs(10), clock.entered.notified())
                    .await
                    .unwrap();
                let owner = coherent_owner(&h);
                let id = *h.ids.last().unwrap();
                let writer = tokio::spawn(async move {
                    if compact {
                        tokio::task::spawn_blocking(move || {
                            owner.storage.write().unwrap().compact_text_log()
                        })
                        .await
                        .unwrap()
                        .unwrap();
                    } else {
                        assert!(owner.forget(id, Some("default")).await.unwrap());
                    }
                });
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if h.f
                            .storage
                            .read()
                            .unwrap()
                            .meta_store()
                            .publication_state()
                            .unwrap()
                            .canonical
                            != at
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                resume_tx.send(()).unwrap();
                let (ok, body) = tokio::time::timeout(Duration::from_secs(10), query)
                    .await
                    .unwrap()
                    .unwrap();
                assert!(!ok, "{mcp}/{similar}/{compact}/{access}: {body}");
                assert!(!body.contains("owned résumé"));
                assert_eq!(h.access.completed.load(Ordering::SeqCst), 0);
                assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
                tokio::time::timeout(Duration::from_secs(10), writer)
                    .await
                    .unwrap()
                    .unwrap();
                let state =
                    h.f.storage
                        .read()
                        .unwrap()
                        .meta_store()
                        .work_tracker()
                        .status();
                assert_eq!(state.pending, 0);
                assert_eq!(state.unresolved, 0);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coherent_by_id_actual_postcommit_failure_preserves_receipt_and_hides_payload() {
    for mcp in [false, true] {
        for similar in [false, true] {
            for fault in [1, 2] {
                let h = coherent_harness().await;
                h.f.storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .observation_fault
                    .store(fault, Ordering::SeqCst);
                let before = h.snapshot();
                assert!(call(&h, mcp, similar, false, None).await.0);
                h.assert_quiet(&before).await;
                let (ok, body) = call(&h, mcp, similar, true, None).await;
                assert!(!ok, "{mcp}/{similar}/{fault}: {body}");
                assert!(!body.contains("owned résumé"));
                let point = mcp && !similar;
                let error: Value = if point {
                    serde_json::from_str(&body).unwrap()
                } else if mcp {
                    let start = body.find("{\"commitState\"").expect(&body);
                    serde_json::Deserializer::from_str(&body[start..])
                        .into_iter()
                        .next()
                        .unwrap()
                        .unwrap()
                } else {
                    serde_json::from_str::<Value>(&body).unwrap()["observation"].clone()
                };
                assert_eq!(error["commitState"], "committed", "{body}");
                let receipts: Vec<crate::storage::control::WriteReceipt> = serde_json::from_value(
                    error[if point {
                        "observationReceipts"
                    } else {
                        "receipts"
                    }]
                    .clone(),
                )
                .unwrap();
                assert_eq!(receipts.len(), 1);
                if point {
                    assert_eq!(error["error"], "INDEX_NOT_READY");
                    assert_eq!(
                        error["receipt"],
                        serde_json::to_value(&receipts[0]).unwrap()
                    );
                }
                let crate::storage::control::MutationOutcome::ExposuresRecorded { accesses } =
                    &receipts[0].outcome
                else {
                    panic!("{body}")
                };
                assert_eq!(accesses.len(), 1);
                // Fault1 deliberately poisons the serving handle. Verify the
                // complete receipt after real reopen, never bypass that gate.
                let Harness {
                    f,
                    engine,
                    access,
                    embedding,
                    ..
                } = h;
                drop((engine, access, embedding));
                let root = f.close();
                let reopened = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
                assert_eq!(
                    reopened
                        .lookup_operation(
                            receipts[0].store_id,
                            crate::storage::control::MutationScope::Namespace(
                                crate::model::NamespaceId::new(1)
                            ),
                            receipts[0].operation_id,
                        )
                        .unwrap(),
                    Some(receipts[0].clone())
                );
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coherent_by_id_close_waits_for_payload_and_exposure_descendants_with_one_clock() {
    for mcp in [false, true] {
        for similar in [false, true] {
            let mut h = coherent_harness().await;
            let (resume_tx, resume_rx) = std::sync::mpsc::channel();
            let clock = Arc::new(LatchClock {
                calls: AtomicUsize::new(0),
                entered: Default::default(),
                resume: std::sync::Mutex::new(resume_rx),
            });
            h.engine = Arc::new(
                Arc::try_unwrap(h.engine)
                    .ok()
                    .unwrap()
                    .with_lifecycle_inputs(clock.clone(), Default::default(), 0.0),
            );
            let owner = coherent_owner(&h);
            let at =
                h.f.storage
                    .read()
                    .unwrap()
                    .meta_store()
                    .publication_state()
                    .unwrap()
                    .canonical;
            let tracker = h.f.storage.read().unwrap().meta_store().work_tracker();
            let h = Arc::new(h);
            let child = h.clone();
            let c = clock.clone();
            let query = tokio::spawn(async move {
                call_with_clock(&child, mcp, similar, true, None, Some(c)).await
            });
            tokio::time::timeout(Duration::from_secs(10), clock.entered.notified())
                .await
                .unwrap();
            let barrier = tokio::spawn(async move {
                owner
                    .readiness_barrier(at, std::time::Instant::now() + Duration::from_secs(15))
                    .await
            });
            let waiting = tracker.clone();
            tokio::task::spawn_blocking(move || waiting.wait_until_closed())
                .await
                .unwrap();
            assert!(!barrier.is_finished());
            assert!(tracker.status().pending > 0);
            resume_tx.send(()).unwrap();
            let (ok, body) = tokio::time::timeout(Duration::from_secs(10), query)
                .await
                .unwrap()
                .unwrap();
            assert!(ok, "{mcp}/{similar}: {body}");
            let ready = tokio::time::timeout(Duration::from_secs(10), barrier)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(ready.ready);
            assert_eq!(ready.pending_work, 0);
            assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
            let row = h.f.record(*h.ids.last().unwrap()).unwrap();
            assert_eq!(row.access_history.len(), 1);
            assert_eq!(row.access_history[0].timestamp, 2_000_000_000_000);
            assert_eq!(tracker.status().unresolved, 0);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coherent_by_id_marker_missing_empty_and_owned_payload_survive_handle_change() {
    let h = coherent_harness().await;
    let id = *h.ids.last().unwrap();
    let missing = MemoryId::new();
    let before = h.snapshot();
    let prepared = h
        .engine
        .prepare_coherent_get(missing, None, false, None)
        .await
        .unwrap()
        .unwrap();
    assert!(prepared.payload.is_none() && prepared.response.is_none());
    let empty = h
        .engine
        .prepare_coherent_similar(id, None, 0, None, false, true, None)
        .await
        .unwrap()
        .unwrap();
    assert!(empty.results.is_empty() && empty.payload.rows.is_empty());
    let owned = h
        .engine
        .prepare_coherent_get(id, Some("default"), false, None)
        .await
        .unwrap()
        .unwrap();
    h.assert_quiet(&before).await;
    h.f.storage.write().unwrap().compact_text_log().unwrap();
    coherent_owner(&h).complete().await.unwrap();
    assert_eq!(
        owned.payload.unwrap().1.full_text.as_deref(),
        Some("owned résumé 日本語")
    );
    assert!(
        coherent_owner(&h)
            .forget(id, Some("default"))
            .await
            .unwrap()
    );
    let before = h.snapshot();
    for mcp in [false, true] {
        for access in [false, true] {
            let (ok, body) = call(&h, mcp, false, access, Some("default")).await;
            assert!(ok, "{body}");
            assert!(body.to_lowercase().contains("tombstone"));
            assert!(!body.contains("owned résumé") && !body.contains("coherent payload"));
            h.assert_quiet(&before).await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn coherent_by_id_point_capture_requires_canonical_admission_not_unused_index_stamps() {
    let h = Harness::new().await;
    let before = h.snapshot();
    // Raw legacy fixture insertion has canonical rows but no published derived
    // registry. A canonical point fetch does not consume any of those indexes.
    assert!(
        h.f.storage
            .read()
            .unwrap()
            .meta_store()
            .capture_read_view()
            .is_err()
    );
    let (ok, body) = call(&h, true, false, false, Some("default")).await;
    assert!(ok, "{body}");
    assert!(body.contains("other"));
    h.assert_quiet(&before).await;
    let (ok, body) = call(&h, true, false, true, Some("missing")).await;
    assert!(!ok && body.contains("NAMESPACE_MISMATCH"), "{body}");
    h.assert_quiet(&before).await;
    h.f.storage.read().unwrap().mark_unavailable();
    let (ok, body) = call(&h, true, false, false, None).await;
    assert!(!ok, "{body}");
    assert_eq!(h.access.completed.load(Ordering::SeqCst), 0);
}
