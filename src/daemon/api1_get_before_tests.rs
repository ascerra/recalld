// Included inside client::tests. The old-peer fault is a real socket boundary;
// its get operation forwards to the actual isolated daemon/storage adapter.
async fn api1_point_old_peer_lost_reply_probe(access: bool) {
    use crate::mcp::bridge::StorageEngine as _;
    use crate::test_support::{Fixture, MemorySpec};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let f = Fixture::new();
    let id = f
        .insert_memory_with(MemorySpec {
            summary: "lost API1 get reply",
            embedding: Some(&[1., 0., 0., 0.]),
            ..MemorySpec::new(crate::model::NamespaceId::new(1))
        })
        .await;
    f.completion().rebuild_all().await.unwrap();
    let before = f.record(id).unwrap().access_history.len();
    let root = tempfile::tempdir().unwrap();
    let (backend, backend_task) = crate::daemon::server::native_lifecycle_test_remote(
        Arc::new(
            f.storage_adapter()
                .with_clock(Arc::new(crate::time::ManualClock::new(123_456))),
        ),
        &root.path().join("owner.sock"),
    )
    .await;
    let path = root.path().join("old-peer.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let peer = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            loop {
                let Ok(request) = read_framed_message(&mut stream, MAX_MESSAGE_SIZE).await else {
                    break;
                };
                let value = if request.method == "ping" {
                    // Valid historical handshake, no API1-get capability.
                    json!({"protocolVersion":2,"maxMessageSize":MAX_MESSAGE_SIZE})
                } else {
                    assert_eq!(
                        request.method,
                        if access {
                            "get_memory"
                        } else {
                            "get_memory_read_only"
                        }
                    );
                    assert_eq!(request.params["id"], id.to_string());
                    assert_eq!(request.params["recordAccess"], access);
                    let value = backend.get_memory(id, access, None).await.unwrap();
                    let n = seen.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        // The real owner completed; deliberately lose only its reply.
                        break;
                    }
                    serde_json::to_value(value).unwrap()
                };
                write_framed_message(
                    &mut stream,
                    &DaemonResponse::success(request.id, value),
                    MAX_MESSAGE_SIZE,
                )
                .await
                .unwrap();
            }
        }
    });
    let client = Arc::new(DaemonClient::connect(&path).await.unwrap());
    client.ping().await.unwrap();
    let remote = crate::daemon::bridge_adapters::RemoteStorageAdapter::new(client);
    let result = tokio::time::timeout(Duration::from_secs(8), remote.get_memory(id, access, None))
        .await
        .unwrap();
    let frames = calls.load(Ordering::SeqCst);
    let events = f.record(id).unwrap().access_history.len() - before;
    drop(remote);
    peer.abort();
    let _ = peer.await;
    tokio::time::timeout(Duration::from_secs(3), backend_task)
        .await
        .unwrap()
        .unwrap();
    eprintln!(
        "API1 old peer access={access}: get_frames={frames} actual_events={events} result={result:?}"
    );
    if access {
        assert_eq!(events, 1, "lost observing reply repeated a real exposure");
        assert_eq!(frames, 1);
        assert!(
            result.is_err(),
            "lost reply must not become a fresh success"
        );
    } else {
        assert_eq!(events, 0);
        assert_eq!(frames, 2, "existing read-only retry compatibility");
        assert!(result.unwrap().is_some());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_point_before_old_peer_lost_observing_reply_never_repeats() {
    api1_point_old_peer_lost_reply_probe(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api1_point_old_peer_lost_read_only_reply_preserves_no_write() {
    api1_point_old_peer_lost_reply_probe(false).await;
}
