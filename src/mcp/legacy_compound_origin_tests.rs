//! Actual legacy-disk consumers and genuine pre-extension format1 fixtures.
use super::*;
use crate::mcp::bridge::StorageEngine as _;
use crate::storage::batch::{BatchInput, BatchKind};
use crate::storage::control::{MutationContext, MutationScope};
use crate::storage::insertion::InsertMemory;
use crate::test_support::Fixture;
use serde_json::{Value, json};

fn request(f: &Fixture, key: &str, version: u32, items: Vec<Value>) -> BatchInput {
    BatchInput {
        request_context: None,
        expected_generation: None,
        result_version: 1,
        mutation_version: version,
        kind: BatchKind::Insert,
        items,
        default_namespace: "default".into(),
        store_id: Some(f.storage.read().unwrap().control_state().unwrap().store_id),
        idempotency_key: Some(key.into()),
        batch_operation_id: None,
    }
}
fn item() -> Value {
    json!({"summary":"legacy original intent","fullText":"synthetic retained text",
        "embedding":[1.,0.,0.,0.],"checkDuplicates":false})
}
fn current(f: &Fixture) -> crate::storage::control::ControlState {
    f.storage.read().unwrap().control_state().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_actual_single_insert_two() {
    let f = Fixture::new();
    let owner = f.storage_adapter().batch_completion();
    let before = current(&f);
    let ctx = MutationContext {
        contract_version: 2,
        store_id: Some(before.store_id),
        scope: MutationScope::Namespace(NamespaceId::new(1)),
        idempotency_key: Some("actual-legacy-insert2".into()),
        expected_generation: Some(before.committed_generation),
    };
    let input = InsertMemory {
        id: None,
        summary: "legacy API2".into(),
        full_text: Some("synthetic".into()),
        tags: vec![],
        initial_stability: None,
        created_at: None,
        embedding: Some(vec![1., 0., 0., 0.]),
        parent_id: None,
        supersedes: None,
    };
    let result = owner.insert(ctx, input, vec![].into(), 100).await;
    assert!(
        result.is_ok(),
        "actual legacy2 insertion rejected: {:?}",
        result.as_ref().err()
    );
    assert_eq!(
        current(&f).committed_generation.get(),
        before.committed_generation.get() + 1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compound_origin_actual_batch_two() {
    let f = Fixture::new();
    let input = request(&f, "actual-legacy-batch2", 4, vec![item()]);
    let result = f.storage_adapter().mutate_batch(input).await;
    assert!(result.is_ok(), "actual legacy2 batch rejected: {result:?}");
    assert!(result.unwrap().all_ready());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "genuine BEFORE exporter; only run on recorded pre-extension source"]
async fn legacy_compound_origin_export_genuine_old_one() {
    let Ok(destination) = std::env::var("RECALLD_LEGACY_COMPOUND_BEFORE") else {
        return;
    };
    let f = Fixture::new();
    let adapter = f.storage_adapter();
    let mut cases = Vec::new();
    for with_cas in [false, true] {
        for invalid in [false, true] {
            let key = format!("genuine-one-{with_cas}-{invalid}");
            let before = current(&f);
            let mut input = request(
                &f,
                &key,
                1,
                vec![if invalid {
                    json!({"summary":""})
                } else {
                    item()
                }],
            );
            input.expected_generation = with_cas.then_some(before.committed_generation);
            let outcome = adapter.mutate_batch(input.clone()).await.unwrap();
            assert_eq!(
                outcome
                    .reservation_receipt
                    .as_ref()
                    .unwrap()
                    .committed_generation
                    .get(),
                before.committed_generation.get() + 1
            );
            assert_eq!(
                current(&f).committed_generation.get(),
                before.committed_generation.get() + if invalid { 1 } else { 2 }
            );
            assert_eq!(outcome.failed, usize::from(invalid));
            assert_eq!(outcome.succeeded, usize::from(!invalid));
            let replay = adapter.mutate_batch(input.clone()).await.unwrap();
            assert_eq!(replay.batch_operation_id, outcome.batch_operation_id);
            cases.push(json!({"input":input,"outcome":outcome,"before":before}));
        }
    }
    drop(adapter);
    let root = f.close();
    // Real backup of completed old-format work, not hand-written canonical rows.
    crate::backup::run_backup(
        &Default::default(),
        std::path::Path::new(&destination),
        Some(root.path()),
        false,
    )
    .await
    .unwrap();
    let receipts = historical_rows(root.path());
    std::fs::write(
        format!("{destination}.json"),
        serde_json::to_vec_pretty(&json!({"cases":cases,"rows":receipts})).unwrap(),
    )
    .unwrap();
}

fn historical_rows(root: &std::path::Path) -> Value {
    use redb::ReadableTable;
    let db = redb::Database::open(root.join("meta.db")).unwrap();
    let tx = db.begin_read().unwrap();
    let mut rows = serde_json::Map::new();
    for name in ["mutation_receipts", "mutation_batches"] {
        let table = tx
            .open_table(redb::TableDefinition::<&[u8], &[u8]>::new(name))
            .unwrap();
        let items: Vec<_> = table
            .iter()
            .unwrap()
            .map(|row| {
                let (k, v) = row.unwrap();
                json!({"key":k.value(),"value":String::from_utf8(v.value().to_vec()).unwrap()})
            })
            .collect();
        rows.insert(name.into(), json!(items));
    }
    Value::Object(rows)
}
