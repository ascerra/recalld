//! Exercise the public MCP startup path against isolated full-daemon fixtures.
use std::process::Stdio;
use std::time::Duration;

use recalld::daemon::protocol::{
    DaemonResponse, DaemonRpcError, ERR_UNAVAILABLE, LEGACY_MAX_MESSAGE_SIZE, read_framed_message,
    write_framed_message,
};
use tokio::net::UnixListener;
use tokio::process::Command;

#[tokio::test]
async fn refused_handshake_does_not_start_another_daemon_or_open_storage() {
    for framed_refusal in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("daemon.sock");
        let data = dir.path().join("data");
        let config = dir.path().join("config.toml");
        std::fs::write(&config, "[embedding]\nprovider = \"passthrough\"\n").unwrap();
        let listener = UnixListener::bind(&socket).unwrap();
        let refusal = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                if framed_refusal {
                    // Synchronize with the ping to exercise receipt of the reason.
                    // Legacy daemons (and a close racing the ping) just give EOF.
                    read_framed_message(&mut stream, LEGACY_MAX_MESSAGE_SIZE)
                        .await
                        .unwrap();
                    let response = DaemonResponse::error(
                        0,
                        DaemonRpcError {
                            code: ERR_UNAVAILABLE,
                            message: "daemon is at its connection limit (32)".into(),
                        },
                    );
                    write_framed_message(&mut stream, &response, LEGACY_MAX_MESSAGE_SIZE)
                        .await
                        .unwrap();
                }
            }
        });
        let output = tokio::time::timeout(
            Duration::from_secs(5),
            Command::new(env!("CARGO_BIN_EXE_recalld"))
                .env_clear()
                .env("HOME", dir.path())
                .env("RECALLD_DAEMON_SOCKET", &socket)
                .current_dir(dir.path())
                .arg("--config")
                .arg(config)
                .arg("--data-dir")
                .arg(&data)
                .arg("mcp")
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("MCP startup must report refusal promptly")
        .unwrap();
        refusal.abort();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{stderr}");
        let expected = if framed_refusal {
            "daemon is at its connection limit (32)"
        } else {
            "may be at its connection limit"
        };
        assert!(stderr.contains(expected), "{stderr}");
        assert!(!dir.path().join("daemon.log").exists(), "spawned a daemon");
        assert!(!data.exists(), "opened direct storage");
    }
}
