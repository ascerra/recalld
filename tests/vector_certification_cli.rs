//! Real offline command and actual HTTP provider adapter, synthetic loopback only.
use recalld::storage::control::{MutationContext, MutationScope};
use recalld::storage::{
    logical_export, offline_snapshot::OfflineSnapshot, vector_certification::ReembedVectors,
};
use std::io::{Read, Write};
use std::process::Command;

#[test]
fn cli_vector_certification_executes_document_recipe_then_replays_without_provider_configuration() {
    run_operator(false);
}

#[test]
fn cli_vector_certification_preserves_pending_parent_and_never_claims_batch_completion() {
    run_operator(true);
}

fn run_operator(pending: bool) {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("before.jsonl");
    std::fs::write(
        &input,
        include_bytes!(
            "../src/storage/metadata/native_records/fixtures/document-recipe-legacy.jsonl"
        ),
    )
    .unwrap();
    let mut source = root.path().join("source");
    logical_export::import_store(&input, &source).unwrap();
    let snapshot = OfflineSnapshot::open(&source).unwrap();
    let binding = snapshot.control().generation_ref();
    let ns = snapshot
        .namespaces()
        .iter()
        .find(|ns| ns.name == "native")
        .unwrap();
    let recalld::model::schema::EmbeddingBinding::Bound {
        descriptor: profile,
        profile_id,
        ..
    } = &ns.schema.as_ref().unwrap().embedding_binding
    else {
        panic!()
    };
    let mut request = ReembedVectors {
        context: MutationContext {
            store_id: Some(binding.store_id),
            expected_generation: Some(binding.generation),
            scope: MutationScope::Namespace(ns.id),
            contract_version: 1,
            idempotency_key: Some("cli-cutover".into()),
        },
        source: binding,
        namespace_id: ns.id,
        namespace_name: ns.name.clone(),
        profile_id: profile_id.clone(),
    };
    let model = profile.model_id.clone();
    let expected_calls = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == ns.id)
        .unwrap()
        .id;
    let mut expected = vec![];
    snapshot
        .for_each_record(|row| {
            if row.record.namespace_id == expected_calls.get() && row.embedding.is_some() {
                let labels = &row.record.schema.as_ref().unwrap().labels;
                // Independent document-v1 protocol expectation from the actual
                // retained fixture, including legacy labels and exact full text.
                let mut tags: Vec<_> = row.record.tags.iter().map(ToString::to_string).collect();
                for (values, prefix) in [
                    (&labels.entities, "entity/"),
                    (&labels.topics, "topic/"),
                    (&labels.emotions, "emotion/"),
                ] {
                    for value in values {
                        let value = format!("{prefix}{}", value.to_lowercase());
                        if !tags.contains(&value) {
                            tags.push(value);
                        }
                    }
                }
                let mut document = row.record.summary;
                if let Some(text) = row.full_text {
                    document.push_str("\n\n");
                    document.push_str(&text);
                }
                if !tags.is_empty() {
                    document.push(' ');
                    document.push_str(&tags.join(" "));
                }
                expected.push(document);
            }
            Ok(())
        })
        .unwrap();
    drop(snapshot);
    if pending {
        let extracted = root.path().join("genuine-pending");
        zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
            "../src/storage/vector_certification/fixtures/pending-before-certification.zip"
        )))
        .unwrap()
        .extract(&extracted)
        .unwrap();
        source = extracted.join("store");
        request =
            serde_json::from_slice(&std::fs::read(extracted.join("remedy.json")).unwrap()).unwrap();
    }
    let original = std::fs::read(source.join("meta.db")).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let count = expected.len();
    let server = std::thread::spawn(move || {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut observed = vec![];
        while observed.len() < count {
            let (mut connection, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < until,
                        "missing operator provider requests"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            connection.set_nonblocking(false).unwrap();
            connection
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let header_end = loop {
                let mut one = [0];
                connection.read_exact(&mut one).unwrap();
                request.push(one[0]);
                assert!(request.len() <= 65536);
                if request.ends_with(b"\r\n\r\n") {
                    break request.len();
                }
            };
            let header = std::str::from_utf8(&request).unwrap();
            assert!(header.starts_with("POST /api/embed HTTP/1.1"));
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse().unwrap())
                })
                .unwrap();
            assert!(length <= 65536);
            request.resize(header_end + length, 0);
            connection.read_exact(&mut request[header_end..]).unwrap();
            let body: serde_json::Value = serde_json::from_slice(&request[header_end..]).unwrap();
            assert_eq!(body["model"], model);
            assert_eq!(body["input"].as_array().unwrap().len(), 1);
            observed.push(body["input"][0].as_str().unwrap().to_string());
            let response = r#"{"embeddings":[[0.0,1.0,0.0,0.0]]}"#;
            write!(connection, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response.len(), response).unwrap();
        }
        observed
    });
    let request_path = root.path().join("request.json");
    std::fs::write(&request_path, serde_json::to_vec(&request).unwrap()).unwrap();
    let config_path = root.path().join("provider.json");
    std::fs::write(&config_path, serde_json::to_vec(&serde_json::json!({"configuration":{"provider":"Ollama","dimensions":4,"model":"configured-model","base_url":format!("http://{address}"),"cache_embeddings":false},"documentPrefix":"","queryPrefix":""})).unwrap()).unwrap();
    let home = root.path().join("empty-home");
    std::fs::create_dir(&home).unwrap();
    let target = root.path().join("certified");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_recalld-cli"))
            .env_clear()
            .env("HOME", &home)
            .env("TMPDIR", root.path())
            .args(["--server", "not-an-http-url", "reembed-vectors", "--source"])
            .arg(&source)
            .arg("--destination")
            .arg(&target)
            .arg("--request")
            .arg(&request_path)
            .arg("--provider")
            .arg(&config_path)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(server.join().unwrap(), expected);
    let first: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(first["readiness"], "reopen_required");
    assert_eq!(std::fs::read(source.join("meta.db")).unwrap(), original);
    if pending {
        assert!(
            OfflineSnapshot::open(&target)
                .err()
                .unwrap()
                .to_string()
                .contains("unfinished batch")
        );
        // Inspect disposable copies, preserving every original manifest byte.
        use redb::ReadableTable;
        let batches = |source: &std::path::Path, copy: &std::path::Path| {
            std::fs::copy(source.join("meta.db"), copy).unwrap();
            let db = redb::Database::open(copy).unwrap();
            let tx = db.begin_read().unwrap();
            let rows = tx
                .open_table(redb::TableDefinition::<&[u8], &[u8]>::new(
                    "mutation_batches",
                ))
                .unwrap();
            rows.iter()
                .unwrap()
                .map(|row| {
                    let (key, value) = row.unwrap();
                    (key.value().to_vec(), value.value().to_vec())
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let before = batches(&source, &root.path().join("source-inspection.db"));
        assert!(!before.is_empty());
        assert_eq!(
            before,
            batches(&target, &root.path().join("target-inspection.db"))
        );
    } else {
        OfflineSnapshot::open(&target).unwrap();
    }
    // Retry does not read the fresh provider file or need the old source path.
    std::fs::remove_file(&config_path).unwrap();
    std::fs::rename(&source, root.path().join("old-source-moved")).unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let replay: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        replay["receipt"]["operationId"],
        first["receipt"]["operationId"]
    );
    assert_eq!(replay["receipt"]["replayed"], true);
}
