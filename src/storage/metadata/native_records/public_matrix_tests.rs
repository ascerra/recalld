//! Public API1 consumers. Planner comparisons prove adapter/policy/clock
//! agreement, not an independent FSRS oracle or deterministic operation IDs.
use super::super::super::phase_clock::RegressedClock;
use super::*;
use crate::decay::lifecycle::{plan_feedback, resolve_policy};
use redb::ReadableTable;
use std::sync::atomic::AtomicI64;
use tower::ServiceExt;

const DAY: i64 = 86_400_000;
#[derive(Clone, Copy, Debug)]
enum Layout {
    Legacy,
    NativeV1,
    NativeV2,
}
#[derive(Clone, Copy, Debug)]
enum Lane {
    Mcp,
    Http,
    Remote,
}
const LAYOUTS: [Layout; 3] = [Layout::Legacy, Layout::NativeV1, Layout::NativeV2];
const LANES: [Lane; 3] = [Lane::Mcp, Lane::Http, Lane::Remote];

async fn matrix_setup(
    layout: Layout,
    phase: DecayPhase,
    multiplier: f32,
    perma: f32,
) -> (
    Fixture,
    Data,
    Arc<RegressedClock>,
    McpBridge,
    crate::api::AppState,
) {
    let root = tempfile::tempdir().unwrap();
    let mut config = NamespaceConfig::default_namespace(0);
    config.decay_rate_multiplier = Some(multiplier);
    config.permastore_threshold = perma;
    let mut d = fixture_configured(root.path(), phase == DecayPhase::Ghost, config, |engine| {
        if phase == DecayPhase::Summary {
            let (id, record) = engine
                .scan_phase_records(DecayPhase::Full)
                .unwrap()
                .pop()
                .unwrap();
            let ns = engine
                .get_namespace(NamespaceId::new(record.namespace_id))
                .unwrap()
                .unwrap();
            assert!(matches!(
                engine
                    .apply_legacy_decay_with_receipt(
                        &MutationContext::unkeyed(ns.id),
                        id,
                        &record,
                        &ns,
                        DecayPhase::Summary,
                        0.6,
                        0.6,
                    )
                    .unwrap(),
                crate::storage::maintenance::DecayMutationResult::Committed(_)
            ));
        }
    });
    d.path = root.path().join(match layout {
        Layout::Legacy => "legacy",
        Layout::NativeV1 => "schema",
        Layout::NativeV2 => "v2",
    });
    let clock = Arc::new(RegressedClock {
        time: AtomicI64::new(8 * DAY),
        calls: AtomicUsize::new(0),
    });
    let engine = match layout {
        Layout::Legacy => RedbStorageEngine::open(&d.path).unwrap(),
        _ => {
            NativeLifecycleSession::open(&d.path, NativePolicyInputs::default(), clock.clone())
                .unwrap()
                .engine
        }
    };
    let f = Fixture::from_open_engine(root, engine);
    completion(&f).rebuild_all().await.unwrap();
    let (bridge, state) = public_adapters(&f, clock.clone());
    (f, d, clock, bridge, state)
}

