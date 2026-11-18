use super::*;
use crate::api::NamespaceRegistry as _;
use crate::mcp::bridge::{CreateNamespaceInput, NamespaceRegistry as _};
use crate::storage::StorageEngine as _;
use crate::test_support::Fixture;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

struct CountingClock(AtomicI64);
impl crate::time::Clock for CountingClock {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

#[tokio::test]
async fn owned_namespace_clock_samples_once_for_response_canonical_row_and_receipt() {
    let f = Fixture::new();
    let clock = Arc::new(CountingClock(AtomicI64::new(1_767_225_660_000)));
    let mcp = f.namespace_adapter().with_clock(clock.clone());
    let rest = crate::api::adapters::NamespaceRegistryAdapter::new(
        f.storage.clone(),
        std::sync::Arc::new(crate::embedding::PassthroughProvider::new(
            crate::test_support::DIM,
        )),
    )
    .with_completion(f.completion())
    .with_clock(clock.clone());
    let response = mcp
        .create_namespace(CreateNamespaceInput {
            name: "clock-mcp".into(),
            embedding_dim: Some(4),
            initial_stability: None,
            desired_retention: None,
            decay_rate_multiplier: None,
        })
        .await
        .unwrap();
    assert_eq!(response.created_at, "2026-01-01 00:01:00 UTC");
    assert_eq!(clock.0.load(Ordering::SeqCst), 1_767_225_660_001);
    let response = rest
        .create("clock-rest", Some(4), 3.7145, 0.9, None)
        .await
        .unwrap();
    assert_eq!(response.created_at, 1_767_225_660_001);
    assert_eq!(clock.0.load(Ordering::SeqCst), 1_767_225_660_002);
    let engine = f.storage.read().unwrap();
    let read = engine.meta_store().db.begin_read().unwrap();
    let receipts = read.open_table(control::RECEIPTS).unwrap();
    for (name, expected) in [
        ("clock-mcp", 1_767_225_660_000),
        ("clock-rest", 1_767_225_660_001),
    ] {
        let ns = engine.get_namespace_by_name(name).unwrap().unwrap();
        assert_eq!(ns.created_at, expected);
        let matching: Vec<_> = receipts
            .iter()
            .unwrap()
            .filter_map(|r| {
                let (_, v) = r.unwrap();
                let receipt = control::validate_receipt(v.value()).unwrap().receipt;
                match receipt.outcome {
                    MutationOutcome::NamespaceCreated {
                        namespace_id,
                        created_at,
                    } if namespace_id == ns.id => Some(created_at),
                    _ => None,
                }
            })
            .collect();
        assert_eq!(matching, vec![expected]);
    }
}
