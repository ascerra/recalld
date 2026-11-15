use super::*;
use crate::storage::StorageEngine as _;
use crate::storage::batch::{BatchInput, LegacyBatchOrigin};
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
