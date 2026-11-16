//! Real post-TX certification failure, not a lost commit ACK or authored receipt.
use super::*;
use crate::serialization::batch_api1::{Reply, Surface};
use crate::storage::batch::{BatchItemState, BatchStatus};
use redb::ReadableTable;

async fn postcommit(nth: usize, remote: bool, successful_prefix: bool) {
    let (d, f, a, provider, clock) = setup().await;
    let before = f.storage.read().unwrap().control_state().unwrap();
    let tracker = f.storage.read().unwrap().meta_store().work_tracker();
    let missing = MemoryId::new();
    let mut items = vec![
        json!({"summary":"rejected after reservation", "parentId":missing,
        "embedding":[1.,0.,0.,0.],"embeddingProfileId":d.profile.profile_id().unwrap(),"checkDuplicates":false});
        nth.saturating_sub(1).max(1)
    ];
    if successful_prefix {
        items[0].as_object_mut().unwrap().remove("parentId");
        items.push(json!({"summary":"must stay pending", "checkDuplicates":false}));
    }
    let request = api1_request(&f, items);
    f.storage
        .read()
        .unwrap()
        .meta_store()
        .vector_publication_failure_nth
        .store(nth, Ordering::SeqCst);
    let reply = if remote {
        let socket = tempfile::tempdir().unwrap();
        let (remote, task) = crate::daemon::server::native_lifecycle_test_remote(
            a.clone(),
            &socket.path().join("s.sock"),
        )
        .await;
        let reply = remote.store_batch_api1(request.clone()).await;
        drop(remote);
        task.await.unwrap();
        reply
    } else {
        a.store_batch_api1(request.clone()).await
    };
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(
        f.storage
            .read()
            .unwrap()
            .meta_store()
            .ensure_available()
            .is_err()
    );
    assert!(tracker.status().unresolved > 0);
    drop(a);
    let root = f.close();
    let reopened = open(&d, clock.clone());
    let after = reopened.metadata().control_state().unwrap();
    assert_eq!(
        after.committed_generation.get(),
        before.committed_generation.get() + nth as u64
    );
    let mut durable = {
        let tx = reopened.metadata().db.begin_read().unwrap();
        tx.open_table(control::RECEIPTS)
            .unwrap()
            .iter()
            .unwrap()
            .map(|row| control::validate_receipt(row.unwrap().1.value()).unwrap())
            .filter(|stored| stored.receipt.committed_generation > before.committed_generation)
            .map(|stored| stored.receipt)
            .collect::<Vec<_>>()
    };
    durable.sort_by_key(|r| r.committed_generation);
    assert_eq!(durable.len(), nth);
    let reservation = &durable[0];
    assert!(matches!(
        reservation.outcome,
        MutationOutcome::BatchReserved { .. }
    ));
    let progress = reopened
        .metadata()
        .lookup_batch(before.store_id, reservation.operation_id)
        .unwrap()
        .unwrap();
    assert_eq!(progress.original_contract().unwrap(), 1);
    assert_eq!(&progress.reservation_receipt, reservation);
    let bookkeeping: Vec<_> = durable
        .iter()
        .filter(|r| matches!(r.outcome, MutationOutcome::BatchItemRejected { .. }))
        .cloned()
        .collect();
    assert_eq!(progress.bookkeeping_receipts, bookkeeping);
    for receipt in std::iter::once(reservation).chain(bookkeeping.iter()) {
        assert_eq!(
            reopened
                .metadata()
                .lookup_operation(
                    before.store_id,
                    control::MutationScope::Catalog,
                    receipt.operation_id
                )
                .unwrap()
                .as_ref(),
            Some(receipt)
        );
        assert!(
            reopened
                .metadata()
                .lookup_operation(
                    before.store_id,
                    control::MutationScope::Namespace(d.ns),
                    receipt.operation_id
                )
                .unwrap()
                .is_none()
        );
    }
    for (index, item) in progress.items.iter().enumerate() {
        if nth == 1 || (successful_prefix && index == 2) {
            assert!(matches!(item.state, BatchItemState::NotAttempted {}));
        } else if successful_prefix && index == 0 {
            assert!(matches!(item.state, BatchItemState::Ok { .. }));
        } else {
            assert!(
                matches!(&item.state, BatchItemState::Error { error } if error.code == "NOT_FOUND")
            );
        }
    }
    eprintln!(
        "strict reopen proved nth={nth} remote={remote} G={} receipts={} bookkeeping={}",
        after.committed_generation.get(),
        durable.len(),
        bookkeeping.len()
    );
    // Actual durable proof above precedes the RED response-retention assertions.
    if nth == 1 {
        let Reply::Owned(Err(error)) = &reply else {
            panic!("reservation response: {reply:?}")
        };
        assert_eq!(
            error.reservation_receipt.as_ref(),
            Some(reservation),
            "reservation proof misclassified: {error:?}"
        );
        assert!(error.receipt.is_none());
        assert_eq!(
            error.error.commit_state,
            crate::serialization::contract::CommitState::Committed
        );
        assert!(error.readiness.is_none());
    } else {
        let Reply::Owned(Ok(out)) = &reply else {
            panic!("rejection response: {reply:?}")
        };
        assert_eq!(
            out.outcome.bookkeeping_receipts, bookkeeping,
            "known rejection receipt lost"
        );
        assert_eq!(out.outcome.reservation_receipt.as_ref(), Some(reservation));
        assert!(out.outcome.unknown_bookkeeping_operations.is_empty());
        for (actual, retained) in out.outcome.items.iter().zip(&progress.items) {
            assert_eq!(actual.operation_id, Some(retained.operation_id));
            assert!(!actual.ready);
            match &retained.state {
                BatchItemState::Error { error } => {
                    assert_eq!(actual.status, BatchStatus::Error);
                    assert_eq!(actual.error.as_ref(), Some(error));
                    assert!(actual.receipt.is_none());
                    assert!(!actual.retryable);
                }
                BatchItemState::Ok { receipt } => {
                    assert_eq!(actual.status, BatchStatus::Ok);
                    assert_eq!(actual.receipt.as_ref(), Some(receipt));
                }
                BatchItemState::NotAttempted {} => {
                    assert_eq!(actual.status, BatchStatus::NotAttempted)
                }
            }
        }
        assert!(!out.readiness.as_ref().is_some_and(|r| r.ready));
    }
    let wire = serde_json::to_value(&reply).unwrap();
    let (status, value) = Reply::decode(wire.clone(), &request).response(Surface::Rest);
    assert_eq!(
        status,
        if nth == 1 {
            503
        } else if successful_prefix {
            207
        } else {
            422
        }
    );
    assert_eq!(value["error"]["commitState"], "committed");
    if nth == 1 {
        assert_eq!(value["commitEvidenceScope"], "reservation");
        assert_eq!(value["reservationReceipt"], canonical(reservation));
    }
    let mcp = Reply::decode(wire, &request).mcp();
    assert_eq!(mcp.is_error, Some(true));
    assert!(serde_json::to_vec(&mcp).unwrap().len() < crate::daemon::protocol::MAX_MESSAGE_SIZE);
    let f = Fixture::from_open_engine(root, reopened.engine);
    readers::completion(&f).complete().await.unwrap();
    let a = adapter(&f, provider.clone(), clock.clone(), Default::default());
    let baseline = bytes(&d.path);
    let time = clock.0.load(Ordering::SeqCst);
    let Reply::Owned(Ok(replay)) = a.store_batch_api1(request).await else {
        panic!("replay failed")
    };
    assert_eq!(replay.outcome.batch_operation_id, reservation.operation_id);
    for (item, retained) in replay.outcome.items.iter().zip(&progress.items) {
        assert_eq!(item.operation_id, Some(retained.operation_id));
    }
    if nth > 1 && !successful_prefix {
        assert_eq!(bytes(&d.path), baseline);
        assert_eq!(clock.0.load(Ordering::SeqCst), time);
        assert_eq!(replay.outcome.bookkeeping_receipts, bookkeeping);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_postcommit_first_rejection() {
    postcommit(2, false, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_postcommit_later_rejection() {
    postcommit(3, false, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_postcommit_rejection_keeps_success_and_pending() {
    postcommit(3, true, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_postcommit_reservation_local() {
    postcommit(1, false, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_postcommit_reservation_real_remote() {
    postcommit(1, true, false).await;
}
