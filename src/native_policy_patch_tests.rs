//! #237: `update_namespace_policy` changes an existing namespace's decay and
//! `namespace_stats`/`list_namespaces` report what the sweep will use.
use super::*;
use crate::mcp::bridge::McpBridge;

fn bridge(f: &Fixture, provider: Arc<Provider>, inputs: NativePolicyInputs) -> McpBridge {
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = Arc::new(f.namespace_adapter().with_native_catalog(
        provider,
        inputs,
        Arc::new(NoPolicyClock),
    ));
    bridge
}

async fn call(bridge: &McpBridge, tool: &str, args: Value) -> (bool, Value) {
    let result = crate::mcp::tools::dispatch_tool(bridge, tool, args).await;
    let error = result.is_error == Some(true);
    let value = serde_json::to_value(&result).unwrap();
    let text = value["content"][0]["text"].as_str().unwrap().to_owned();
    (
        error,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

fn num(value: &Value) -> f32 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("not a number: {value}")) as f32
}

fn namespace(f: &Fixture) -> crate::model::NamespaceConfig {
    f.storage
        .read()
        .unwrap()
        .get_namespace_by_name("default")
        .unwrap()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_namespace_policy_changes_default_and_the_sweep_policy_follows() {
    let (f, _path, provider) = fixture_with_record(true);
    let bridge = bridge(&f, provider, NativePolicyInputs::default());
    // The migrated fixture has not published its derived state yet.
    f.completion().rebuild_all().await.unwrap();

    // Built-in values: about 16 days to Summary and 160 to Ghost (#237).
    let (error, before) = call(&bridge, "namespace_stats", json!({})).await;
    assert!(!error, "{before}");
    let p = &before["policy"];
    assert!(p["decayRateMultiplier"].is_null());
    assert_eq!(num(&p["effectiveDecayRateMultiplier"]), 1.0);
    assert_eq!(num(&p["daysUntilSummary"]).round(), 16.0);
    assert_eq!(num(&p["daysUntilGhost"]).round(), 160.0);

    // Namespace omitted: the session default, `default`.
    let (error, reply) = call(
        &bridge,
        "update_namespace_policy",
        json!({"decayRateMultiplier": 2, "phaseThresholds": {"fullToSummary": 0.8}}),
    )
    .await;
    assert!(!error, "{reply}");
    assert_eq!(reply["data"]["changed"], true);
    assert_eq!(
        reply["receipt"]["outcome"]["kind"],
        "nativeNamespacePolicyUpdated"
    );
    let policy = &reply["data"]["policy"];
    assert_eq!(num(&policy["decayRateMultiplier"]), 2.0);
    assert_eq!(num(&policy["phaseThresholds"]["fullToSummary"]), 0.8);
    // Fields the call did not name keep their values.
    assert_eq!(num(&policy["phaseThresholds"]["summaryToGhost"]), 0.3);
    assert_eq!(num(&policy["initialStability"]), 3.7145);

    let ns = namespace(&f);
    assert_eq!(ns.decay_rate_multiplier, Some(2.0));
    assert_eq!(ns.phase_thresholds.full_to_summary, 0.8);
    // The native sweep and feedback resolve through exactly this.
    let resolved = NativePolicyInputs::default().resolve(&ns).unwrap();
    assert_eq!(resolved.decay_rate_multiplier, 2.0);
    assert_eq!(resolved.config.phase_1_threshold, 0.8);

    let (error, stats) = call(&bridge, "namespace_stats", json!({"namespace": "default"})).await;
    assert!(!error, "{stats}");
    let p = &stats["policy"];
    assert_eq!(num(&p["effectiveDecayRateMultiplier"]), 2.0);
    let days = num(&p["daysUntilSummary"]);
    assert!(num(&p["daysUntilGhost"]) > days);
    // A never-recalled memory reaches fullToSummary at exactly that age
    // under the projection the sweep runs.
    let at = crate::decay::lifecycle::project(
        ns.initial_stability,
        0,
        &resolved,
        (days as f64 * 86_400_000.0) as i64,
        0.0,
    )
    .unwrap();
    assert!((at.strength - 0.8).abs() < 1e-3, "{}", at.strength);

    let (error, list) = call(&bridge, "list_namespaces", json!({})).await;
    assert!(!error, "{list}");
    let listed = list
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "default")
        .unwrap();
    assert_eq!(listed["policy"], stats["policy"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_namespace_policy_rejects_bad_changes_without_writing() {
    let (f, _path, provider) = fixture_with_record(true);
    let bridge = bridge(&f, provider, NativePolicyInputs::default());
    let before = f.storage.read().unwrap().control_state().unwrap();
    for args in [
        json!({}),
        json!({"multiplier": 2}),
        json!({"phaseThresholds": {"fullToSummary": 0.2}}),
        json!({"phaseThresholds": {"other": 0.2}}),
        json!({"initialStability": -1}),
        json!({"namespace": "missing", "initialStability": 2}),
    ] {
        let (error, reply) = call(&bridge, "update_namespace_policy", args.clone()).await;
        assert!(error, "{args} -> {reply}");
    }
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clearing_an_explicit_multiplier_binds_the_configured_one() {
    let (f, path, provider) = fixture_with_record(true);
    let configured = NativePolicyInputs {
        multiplier: 1.5,
        ..Default::default()
    };
    let bridge = bridge(&f, provider, configured.clone());
    for multiplier in [json!(2), Value::Null] {
        let (error, reply) = call(
            &bridge,
            "update_namespace_policy",
            json!({"namespace": "default", "decayRateMultiplier": multiplier}),
        )
        .await;
        assert!(!error, "{reply}");
        assert_eq!(reply["data"]["changed"], true);
    }
    let ns = namespace(&f);
    assert_eq!(ns.decay_rate_multiplier, None);
    let binding = ns.schema.as_ref().unwrap().inherited_decay.clone().unwrap();
    assert_eq!(binding.multiplier_bits, 1.5f32.to_bits());
    // Usable at once with the configured value, and bound to it.
    assert_eq!(configured.resolve(&ns).unwrap().decay_rate_multiplier, 1.5);
    assert!(
        NativePolicyInputs {
            multiplier: 3.0,
            ..Default::default()
        }
        .resolve(&ns)
        .is_err()
    );
    let id = f.storage.read().unwrap().scan_all().unwrap()[0].0;
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .capture_feedback(id, Some(ns.id), 3, &configured)
            .is_ok()
    );
    let (_, stats) = call(&bridge, "namespace_stats", json!({})).await;
    assert!(stats["policy"]["decayRateMultiplier"].is_null());
    assert_eq!(num(&stats["policy"]["effectiveDecayRateMultiplier"]), 1.5);

    // The receipt chain, with the new binding, validates on inspection,
    // reopen and a logical export/import round trip.
    drop(bridge);
    let dir = f.close();
    inspect_schema(&path).unwrap();
    let session = NativeLifecycleSession::open(&path, configured, Arc::new(FixedClock)).unwrap();
    drop(session);
    let stream = dir.path().join("policy.jsonl");
    assert_eq!(
        crate::storage::logical_export::export_store(&path, &stream).unwrap(),
        crate::storage::logical_export::import_store(&stream, &dir.path().join("copy")).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_namespace_policy_reaches_the_daemon_over_its_socket() {
    let (f, _path, provider) = fixture_with_record(true);
    let adapter = f.namespace_adapter().with_native_catalog(
        provider,
        NativePolicyInputs::default(),
        Arc::new(NoPolicyClock),
    );
    let socket = tempfile::tempdir().unwrap();
    let (remote, task) =
        crate::daemon::server::catalog_test_remote(Arc::new(adapter), &socket.path().join("s"))
            .await;
    let mut bridge = crate::test_support::StubBridge::default().into_bridge();
    bridge.namespaces = remote;
    let (error, reply) = call(
        &bridge,
        "update_namespace_policy",
        json!({"initialStability": 7}),
    )
    .await;
    assert!(!error, "{reply}");
    assert_eq!(reply["data"]["changed"], true);
    assert_eq!(namespace(&f).initial_stability, 7.0);
    let (error, reply) = call(&bridge, "update_namespace_policy", json!({"bogus": 1})).await;
    assert!(error, "{reply}");
    drop(bridge);
    task.abort();
}