async fn invoke(
    lane: Lane,
    bridge: &McpBridge,
    state: &crate::api::AppState,
    id: MemoryId,
    operation: Operation,
    quality: Option<Value>,
) -> (bool, Value) {
    let mut args = json!({"id":id});
    if let Some(quality) = quality {
        args["quality"] = quality;
    }
    if matches!(lane, Lane::Http) {
        let (method, uri) = if operation == Operation::Feedback {
            ("POST", format!("/memories/{id}/reinforce"))
        } else {
            ("DELETE", format!("/memories/{id}"))
        };
        args.as_object_mut().unwrap().remove("id");
        let response = crate::api::router(state.clone(), &Default::default())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(args.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let success = response.status().is_success();
        let value: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        return (
            success,
            if success {
                value["data"].clone()
            } else {
                value
            },
        );
    }
    let tool = if operation == Operation::Feedback {
        "reinforce_memory"
    } else {
        "forget_memory"
    };
    let reply = crate::mcp::tools::dispatch_tool(bridge, tool, args).await;
    let success = reply.is_error != Some(true);
    let wire = serde_json::to_value(reply).unwrap();
    let text = wire["content"][0]["text"].as_str().unwrap();
    let value = if success {
        serde_json::from_str(text).unwrap()
    } else {
        // Existing API1 validation errors may be plain text. Preserve that
        // contract instead of requiring the owned lifecycle error envelope.
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
    };
    (success, value)
}

async fn remote_lane(
    lane: Lane,
    bridge: &mut McpBridge,
) -> Option<(tempfile::TempDir, tokio::task::JoinHandle<()>)> {
    if !matches!(lane, Lane::Remote) {
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
        bridge.storage.clone(),
        &dir.path().join("matrix.sock"),
    )
    .await;
    bridge.storage = Arc::new(remote);
    Some((dir, task))
}
async fn finish_remote(remote: Option<(tempfile::TempDir, tokio::task::JoinHandle<()>)>) {
    if let Some((_dir, task)) = remote {
        task.abort();
        let _ = task.await;
    }
}
fn receipts(f: &Fixture) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .db
        .begin_read()
        .unwrap()
        .open_table(crate::storage::control::RECEIPTS)
        .unwrap()
        .iter()
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (k.value().to_vec(), v.value().to_vec())
        })
        .collect()
}
fn ready(f: &Fixture) {
    let status = f
        .storage
        .read()
        .unwrap()
        .meta_store()
        .work_tracker()
        .status();
    assert_eq!(status.pending, 0);
    assert_eq!(status.unresolved, 0);
}
fn reopen(f: Fixture, d: &Data, layout: Layout, expected: &DiskRecord) {
    let root = f.close();
    inspect_schema(&d.path).unwrap();
    let engine = match layout {
        Layout::Legacy => RedbStorageEngine::open(&d.path).unwrap(),
        _ => {
            NativeLifecycleSession::open(
                &d.path,
                NativePolicyInputs::default(),
                Arc::new(crate::time::ManualClock::new(8 * DAY)),
            )
            .unwrap()
            .engine
        }
    };
    assert_eq!(
        engine
            .get_record(d.parent)
            .unwrap()
            .unwrap()
            .encode()
            .unwrap(),
        expected.encode().unwrap()
    );
    drop((engine, root));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_matrix_feedback_matches_resolved_planner_and_original_one() {
    for layout in LAYOUTS {
        for lane in LANES {
            for multiplier in [0., 1., 2.] {
                let (f, d, clock, mut bridge, state) =
                    matrix_setup(layout, DecayPhase::Full, multiplier, 1500.).await;
                let remote = remote_lane(lane, &mut bridge).await;
                for quality in [None, Some(1u8), Some(2), Some(3), Some(4)] {
                    let before = f.record(d.parent).unwrap();
                    let prior_receipts = receipts(&f);
                    let ns = f
                        .storage
                        .read()
                        .unwrap()
                        .get_namespace(d.ns)
                        .unwrap()
                        .unwrap();
                    let plan = plan_feedback(
                        &before,
                        &resolve_policy(&ns, &Default::default(), 1.).unwrap(),
                        8 * DAY,
                        quality.unwrap_or(3),
                    )
                    .unwrap();
                    let calls = clock.calls.load(Ordering::SeqCst);
                    let (success, value) = invoke(
                        lane,
                        &bridge,
                        &state,
                        d.parent,
                        Operation::Feedback,
                        quality.map(|q| json!(q)),
                    )
                    .await;
                    assert!(success, "{layout:?}/{lane:?}/{multiplier}: {value}");
                    assert_eq!(value["stability"].as_f64().unwrap() as f32, plan.stability);
                    assert_eq!(value["strength"].as_f64().unwrap() as f32, plan.strength);
                    let after = f.record(d.parent).unwrap();
                    assert_eq!(after.stability, plan.stability);
                    assert_eq!(after.phase, before.phase);
                    assert_eq!(after.access_history.len(), before.access_history.len() + 1);
                    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
                    assert_eq!(after.last_accessed_at, plan.timestamp);
                    let next_receipts = receipts(&f);
                    assert_eq!(next_receipts.len(), prior_receipts.len() + 1);
                    for (key, bytes) in &prior_receipts {
                        assert_eq!(next_receipts.get(key), Some(bytes));
                    }
                    let (_, added) = next_receipts
                        .iter()
                        .find(|(k, _)| !prior_receipts.contains_key(*k))
                        .unwrap();
                    let stored = crate::storage::control::validate_receipt(added).unwrap();
                    let ctx = MutationContext::unkeyed(d.ns);
                    assert_eq!(ctx.contract_version, 1);
                    let expected_hash = if matches!(layout, Layout::Legacy) {
                        crate::storage::control::request_hash(
                            &ctx,
                            "record_legacy_feedback",
                            &[d.parent.as_bytes(), &[quality.unwrap_or(3)]],
                        )
                    } else {
                        let semantic = crate::storage::control::encode(&(
                            scope(&d),
                            vec![(d.parent, crate::model::AccessKind::ManualReinforcement)],
                            Some(quality.unwrap_or(3)),
                        ))
                        .unwrap();
                        crate::storage::control::request_hash(&ctx, "native_feedback", &[&semantic])
                    };
                    assert_eq!(stored.request_hash, expected_hash);
                    ready(&f);
                }
                finish_remote(remote).await;
                let after = f.record(d.parent).unwrap();
                drop((bridge, state));
                reopen(f, &d, layout, &after);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_matrix_summary_extreme_clocks_sticky_permastore_and_forget() {
    let min = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
    let max = chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis();
    assert!(chrono::DateTime::from_timestamp_millis(min).is_some());
    assert!(chrono::DateTime::from_timestamp_millis(max).is_some());
    for layout in LAYOUTS {
        for lane in LANES {
            let (f, d, clock, mut bridge, state) =
                matrix_setup(layout, DecayPhase::Summary, 2., 5.).await;
            let remote = remote_lane(lane, &mut bridge).await;
            let original = f.record(d.parent).unwrap();
            assert_eq!(original.text_length, 0);
            assert_eq!(original.is_permastore, 0);
            assert!(original.stability < 5.);
            for (time, quality) in [(8 * DAY, 4), (min, 1), (max, 3)] {
                let before = f.record(d.parent).unwrap();
                clock.time.store(time, Ordering::SeqCst);
                let calls = clock.calls.load(Ordering::SeqCst);
                let (ok, reply) = invoke(
                    lane,
                    &bridge,
                    &state,
                    d.parent,
                    Operation::Feedback,
                    Some(json!(quality)),
                )
                .await;
                assert!(ok, "{layout:?}/{lane:?}/{time}: {reply}");
                assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
                let after = f.record(d.parent).unwrap();
                assert!(after.stability.is_finite() && after.stability > 0.);
                assert!(after.strength.is_finite() && (0. ..=1.).contains(&after.strength));
                assert_eq!(after.phase, DecayPhase::Summary);
                assert_eq!(after.text_length, 0);
                assert_eq!(after.summary, original.summary);
                assert_eq!(after.is_permastore, 1);
                if quality == 4 {
                    assert!(
                        after.stability >= 5.,
                        "feedback must cross the actual threshold"
                    );
                } else if quality == 1 {
                    assert!(
                        after.stability < 5. && after.stability < before.stability,
                        "lapse must fall below the threshold while exemption remains sticky"
                    );
                }
                assert!(after.last_accessed_at >= before.last_accessed_at);
                if let (Some(a), Some(b)) = (&after.schema, &before.schema) {
                    assert_eq!(a.lifecycle.observed_at, b.lifecycle.observed_at);
                    assert_eq!(a.lifecycle.retrieved_at, b.lifecycle.retrieved_at);
                    assert_eq!(a.lifecycle.phase_changed_at, b.lifecycle.phase_changed_at);
                    assert_eq!(a.lifecycle.timestamp_origin, b.lifecycle.timestamp_origin);
                    assert_eq!(
                        a.lifecycle.reinforced_at,
                        Some(time.max(b.lifecycle.lifecycle_updated_at))
                    );
                    assert_eq!(
                        a.lifecycle.decay_anchor_at,
                        a.lifecycle.reinforced_at.unwrap()
                    );
                }
                completion(&f).rebuild_all().await.unwrap();
                assert_eq!(f.record(d.parent).unwrap().text_length, 0);
                let receipt_state = receipts(&f);
                let read = super::super::mcp(
                    &bridge,
                    "get_memory",
                    json!({"id": d.parent, "namespace": "native", "recordAccess": false}),
                )
                .await;
                assert!(read.get("fullText").is_none_or(Value::is_null));
                assert_eq!(read["summary"], json!(original.summary));
                assert_eq!(receipts(&f), receipt_state);
                ready(&f);
            }
            // Reopen while still Summary, before terminal deletion could hide
            // accidental payload reconstruction. Reuse the same actual owner,
            // clock and adapters; this is not a second storage plane.
            let summary = f.record(d.parent).unwrap();
            finish_remote(remote).await;
            drop((bridge, state));
            let root = f.close();
            inspect_schema(&d.path).unwrap();
            let engine = match layout {
                Layout::Legacy => RedbStorageEngine::open(&d.path).unwrap(),
                _ => {
                    NativeLifecycleSession::open(
                        &d.path,
                        NativePolicyInputs::default(),
                        clock.clone(),
                    )
                    .unwrap()
                    .engine
                }
            };
            let f = Fixture::from_open_engine(root, engine);
            assert_eq!(
                f.record(d.parent).unwrap().encode().unwrap(),
                summary.encode().unwrap()
            );
            completion(&f).rebuild_all().await.unwrap();
            let (mut bridge, state) = public_adapters(&f, clock.clone());
            let remote = remote_lane(lane, &mut bridge).await;
            let read = super::super::mcp(
                &bridge,
                "get_memory",
                json!({"id": d.parent, "namespace": "native", "recordAccess": false}),
            )
            .await;
            assert!(read.get("fullText").is_none_or(Value::is_null));
            assert_eq!(read["summary"], json!(original.summary));
            let before = f.record(d.parent).unwrap();
            clock.time.store(min, Ordering::SeqCst);
            let (ok, reply) =
                invoke(lane, &bridge, &state, d.parent, Operation::Forget, None).await;
            assert!(ok, "{reply}");
            assert_eq!(reply["deleted"], true);
            let after = f.record(d.parent).unwrap();
            assert_eq!(after.phase, DecayPhase::Tombstone);
            assert!(after.summary.is_empty());
            assert_eq!(after.text_length, 0);
            if let (Some(a), Some(b)) = (&after.schema, &before.schema) {
                assert_eq!(
                    a.lifecycle.phase_changed_at,
                    b.lifecycle
                        .phase_changed_at
                        .max(b.lifecycle.lifecycle_updated_at)
                );
                assert_eq!(a.lifecycle.decay_anchor_at, b.lifecycle.decay_anchor_at);
                assert_eq!(a.lifecycle.reinforced_at, b.lifecycle.reinforced_at);
                assert!(a.vector.is_none());
            }
            let history = receipts(&f);
            let calls = clock.calls.load(Ordering::SeqCst);
            let (ok, _) = invoke(lane, &bridge, &state, d.parent, Operation::Feedback, None).await;
            assert!(!ok);
            assert_eq!(receipts(&f), history);
            assert_eq!(clock.calls.load(Ordering::SeqCst), calls);
            completion(&f).rebuild_all().await.unwrap();
            ready(&f);
            finish_remote(remote).await;
            drop((bridge, state));
            reopen(f, &d, layout, &after);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_matrix_ghost_and_existing_quality_validation_do_not_write() {
    for layout in LAYOUTS {
        for lane in LANES {
            let (f, d, clock, mut bridge, state) =
                matrix_setup(layout, DecayPhase::Ghost, 1., 1500.).await;
            let remote = remote_lane(lane, &mut bridge).await;
            let before = bytes(&f, d.parent);
            let history = receipts(&f);
            for quality in [
                None,
                Some(json!(null)),
                Some(json!(true)),
                Some(json!("3")),
                Some(json!(1.5)),
                Some(json!(0)),
                Some(json!(5)),
            ] {
                let malformed = quality.is_some();
                let (ok, reply) = invoke(
                    lane,
                    &bridge,
                    &state,
                    d.parent,
                    Operation::Feedback,
                    quality,
                )
                .await;
                assert!(!ok, "{layout:?}/{lane:?}: {reply}");
                if malformed {
                    assert!(
                        reply.to_string().to_ascii_lowercase().contains("quality"),
                        "{reply}"
                    );
                }
                assert_eq!(bytes(&f, d.parent), before);
                assert_eq!(receipts(&f), history);
            }
            assert_eq!(clock.calls.load(Ordering::SeqCst), 0);
            ready(&f);
            finish_remote(remote).await;
            let after = f.record(d.parent).unwrap();
            drop((bridge, state));
            reopen(f, &d, layout, &after);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_matrix_day_seven_recorder_then_day_eight_public_feedback() {
    use crate::search::AccessRecorder;
    // Recorder is explicit setup, NOT evidence of public retrieval routing.
    for layout in LAYOUTS {
        for lane in LANES {
            let mut stability = Vec::new();
            for exposed in [false, true] {
                let (f, d, clock, mut bridge, state) =
                    matrix_setup(layout, DecayPhase::Full, 1., 1500.).await;
                let original = f.record(d.parent).unwrap();
                if exposed {
                    clock.time.store(7 * DAY, Ordering::SeqCst);
                    let recorder = StorageAccessRecorder::new(f.storage.clone(), f.cache.clone())
                        .with_clock(clock.clone())
                        .with_completion(completion(&f));
                    let observed = recorder
                        .record_access_batch_for_publication(
                            &[(d.parent, crate::model::AccessKind::DirectRetrieval)],
                            None,
                            None,
                            None,
                            Some(&NativePolicyInputs::default()),
                            1,
                        )
                        .await
                        .unwrap();
                    assert_eq!(observed.receipts.len(), 1);
                    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
                    let after = f.record(d.parent).unwrap();
                    assert_eq!(after.stability, original.stability);
                    assert_eq!(after.phase, original.phase);
                    if let Some(schema) = after.schema {
                        assert_eq!(schema.lifecycle.retrieved_at, Some(7 * DAY));
                        assert_eq!(schema.lifecycle.reinforced_at, None);
                        assert_eq!(
                            schema.lifecycle.decay_anchor_at,
                            if matches!(layout, Layout::NativeV2) {
                                original.schema.as_ref().unwrap().lifecycle.decay_anchor_at
                            } else {
                                7 * DAY
                            }
                        );
                    }
                    completion(&f).complete().await.unwrap();
                    ready(&f);
                }
                clock.time.store(8 * DAY, Ordering::SeqCst);
                let calls = clock.calls.load(Ordering::SeqCst);
                let remote = remote_lane(lane, &mut bridge).await;
                let (ok, reply) = invoke(
                    lane,
                    &bridge,
                    &state,
                    d.parent,
                    Operation::Feedback,
                    Some(json!(3)),
                )
                .await;
                assert!(ok, "{layout:?}/{lane:?}: {reply}");
                assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
                let after = f.record(d.parent).unwrap();
                stability.push(after.stability);
                if let Some(schema) = &after.schema {
                    assert_eq!(schema.lifecycle.reinforced_at, Some(8 * DAY));
                    assert_eq!(schema.lifecycle.decay_anchor_at, 8 * DAY);
                    assert_eq!(schema.lifecycle.retrieved_at, exposed.then_some(7 * DAY));
                    assert_eq!(
                        schema.lifecycle.observed_at,
                        original.schema.unwrap().lifecycle.observed_at
                    );
                }
                ready(&f);
                finish_remote(remote).await;
                drop((bridge, state));
                reopen(f, &d, layout, &after);
            }
            if matches!(layout, Layout::NativeV2) {
                assert_eq!(stability[0], stability[1]);
            } else {
                assert_ne!(stability[0], stability[1]);
            }
        }
    }
}
