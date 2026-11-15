use super::*;
use crate::storage::StorageEngine as _;
use crate::storage::batch::{BatchInput, LegacyBatchOrigin};
use crate::storage::control::Generation;
use crate::test_support::Fixture;
use serde_json::{Value, json};

fn request(f: &Fixture, items: Vec<Value>, cas: bool) -> BatchInput {
    let c = f.storage.read().unwrap().control_state().unwrap();
    BatchInput {
        request_context: None,
        expected_generation: cas.then_some(c.committed_generation),
        result_version: 1,
        mutation_version: 4,
        kind: BatchKind::Insert,
        items,
        default_namespace: "default".into(),
        store_id: Some(c.store_id),
        idempotency_key: Some("legacy-four".into()),
        batch_operation_id: None,
    }
}
fn plans(input: &BatchInput) -> Vec<BatchItemPlan> {
    crate::mcp::batches::prepare(input)
        .unwrap()
        .into_iter()
        .map(|p| p.plan)
        .collect()
}
fn reserve(f: &Fixture, input: &BatchInput) -> BatchProgress {
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .reserve_batch_version_with_work(
            &input.context(),
            4,
            input.kind,
            &plans(input),
            None,
            None,
            || Ok(None),
        )
        .unwrap()
}
fn item() -> Value {
    json!({"summary":"legacy4 proof","fullText":"synthetic","embedding":[1.,0.,0.,0.],"checkDuplicates":false})
}
fn replace(f: &Fixture, p: &BatchProgress, value: &Value) {
    let s = f.storage.read().unwrap();
    let tx = s.meta_store().db.begin_write().unwrap();
    tx.open_table(BATCHES)
        .unwrap()
        .insert(
            p.reservation_receipt.operation_id.as_bytes().as_slice(),
            serde_json::to_vec(value).unwrap().as_slice(),
        )
        .unwrap();
    tx.commit().unwrap();
}

