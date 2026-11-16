use super::*;
use crate::storage::vector_certification::{reembed_fresh, tests::fixture};

fn lines(path: &Path) -> Vec<Line> {
    let mut reader = BufReader::new(std::fs::File::open(path).unwrap());
    let mut lines = vec![];
    while let Some(line) = read_line(&mut reader).unwrap() {
        lines.push(line);
    }
    lines
}
fn rewrite(path: &Path, mut lines: Vec<Line>) {
    let Line::Header(header) = &lines[0] else {
        panic!()
    };
    let mut previous = digest(&encoded(header).unwrap());
    let mut logical = LogicalHash::new(header.source).unwrap();
    let mut sequence = 0;
    for line in &mut lines[1..] {
        match line {
            Line::Chunk(chunk) => {
                sequence += 1;
                chunk.sequence = sequence;
                chunk.sha256 = chain(previous, sequence, &encoded(&chunk.item).unwrap());
                previous = chunk.sha256;
                logical.item(&chunk.item).unwrap();
            }
            Line::Footer {
                summary,
                chain_sha256,
            } => {
                summary.chunks = sequence;
                summary.records = logical.records;
                summary.logical_sha256 = logical.hash.clone().finalize().into();
                *chain_sha256 = previous;
            }
            _ => panic!(),
        }
    }
    let mut output = std::fs::File::create(path).unwrap();
    for line in lines {
        write_line(&mut output, &line).unwrap();
    }
}
fn names(path: &Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|p| p.unwrap().file_name())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn vector_certification_format2_layout_rejects_before_stage_and_format1_bytes_stay_identical()
{
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    // A genuine old format1 fixture still verifies its original chained bytes.
    let old = root.path().join("genuine-before.jsonl");
    let (header, _, _, _) = scan_stream(&old, |_, chunk, _| {
        if let Item::Memory(memory) = &chunk.item {
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Before<'a> {
                record: &'a Vec<u8>,
                full_text: &'a Option<String>,
                embedding_bits: &'a Option<Vec<u32>>,
            }
            assert!(memory.vector_placement.is_none());
            assert_eq!(
                encoded(memory).unwrap(),
                encoded(&Before {
                    record: &memory.record,
                    full_text: &memory.full_text,
                    embedding_bits: &memory.embedding_bits
                })
                .unwrap()
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(header.export_format_version, 1);
    let certified = root.path().join("certified");
    reembed_fresh(&source, &certified, &request, &provider)
        .await
        .unwrap();
    let export = root.path().join("export.jsonl");
    export_store(&certified, &export).unwrap();
    let (header, _, _, layout) = scan_stream(&export, |_, _, _| Ok(())).unwrap();
    assert_eq!(header.export_format_version, 2);
    assert!(layout.capacity() > 0);
    for case in [
        "missing",
        "duplicate",
        "extreme",
        "rounded-capacity",
        "extraneous",
        "dimension",
        "cardinality",
        "downgrade",
        "checksum",
        "null",
    ] {
        let mut items = lines(&export);
        if case == "extraneous" {
            for line in &mut items {
                if let Line::Chunk(Chunk {
                    item: Item::Namespace(namespace),
                    ..
                }) = line
                {
                    if namespace.id == request.namespace_id {
                        let schema = namespace.schema.as_mut().unwrap();
                        schema.vector_certification = None;
                        schema.required_features.clear();
                    }
                }
            }
        }
        if case == "downgrade" {
            let Line::Header(header) = &mut items[0] else {
                panic!()
            };
            header.export_format_version = 1;
        } else {
            let memories: Vec<_> = items
                .iter_mut()
                .filter_map(|line| match line {
                    Line::Chunk(Chunk {
                        item: Item::Memory(memory),
                        ..
                    }) => Some(memory),
                    _ => None,
                })
                .collect();
            let mut memories = memories.into_iter();
            let first = memories.next().unwrap();
            match case {
                "missing" => first.vector_placement = None,
                "duplicate" => memories.next().unwrap().vector_placement = first.vector_placement,
                "extreme" => first.vector_placement = Some(u32::MAX),
                "rounded-capacity" => {
                    first.vector_placement = Some(
                        ((crate::storage::vector_certification::MAX_BYTES - 64) / 16 - 1) as u32,
                    )
                }
                "extraneous" => {}
                "dimension" => {
                    let bits = first.embedding_bits.as_mut().unwrap();
                    bits.pop();
                    let mut record = DiskRecord::from_bytes(&first.record).unwrap();
                    let mut hash = Sha256::new();
                    for value in bits {
                        hash.update(value.to_le_bytes());
                    }
                    record
                        .schema
                        .as_mut()
                        .unwrap()
                        .vector
                        .as_mut()
                        .unwrap()
                        .sha256 = hash.finalize().into();
                    first.record = record.encode().unwrap();
                }
                "cardinality" => first.embedding_bits = None,
                "checksum" | "null" => {}
                _ => panic!(),
            }
        }
        let input = root.path().join(format!("{case}.jsonl"));
        rewrite(&input, items);
        if case == "checksum" {
            let mut raw = lines(&input);
            let Line::Chunk(chunk) = &mut raw[1] else {
                panic!()
            };
            chunk.sha256 = [0; 32];
            let mut file = std::fs::File::create(&input).unwrap();
            for line in raw {
                write_line(&mut file, &line).unwrap();
            }
        }
        if case == "null" {
            // Use the actual serialized field wherever Item's tag/content lives.
            let text = std::fs::read_to_string(&input).unwrap();
            let start = text.find("\"vectorPlacement\":").unwrap() + "\"vectorPlacement\":".len();
            let end = start + text[start..].find(',').unwrap();
            let mut changed = text;
            changed.replace_range(start..end, "null");
            std::fs::write(&input, changed).unwrap();
        }
        let before = std::fs::read(&input).unwrap();
        let directory_before = names(root.path());
        let result = import_store(&input, &root.path().join(format!("rejected-{case}")));
        assert!(result.is_err(), "accepted {case}");
        assert_eq!(std::fs::read(&input).unwrap(), before);
        assert_eq!(
            names(root.path()),
            directory_before,
            "staged before rejecting {case}"
        );
    }
}

#[tokio::test]
async fn vector_certification_format2_reservation_reclamation_crash_retry_preserves_live_slots() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let certified = root.path().join("certified");
    reembed_fresh(&source, &certified, &request, &provider)
        .await
        .unwrap();
    let export = root.path().join("certified.jsonl");
    let expected = export_store(&certified, &export).unwrap();
    let bytes = std::fs::read(&export).unwrap();
    let placements: std::collections::HashMap<_, _> = lines(&export)
        .into_iter()
        .filter_map(|line| match line {
            Line::Chunk(Chunk {
                item: Item::Memory(memory),
                ..
            }) => memory
                .vector_placement
                .map(|slot| (slot, memory.embedding_bits.unwrap())),
            _ => None,
        })
        .collect();
    for point in [
        "vector_reserved",
        "payload_synced",
        "vector_reclaimed",
        "selected",
        "published",
    ] {
        let target = root.path().join(point);
        // Repeated exits advance only unfinished reservation/reclamation work.
        let repeats = if matches!(point, "vector_reserved" | "vector_reclaimed") {
            2
        } else {
            1
        };
        for _ in 0..repeats {
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "storage::logical_export::tests::import_interruption_child",
                    "--nocapture",
                ])
                .env("RECALLD_IMPORT_TEST_INPUT", &export)
                .env("RECALLD_IMPORT_TEST_TARGET", &target)
                .env("RECALLD_IMPORT_TEST_STOP", point)
                .output()
                .unwrap();
            assert_eq!(
                child.status.code(),
                Some(76),
                "{point}: {}",
                String::from_utf8_lossy(&child.stderr)
            );
            for entry in std::fs::read_dir(root.path()).unwrap() {
                let stage = entry.unwrap().path();
                if stage.join(".recalld-importing").exists() {
                    let before = std::fs::read(stage.join("meta.db")).unwrap();
                    assert!(OfflineSnapshot::open(&stage).is_err());
                    assert!(crate::storage::RedbStorageEngine::open_native(&stage).is_err());
                    assert_eq!(std::fs::read(stage.join("meta.db")).unwrap(), before);
                }
            }
        }
        assert_eq!(import_store(&export, &target).unwrap(), expected);
        assert_eq!(import_store(&export, &target).unwrap(), expected);
        let snapshot = OfflineSnapshot::open(&target).unwrap();
        let mut actual = std::collections::HashMap::new();
        snapshot
            .for_each_record(|row| {
                if let Some(vector) = row.embedding {
                    assert!(
                        actual
                            .insert(
                                row.record.vector_slot,
                                vector.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                            )
                            .is_none()
                    );
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(actual, placements);
        drop(snapshot);
        // All placeholders are reclaimed, every referenced slot stays occupied;
        // subsequent allocation consumes one free slot without replacing a peer.
        let mut vectors =
            crate::storage::VectorStore::open(&target.join(&request.namespace_name), 4).unwrap();
        for slot in 0..vectors.slot_count() {
            assert_eq!(vectors.is_free(slot), !placements.contains_key(&slot));
        }
        let next = vectors.insert_vector(&[1., 0., 0., 0.]).unwrap();
        assert!(!placements.contains_key(&next));
        for (slot, bits) in &placements {
            assert_eq!(
                vectors
                    .get_vector(*slot)
                    .unwrap()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                *bits
            );
        }
        assert_eq!(std::fs::read(&export).unwrap(), bytes);
    }
}

#[tokio::test]
async fn vector_certification_location_hint_cannot_grant_authority_or_publish_invalid_stage() {
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let certified = root.path().join("certified");
    reembed_fresh(&source, &certified, &request, &provider)
        .await
        .unwrap();
    let export = root.path().join("wrong-location.jsonl");
    export_store(&certified, &export).unwrap();
    let mut items = lines(&export);
    let memory = items
        .iter_mut()
        .find_map(|line| match line {
            Line::Chunk(Chunk {
                item: Item::Memory(memory),
                ..
            }) => Some(memory),
            _ => None,
        })
        .unwrap();
    memory.vector_placement = Some(20); // bounded, unique, correct f32 checksum, but no typed proof for this location
    rewrite(&export, items);
    scan_stream(&export, |_, _, _| Ok(())).unwrap();
    let bytes = std::fs::read(&export).unwrap();
    let target = root.path().join("must-not-publish");
    for _ in 0..2 {
        assert!(import_store(&export, &target).is_err());
        assert!(!target.exists());
        let stage = std::fs::read_dir(root.path())
            .unwrap()
            .map(|p| p.unwrap().path())
            .find(|p| p.join(".recalld-importing").exists())
            .unwrap();
        assert!(OfflineSnapshot::open(&stage).is_err());
        assert!(crate::storage::RedbStorageEngine::open_native(&stage).is_err());
        assert_eq!(std::fs::read(&export).unwrap(), bytes);
    }
}

#[tokio::test]
async fn vector_certification_genuine_old_parser_header_rejection_and_downgraded_feature_are_distinct()
 {
    let Some(before) = std::env::var_os("RECALLD_CERTIFICATION_BEFORE_BINARY") else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let (source, request, provider) = fixture(root.path());
    let certified = root.path().join("certified");
    reembed_fresh(&source, &certified, &request, &provider)
        .await
        .unwrap();
    let export = root.path().join("format2.jsonl");
    export_store(&certified, &export).unwrap();
    for (case, expected) in [
        ("header2", "unsupported export version"),
        ("downgraded-shape", "malformed or unsupported stream"),
        (
            "downgraded-feature",
            "unsupported namespace schema/features",
        ),
    ] {
        let mut content = lines(&export);
        if case != "header2" {
            let Line::Header(header) = &mut content[0] else {
                panic!()
            };
            header.export_format_version = 1;
            if case == "downgraded-feature" {
                // Deliberately remove the unknown optional field to isolate
                // the genuine old required-feature fence before memory hints.
                for line in &mut content {
                    if let Line::Chunk(Chunk {
                        item: Item::Namespace(namespace),
                        ..
                    }) = line
                    {
                        namespace.schema.as_mut().unwrap().vector_certification = None;
                    }
                }
            }
        }
        let input = root.path().join(format!("{case}.jsonl"));
        rewrite(&input, content);
        let bytes = std::fs::read(&input).unwrap();
        let before_names = names(root.path());
        let child = std::process::Command::new(&before)
            .args([
                "--exact",
                "storage::logical_export::tests::import_interruption_child",
                "--nocapture",
            ])
            .env("RECALLD_IMPORT_TEST_INPUT", &input)
            .env(
                "RECALLD_IMPORT_TEST_TARGET",
                root.path().join(format!("rejected-{case}")),
            )
            .output()
            .unwrap();
        assert_eq!(child.status.code(), Some(101));
        let stderr = String::from_utf8_lossy(&child.stderr);
        assert!(stderr.contains(expected), "{case}: {stderr}");
        assert_eq!(std::fs::read(&input).unwrap(), bytes);
        assert_eq!(names(root.path()), before_names);
    }
}

#[tokio::test]
async fn vector_certification_mixed_namespaces_preserve_uncertified_payload_bits_without_placements()
 {
    use crate::embedding::EmbeddingProvider as _;
    use crate::storage::{
        StorageEngine as _,
        schema_migration::{
            MigrationOptions, ProfileAttestationInput, inspect_schema, migrate_schema,
        },
    };
    let root = tempfile::tempdir().unwrap();
    let (_, _, provider) = fixture(root.path());
    let legacy = root.path().join("mixed-legacy");
    let mut engine = crate::storage::RedbStorageEngine::open(&legacy).unwrap();
    let mut config = NamespaceConfig::default_namespace(0);
    config.embedding_dim = 4;
    config.name = "certified".into();
    let certified_ns = engine.create_namespace(&config).unwrap();
    config.name = "uncertified".into();
    let uncertified_ns = engine.create_namespace(&config).unwrap();
    let mut uncertified_ids = vec![];
    for (ns, text) in [
        (certified_ns, "certified input"),
        (uncertified_ns, "unknown original producer one"),
        (uncertified_ns, "unknown original producer two"),
    ] {
        let receipt = engine
            .insert_memory_with_receipt(
                &crate::storage::control::MutationContext::unkeyed(ns),
                &crate::storage::insertion::InsertMemory {
                    id: None,
                    summary: text.into(),
                    full_text: Some(text.into()),
                    tags: vec![],
                    initial_stability: None,
                    created_at: Some(1000),
                    embedding: Some(vec![1., -0., 0., 0.]),
                    parent_id: None,
                    supersedes: None,
                },
                &[],
                1000,
            )
            .unwrap();
        if ns == uncertified_ns {
            uncertified_ids.push(crate::storage::insertion::inserted_id(&receipt).unwrap());
        }
    }
    drop(engine);
    let native = root.path().join("mixed-native");
    let profile = provider.profile().unwrap();
    migrate_schema(
        &legacy,
        &native,
        &MigrationOptions {
            migration_now: 2000,
            expected_source: inspect_schema(&legacy).unwrap().control.generation_ref(),
            attestations: vec![ProfileAttestationInput {
                namespace_id: certified_ns,
                namespace_name: "certified".into(),
                descriptor: profile.clone(),
            }],
        },
    )
    .unwrap();
    let snapshot = OfflineSnapshot::open(&native).unwrap();
    let source = snapshot.control().generation_ref();
    let mut before = std::collections::BTreeMap::new();
    for id in &uncertified_ids {
        let row = snapshot.record(*id).unwrap().unwrap();
        before.insert(
            *id,
            (
                row.record.clone(),
                row.embedding
                    .unwrap()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
            ),
        );
    }
    drop(snapshot);
    let request = crate::storage::vector_certification::ReembedVectors {
        source,
        namespace_id: certified_ns,
        namespace_name: "certified".into(),
        profile_id: profile.profile_id().unwrap(),
        context: crate::storage::control::MutationContext {
            store_id: Some(source.store_id),
            scope: crate::storage::control::MutationScope::Namespace(certified_ns),
            contract_version: 1,
            idempotency_key: Some("mixed-reembed".into()),
            expected_generation: Some(source.generation),
        },
    };
    let destination = root.path().join("mixed-certified");
    reembed_fresh(&native, &destination, &request, &provider)
        .await
        .unwrap();
    let export = root.path().join("mixed.jsonl");
    let expected = export_store(&destination, &export).unwrap();
    let mut hints = 0;
    scan_stream(&export, |_, chunk, _| {
        if let Item::Memory(memory) = &chunk.item {
            let row = DiskRecord::from_bytes(&memory.record)?;
            assert_eq!(
                memory.vector_placement.is_some(),
                row.namespace_id == certified_ns.get()
            );
            hints += usize::from(memory.vector_placement.is_some());
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(hints, 1);
    let imported = root.path().join("mixed-imported");
    assert_eq!(import_store(&export, &imported).unwrap(), expected);
    let snapshot = OfflineSnapshot::open(&imported).unwrap();
    for (id, (record, bits)) in before {
        let row = snapshot.record(id).unwrap().unwrap();
        assert_eq!(
            row.embedding
                .unwrap()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            bits
        );
        assert_eq!(row.record.schema, record.schema);
        assert_eq!(row.record.vector_slot, record.vector_slot);
        assert_eq!(row.record.summary, record.summary);
    }
    let unknown = snapshot
        .namespaces()
        .iter()
        .find(|n| n.id == uncertified_ns)
        .unwrap()
        .schema
        .as_ref()
        .unwrap();
    assert!(unknown.vector_certification.is_none());
    assert!(matches!(
        unknown.embedding_binding,
        crate::model::schema::EmbeddingBinding::UnknownLegacy
    ));
}