#[test]
fn legacy_compound_origin_required_nullable_codec() {
    let valid = json!({"proofVersion":1,"contractVersion":2,"expectedGeneration":null});
    assert_eq!(
        serde_json::from_value::<LegacyBatchOrigin>(valid.clone())
            .unwrap()
            .expected_generation,
        None
    );
    let mut missing = valid.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("expectedGeneration");
    assert!(serde_json::from_value::<LegacyBatchOrigin>(missing).is_err());
    for value in [
        json!(0),
        json!("01"),
        json!("-1"),
        json!("9223372036854775808"),
    ] {
        let mut bad = valid.clone();
        bad["expectedGeneration"] = value;
        assert!(serde_json::from_value::<LegacyBatchOrigin>(bad).is_err());
    }
    assert!(serde_json::from_str::<LegacyBatchOrigin>(r#"{"proofVersion":1,"contractVersion":2,"expectedGeneration":null,"expectedGeneration":"0"}"#).is_err());
    let mut extra = valid;
    extra["extra"] = json!(1);
    assert!(serde_json::from_value::<LegacyBatchOrigin>(extra).is_err());
}

#[test]
fn legacy_compound_origin_anchor_precedes_children_and_rejects_stripping() {
    for all_invalid in [false, true] {
        for mode in 0..8 {
            let f = Fixture::new();
            let input = request(&f, vec![if all_invalid { json!(17) } else { item() }], true);
            let p = reserve(&f, &input);
            assert_eq!(
                p.reservation_receipt.committed_generation.get(),
                input.expected_generation.unwrap().get() + 1
            );
            let mut value = serde_json::to_value(&p).unwrap();
            match mode {
                0 => {
                    value.as_object_mut().unwrap().remove("legacyOrigin");
                }
                1 => value["legacyOrigin"] = Value::Null,
                2 => {
                    value["legacyOrigin"]
                        .as_object_mut()
                        .unwrap()
                        .remove("expectedGeneration");
                }
                3 => value["legacyOrigin"]["expectedGeneration"] = Value::Null,
                4 => value["legacyOrigin"]["contractVersion"] = json!(1),
                5 => value["legacyOrigin"]["proofVersion"] = json!(2),
                6 => {
                    value["version"] = json!(1);
                    value.as_object_mut().unwrap().remove("legacyOrigin");
                }
                7 => value["requestContext"] = json!({"proofVersion":1,"contractVersion":1}),
                _ => unreachable!(),
            }
            replace(&f, &p, &value);
            let root = f.close();
            let before = std::fs::read(root.path().join("meta.db")).unwrap();
            assert!(
                crate::storage::RedbStorageEngine::open(root.path()).is_err(),
                "mode{mode},invalid{all_invalid}"
            );
            assert_eq!(std::fs::read(root.path().join("meta.db")).unwrap(), before);
            assert!(crate::storage::validation::canonical_files(root.path()).is_err());
            assert_eq!(std::fs::read(root.path().join("meta.db")).unwrap(), before);
        }
    }
}

#[test]
fn legacy_compound_origin_final_transaction_rechecks_parent_anchor() {
    let f = Fixture::new();
    let input = request(&f, vec![item()], true);
    let p = reserve(&f, &input);
    let mut child = p.child(0).unwrap();
    let mut bad = p.clone();
    bad.legacy_origin.as_mut().unwrap().expected_generation = None;
    // Change both mutable copies consistently; immutable parent hash must win.
    child.legacy_origin = bad.legacy_origin.clone();
    replace(&f, &p, &serde_json::to_value(bad).unwrap());
    let before = f.storage.read().unwrap().control_state().unwrap();
    let payload = std::fs::read(f.path().join("fulltext.dat")).unwrap();
    let plans = plans(&input);
    let BatchItemPlan::Insert { input, .. } = &plans[0] else {
        panic!()
    };
    let error = f
        .storage
        .write()
        .unwrap()
        .insert_memory_reserved(&child.context(), input, &[], 100, Some(&child))
        .unwrap_err();
    assert!(matches!(error, StorageError::InvalidControl(_)), "{error}");
    assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    assert_eq!(
        std::fs::read(f.path().join("fulltext.dat")).unwrap(),
        payload
    );
    assert!(f.storage.read().unwrap().scan_all().unwrap().is_empty());
}

#[test]
fn legacy_compound_origin_child_context_policy_and_cas_are_final_guarded() {
    let f = Fixture::new();
    let input = request(&f, vec![item()], false);
    let p = reserve(&f, &input);
    let child = p.child(0).unwrap();
    let plans = plans(&input);
    let BatchItemPlan::Insert { input, .. } = &plans[0] else {
        panic!()
    };
    let before = f.storage.read().unwrap().control_state().unwrap();
    for mode in 0..3 {
        let mut ctx = child.context();
        match mode {
            0 => ctx.contract_version = 1,
            1 => ctx.expected_generation = Some(before.committed_generation),
            2 => ctx.scope = MutationScope::Namespace(NamespaceId::new(99)),
            _ => unreachable!(),
        }
        assert!(
            f.storage
                .write()
                .unwrap()
                .insert_memory_reserved(&ctx, input, &[], 100, Some(&child))
                .is_err()
        );
        assert_eq!(f.storage.read().unwrap().control_state().unwrap(), before);
    }
    let mut config = f
        .storage
        .read()
        .unwrap()
        .get_namespace(NamespaceId::new(1))
        .unwrap()
        .unwrap();
    config.initial_stability += 1.;
    // Synthetic concurrent canonical-policy change before final child guard.
    {
        let s = f.storage.read().unwrap();
        let tx = s.meta_store().db.begin_write().unwrap();
        tx.open_table(NAMESPACE_TABLE)
            .unwrap()
            .insert(
                config.id.get(),
                control::encode(&config).unwrap().as_slice(),
            )
            .unwrap();
        tx.commit().unwrap();
    }
    assert!(
        f.storage
            .write()
            .unwrap()
            .insert_memory_reserved(&child.context(), input, &[], 100, Some(&child))
            .is_err()
    );
    assert!(f.storage.read().unwrap().scan_all().unwrap().is_empty());
}

fn process_request(engine: &crate::storage::RedbStorageEngine, expected: Generation) -> BatchInput {
    BatchInput {
        request_context: None,
        expected_generation: Some(expected),
        result_version: 1,
        mutation_version: 4,
        kind: BatchKind::Insert,
        items: vec![item()],
        default_namespace: "default".into(),
        store_id: Some(engine.control_state().unwrap().store_id),
        idempotency_key: Some("legacy-four-process".into()),
        batch_operation_id: None,
    }
}
#[test]
fn legacy_compound_origin_process_child() {
    let Some(path) = std::env::var_os("RECALLD_LEGACY_ORIGIN_STORE") else {
        return;
    };
    let expected: Generation = std::env::var("RECALLD_LEGACY_ORIGIN_CAS")
        .unwrap()
        .try_into()
        .unwrap();
    let mut engine = crate::storage::RedbStorageEngine::open(path).unwrap();
    let input = process_request(&engine, expected);
    let plans = plans(&input);
    let p = engine
        .meta_store()
        .reserve_batch_version_with_work(
            &input.context(),
            4,
            input.kind,
            &plans,
            None,
            None,
            || Ok(None),
        )
        .unwrap();
    let child = p.child(0).unwrap();
    let BatchItemPlan::Insert { input, .. } = &plans[0] else {
        panic!()
    };
    engine
        .insert_memory_reserved(&child.context(), input, &[], 100, Some(&child))
        .unwrap();
    panic!("process boundary was not reached")
}

#[test]
fn legacy_compound_origin_process_exit_reconciles_original_cas_and_child() {
    for point in [
        "before_batch_commit",
        "after_batch_commit",
        "after_insert_payload_sync",
        "before_metadata_commit",
        "after_metadata_commit",
        "after_edge_commit",
        "after_canonical_ack",
    ] {
        let f = Fixture::new();
        let before = f.storage.read().unwrap().control_state().unwrap();
        let root = f.close();
        let result=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","storage::metadata::batch::legacy_origin_tests::legacy_compound_origin_process_child","--nocapture"])
            .env("RECALLD_LEGACY_ORIGIN_STORE",root.path()).env("RECALLD_LEGACY_ORIGIN_CAS",before.committed_generation.get().to_string())
            .env("RECALLD_TEST_EFFECT_CRASH",point).output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(91),
            "{point}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let parent_committed = point != "before_batch_commit";
        let child_committed = matches!(
            point,
            "after_metadata_commit" | "after_edge_commit" | "after_canonical_ack"
        );
        let mut engine = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
        assert_eq!(
            engine.control_state().unwrap().committed_generation.get(),
            before.committed_generation.get()
                + u64::from(parent_committed)
                + u64::from(child_committed)
        );
        let input = process_request(&engine, before.committed_generation);
        let plans = plans(&input);
        let p = engine
            .meta_store()
            .reserve_batch_version_with_work(
                &input.context(),
                4,
                input.kind,
                &plans,
                None,
                None,
                || Ok(None),
            )
            .unwrap();
        assert_eq!(p.reservation_receipt.replayed, parent_committed);
        let child = p.child(0).unwrap();
        let BatchItemPlan::Insert { input, .. } = &plans[0] else {
            panic!()
        };
        let receipt = engine
            .insert_memory_reserved(&child.context(), input, &[], 100, Some(&child))
            .unwrap();
        assert_eq!(receipt.replayed, child_committed);
        assert_eq!(receipt.operation_id, p.items[0].operation_id);
        assert_eq!(engine.scan_all().unwrap().len(), 1);
        assert_eq!(engine.meta_store().pending_batch_items().unwrap(), 0);
        drop(engine);
        let mut engine = crate::storage::RedbStorageEngine::open(root.path()).unwrap();
        let mut replay = engine
            .insert_memory_reserved(&child.context(), input, &[f32::NAN], i64::MAX, Some(&child))
            .unwrap();
        replay.replayed = receipt.replayed;
        assert_eq!(replay, receipt);
        assert_eq!(
            engine.control_state().unwrap().committed_generation.get(),
            before.committed_generation.get() + 2
        );
    }
}

#[tokio::test]
async fn legacy_compound_origin_explicit_endpoints_rechecked_after_reservation() {
    let f = Fixture::new();
    let parent = f.insert_memory().await;
    let old = f.insert_memory().await;
    let other = f.create_namespace("other");
    let foreign = f.insert_memory_in(other, &[]).await;
    let mut value = item();
    value["parentId"] = json!(parent);
    value["supersedes"] = json!(old);
    let request = request(&f, vec![value], false);
    let p = reserve(&f, &request);
    let child = p.child(0).unwrap();
    let prepared = plans(&request);
    let BatchItemPlan::Insert { input, .. } = &prepared[0] else {
        panic!()
    };
    let receipt = f
        .storage
        .write()
        .unwrap()
        .insert_memory_reserved(&child.context(), input, &[], 100, Some(&child))
        .unwrap();
    assert!(matches!(
        receipt.outcome,
        MutationOutcome::MemoryInserted { edges_put: 2, .. }
    ));
    let mut engine = f.storage.write().unwrap();
    let before = engine.control_state().unwrap();
    let edges = engine.load_all_edges().unwrap();
    let text = std::fs::read(f.path().join("fulltext.dat")).unwrap();
    for mode in 0..4 {
        let mut input = input.clone();
        match mode {
            0 => {} // Supersedes branch: old was replaced after the caller selected it.
            1 => {
                input.supersedes = None;
                input.parent_id = Some(foreign);
            }
            2 => {
                input.supersedes = None;
                input.parent_id = Some(MemoryId::new());
            }
            3 => {
                input.id = Some(parent);
                input.parent_id = Some(parent);
                input.supersedes = None;
            }
            _ => unreachable!(),
        }
        let mut ctx = MutationContext::unkeyed(NamespaceId::new(1));
        ctx.contract_version = 2;
        assert!(
            engine
                .insert_memory_with_receipt(&ctx, &input, &[], 100)
                .is_err()
        );
        assert_eq!(engine.control_state().unwrap(), before);
        assert_eq!(engine.load_all_edges().unwrap(), edges);
        assert_eq!(std::fs::read(f.path().join("fulltext.dat")).unwrap(), text);
    }
    drop(engine);
    // Real endpoint deletion after reservation; the cached graph remains stale.
    let lost = f.insert_memory().await;
    let mut value = item();
    value["parentId"] = json!(lost);
    let mut input = request.clone();
    input.idempotency_key = Some("deleted-endpoint".into());
    input.items = vec![value];
    let p = reserve(&f, &input);
    let child = p.child(0).unwrap();
    let plans = plans(&input);
    let BatchItemPlan::Insert { input, .. } = &plans[0] else {
        panic!()
    };
    let mut engine = f.storage.write().unwrap();
    engine.delete_memory(lost).unwrap();
    let before = engine.control_state().unwrap();
    let text = std::fs::read(f.path().join("fulltext.dat")).unwrap();
    assert!(matches!(
        engine.insert_memory_reserved(&child.context(), input, &[], 100, Some(&child)),
        Err(StorageError::NotFound(_))
    ));
    assert_eq!(engine.control_state().unwrap(), before);
    assert_eq!(std::fs::read(f.path().join("fulltext.dat")).unwrap(), text);
}
