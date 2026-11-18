//! Explicit offline legacy repair. Never activates a store or edits the source.
//! The operator approves exact IDs and a fresh snapshot fingerprint first.
use super::metadata::{META_TABLE, NAMESPACE_TABLE};
use super::{StorageError, TextRef, TextStore, VectorStore};
use crate::model::{DecayPhase, DiskRecord, MemoryId, NamespaceConfig};
use redb::{MultimapTableDefinition, ReadableTable};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

fn invalid(s: &str) -> StorageError {
    StorageError::InvalidMutation(s.into())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairPlan {
    pub version: u32,
    pub inventory: BTreeMap<String, String>,
    pub drop_ids: Vec<MemoryId>,
    pub repair_ids: Vec<MemoryId>,
    pub records: usize,
}
pub struct RepairDocument {
    pub id: MemoryId,
    pub namespace_id: u32,
    pub dimension: u32,
    pub document: String,
}

// Bound the entire snapshot and reject links/special files, including in derived data.
fn inventory(root: &Path) -> Result<BTreeMap<String, String>, StorageError> {
    fn walk(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<String, String>,
        bytes: &mut u64,
    ) -> Result<(), StorageError> {
        if !fs::symlink_metadata(dir)?.is_dir() {
            return Err(invalid("snapshot directory is not a real directory"));
        }
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let meta = fs::symlink_metadata(&path)?;
            if meta.is_dir() {
                walk(root, &path, out, bytes)?;
            } else if meta.is_file() {
                *bytes = bytes
                    .checked_add(meta.len())
                    .ok_or_else(|| invalid("snapshot size overflow"))?;
                if *bytes > 2 * 1024 * 1024 * 1024 || out.len() >= 10000 {
                    return Err(invalid("repair snapshot exceeds bounds"));
                }
                let name = path
                    .strip_prefix(root)
                    .map_err(|_| invalid("snapshot path"))?
                    .to_str()
                    .ok_or_else(|| invalid("non UTF8 snapshot path"))?
                    .to_owned();
                let mut f = fs::File::open(&path)?;
                let mut hash = Sha256::new();
                let mut buf = [0; 65536];
                loop {
                    let n = f.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    hash.update(&buf[..n]);
                }
                out.insert(
                    name,
                    hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
                );
            } else {
                return Err(invalid("snapshot contains symlink or special file"));
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out, &mut 0)?;
    Ok(out)
}
fn copy_snapshot(
    source: &Path,
    target: &Path,
    expected: &BTreeMap<String, String>,
) -> Result<(), StorageError> {
    fs::create_dir(target)?;
    for name in expected.keys() {
        let dest = target.join(name);
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(source.join(name), &dest)?;
        fs::File::open(dest)?.sync_all()?;
    }
    for (side, actual) in [
        ("source", inventory(source)?),
        ("destination", inventory(target)?),
    ] {
        if actual != *expected {
            let changed: BTreeSet<_> = actual
                .keys()
                .chain(expected.keys())
                .filter(|key| actual.get(*key) != expected.get(*key))
                .cloned()
                .collect();
            return Err(invalid(&format!(
                "snapshot changed during {} copy: {side} relative files {changed:?}",
                target
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            )));
        }
    }
    super::fsync::fsync_dir(target)?;
    Ok(())
}
struct Snapshot {
    _temp: tempfile::TempDir,
    _db: redb::Database,
    rows: Vec<DiskRecord>,
    namespaces: BTreeMap<u32, NamespaceConfig>,
    vectors: BTreeMap<u32, VectorStore>,
}
fn snapshot(source: &Path, expected: &BTreeMap<String, String>) -> Result<Snapshot, StorageError> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("copy");
    copy_snapshot(source, &path, expected)?;
    let mut builder = redb::Builder::new();
    builder.set_repair_callback(|s| s.abort());
    let db = builder.open(path.join("meta.db"))?;
    let read = db.begin_read()?;
    let mut namespaces = BTreeMap::new();
    let mut vectors = BTreeMap::new();
    for row in read.open_table(NAMESPACE_TABLE)?.iter()? {
        let (key, value) = row?;
        let ns = NamespaceConfig::decode(value.value())?;
        if ns.schema.is_some() || ns.id.get() != key.value() {
            return Err(invalid("repair requires legacy namespace"));
        }
        crate::model::validation::validate_namespace_name(&ns.name)
            .map_err(|_| invalid("invalid namespace name"))?;
        super::validation::require_file(&path.join(&ns.name).join("vectors.dat"))?;
        vectors.insert(
            ns.id.get(),
            VectorStore::open(&path.join(&ns.name), ns.embedding_dim as usize)?,
        );
        namespaces.insert(ns.id.get(), ns);
    }
    let mut rows = Vec::new();
    for row in read.open_table(META_TABLE)?.iter()? {
        let (key, value) = row?;
        if rows.len() >= 100000 || value.value().len() > 16 * 1024 * 1024 {
            return Err(invalid("record bound exceeded"));
        }
        let record =
            DiskRecord::from_bytes(value.value()).map_err(|_| invalid("invalid legacy record"))?;
        if record.version != 1
            || record.id.as_slice() != key.value()
            || !vectors.contains_key(&record.namespace_id)
        {
            return Err(invalid("invalid legacy identity/schema"));
        }
        rows.push(record);
    }
    drop(read);
    Ok(Snapshot {
        _temp: temp,
        _db: db,
        rows,
        namespaces,
        vectors,
    })
}
fn classify(s: &Snapshot, inventory: BTreeMap<String, String>) -> Result<RepairPlan, StorageError> {
    let mut owners = BTreeMap::<(u32, u32), usize>::new();
    for r in &s.rows {
        if r.phase != DecayPhase::Tombstone {
            *owners.entry((r.namespace_id, r.vector_slot)).or_default() += 1;
        }
    }
    let mut result = RepairPlan {
        version: 1,
        inventory,
        drop_ids: vec![],
        repair_ids: vec![],
        records: s.rows.len(),
    };
    for r in &s.rows {
        if r.phase == DecayPhase::Tombstone {
            continue;
        }
        let v = &s.vectors[&r.namespace_id];
        if owners[&(r.namespace_id, r.vector_slot)] > 1
            || v.is_free(r.vector_slot)
            || v.get_vector(r.vector_slot).is_none()
        {
            let id = MemoryId::from_bytes(r.id);
            if r.summary.trim().is_empty() && r.text_length == 0 && r.text_offset == 0 {
                result.drop_ids.push(id)
            } else {
                result.repair_ids.push(id)
            }
        }
    }
    Ok(result)
}
pub fn plan(source: &Path) -> Result<RepairPlan, StorageError> {
    let files = inventory(source)?;
    let s = snapshot(source, &files)?;
    classify(&s, files)
}
fn checked(source: &Path, approved: &RepairPlan) -> Result<Snapshot, StorageError> {
    let files = inventory(source)?;
    if files != approved.inventory {
        return Err(invalid("approved snapshot fingerprint mismatch"));
    }
    let s = snapshot(source, &files)?;
    if classify(&s, files)? != *approved {
        return Err(invalid("approved repair manifest mismatch"));
    }
    Ok(s)
}
pub fn documents(
    source: &Path,
    approved: &RepairPlan,
) -> Result<Vec<RepairDocument>, StorageError> {
    let s = checked(source, approved)?;
    let text = TextStore::open_read_only(&s._temp.path().join("copy/fulltext.dat"))?;
    let mut out = vec![];
    for r in &s.rows {
        let id = MemoryId::from_bytes(r.id);
        if !approved.repair_ids.contains(&id) {
            continue;
        }
        if (r.text_length == 0) != (r.text_offset == 0) {
            return Err(invalid("inconsistent retained text reference"));
        }
        let full_text = if r.text_length == 0 {
            None
        } else {
            Some(text.read(TextRef {
                file_offset: r.text_offset,
                length: r.text_length,
            })?)
        };
        if r.summary.trim().is_empty() && full_text.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(invalid("repair record has no usable retained content"));
        }
        let input = super::native_records::NativeInsert {
            memory: super::insertion::InsertMemory {
                id: Some(id),
                summary: r.summary.clone(),
                full_text,
                tags: r.tags.clone(),
                initial_stability: None,
                created_at: None,
                embedding: None,
                parent_id: None,
                supersedes: None,
            },
            observed_at: None,
            entities: vec![],
            topics: vec![],
            emotions: vec![],
            embedding_profile: None,
        };
        out.push(RepairDocument {
            id,
            namespace_id: r.namespace_id,
            dimension: s.namespaces[&r.namespace_id].embedding_dim,
            document: input.document_v1(),
        });
    }
    Ok(out)
}
/// Writes `destination/backup` and `destination/candidate`; never activates either.
/// Errors leave the workspace for inspection, with no success marker.
pub fn apply(
    source: &Path,
    destination: &Path,
    approved: &RepairPlan,
    replacements: &BTreeMap<MemoryId, Vec<f32>>,
) -> Result<RepairPlan, StorageError> {
    let source = source.canonicalize()?;
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("destination parent missing"))?
        .canonicalize()?;
    let destination = parent.join(
        destination
            .file_name()
            .ok_or_else(|| invalid("destination name missing"))?,
    );
    if destination.starts_with(&source) || source.starts_with(&destination) || destination.exists()
    {
        return Err(invalid("repair requires separate fresh destination"));
    }
    let _owner = super::ownership::StoreOwnership::acquire(&destination)?;
    let s = checked(&source, approved)?;
    if replacements.keys().copied().collect::<Vec<_>>() != approved.repair_ids {
        return Err(invalid("replacement IDs do not match approved repair IDs"));
    }
    // Validate all retained content and generated vectors before creating output.
    let _ = documents(&source, approved)?;
    for r in &s.rows {
        if let Some(v) = replacements.get(&MemoryId::from_bytes(r.id)) {
            if v.len() != s.namespaces[&r.namespace_id].embedding_dim as usize {
                return Err(invalid("replacement vector dimension mismatch"));
            }
            super::canonical_schema::validate_bound_vector(v)?;
        }
    }
    fs::create_dir(&destination)?;
    let backup = destination.join("backup");
    copy_snapshot(&source, &backup, &approved.inventory)?;
    super::fsync::fsync_dir(&destination)?;
    let candidate = destination.join("candidate");
    copy_snapshot(&backup, &candidate, &approved.inventory)?;
    let drops: BTreeSet<_> = approved.drop_ids.iter().copied().collect();
    let affected: BTreeSet<_> = s
        .rows
        .iter()
        .filter(|r| {
            drops.contains(&MemoryId::from_bytes(r.id))
                || replacements.contains_key(&MemoryId::from_bytes(r.id))
        })
        .map(|r| r.namespace_id)
        .collect();
    let mut rows = s.rows.clone();
    for nsid in affected {
        let ns = &s.namespaces[&nsid];
        let fresh = destination.join(format!("vectors-{nsid}"));
        let mut vectors = VectorStore::open(&fresh, ns.embedding_dim as usize)?;
        for r in &mut rows {
            let id = MemoryId::from_bytes(r.id);
            if r.namespace_id != nsid || r.phase == DecayPhase::Tombstone || drops.contains(&id) {
                continue;
            }
            let values = match replacements.get(&id) {
                Some(v) => v.as_slice(),
                None => s.vectors[&nsid]
                    .get_vector(r.vector_slot)
                    .ok_or_else(|| invalid("unaffected vector missing"))?,
            };
            r.vector_slot = vectors.insert_vector_no_remap(values)?;
        }
        vectors.finish_batch()?;
        vectors.sync()?;
        drop(vectors);
        fs::rename(
            fresh.join("vectors.dat"),
            candidate.join(&ns.name).join("vectors.dat"),
        )?;
        fs::remove_dir(fresh)?;
        super::fsync::fsync_dir(&candidate.join(&ns.name))?;
    }
    // Keep all surviving edge values byte-for-byte; remove both incident indexes.
    let edges = super::EdgeStore::open(&candidate.join("edges.db"))?;
    for id in &drops {
        edges.remove_all_edges(*id)?;
    }
    let surviving = edges.load_all_edges()?;
    drop(edges);
    let mut edge_counts = BTreeMap::<MemoryId, u16>::new();
    for edge in surviving {
        let count = edge_counts.entry(edge.source).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| invalid("edge count overflow"))?;
    }
    let db = redb::Database::open(candidate.join("meta.db"))?;
    let tx = db.begin_write()?;
    {
        let mut meta = tx.open_table(META_TABLE)?;
        let mut tags =
            tx.open_multimap_table(MultimapTableDefinition::<&str, &[u8]>::new("tag_index"))?;
        let mut namespaces = tx.open_multimap_table(MultimapTableDefinition::<u32, &[u8]>::new(
            "namespace_index",
        ))?;
        for r in &mut rows {
            let id = MemoryId::from_bytes(r.id);
            if drops.contains(&id) {
                meta.remove(r.id.as_slice())?;
                for tag in &r.tags {
                    tags.remove(tag.as_str(), r.id.as_slice())?;
                }
                namespaces.remove(r.namespace_id, r.id.as_slice())?;
            } else {
                r.edge_count = edge_counts.get(&id).copied().unwrap_or(0);
                let encoded = r
                    .encode()
                    .map_err(|_| invalid("retained record encoding failed"))?;
                meta.insert(r.id.as_slice(), encoded.as_slice())?;
            }
        }
    }
    tx.commit()?;
    drop(db);
    // FTS is derived: clear its existing SQLite indexes using the normal reset,
    // including deleted IDs. Startup rehydrates an empty index from retained rows.
    let fts = crate::search::FtsIndex::new(&candidate)
        .map_err(|_| invalid("candidate FTS open failed"))?;
    fts.clear()
        .map_err(|_| invalid("candidate FTS reset failed"))?;
    drop(fts);
    // Full legacy validation only on the candidate, after ownership repair.
    // MetadataStore::open rebuilds phase indexes; vector indexes are RAM-only.
    let engine = super::RedbStorageEngine::open(&candidate)?;
    drop(engine);
    let result = plan(&candidate)?;
    if !result.drop_ids.is_empty()
        || !result.repair_ids.is_empty()
        || result.records + approved.drop_ids.len() != approved.records
    {
        return Err(invalid("repaired candidate verification failed"));
    }
    if inventory(&backup)? != approved.inventory || inventory(&source)? != approved.inventory {
        return Err(invalid("source/backup changed during repair"));
    }
    for name in result.inventory.keys() {
        fs::File::open(candidate.join(name))?.sync_all()?;
    }
    super::fsync::fsync_dir(&candidate)?;
    let marker =
        serde_json::to_vec_pretty(&result).map_err(|_| invalid("report encoding failed"))?;
    fs::write(destination.join("validated.json"), marker)?;
    fs::File::open(destination.join("validated.json"))?.sync_all()?;
    super::fsync::fsync_dir(&destination)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
            "fixtures/released-v1.zip"
        )))
        .unwrap();
        archive.extract(&source).unwrap();
        (root, source)
    }
    #[test]
    fn approved_duplicate_repair_preserves_source_and_unaffected_vectors() {
        let (root, source) = fixture();
        let original = plan(&source).unwrap();
        assert!(original.repair_ids.is_empty());
        let db = redb::Database::open(source.join("meta.db")).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx.open_table(META_TABLE).unwrap();
            let rows: Vec<_> = table
                .iter()
                .unwrap()
                .map(|r| {
                    let (_, v) = r.unwrap();
                    DiskRecord::from_bytes(v.value()).unwrap()
                })
                .collect();
            let mut a = rows[0].clone();
            let mut b = rows[1].clone();
            b.namespace_id = a.namespace_id;
            b.vector_slot = a.vector_slot;
            a.summary = "".into();
            a.text_offset = 0;
            a.text_length = 0;
            b.summary = "repairable synthetic text".into();
            b.text_offset = 0;
            b.text_length = 0;
            table
                .insert(a.id.as_slice(), a.encode().unwrap().as_slice())
                .unwrap();
            table
                .insert(b.id.as_slice(), b.encode().unwrap().as_slice())
                .unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let initial = plan(&source).unwrap();
        let inspected = snapshot(&source, &initial.inventory).unwrap();
        let dropped = initial.drop_ids[0];
        let repaired = initial.repair_ids[0];
        let row = inspected
            .rows
            .iter()
            .find(|r| r.id == *dropped.as_bytes())
            .unwrap();
        let ns = &inspected.namespaces[&row.namespace_id];
        // Model a referenced free slot as well as a duplicate group.
        let mut vectors =
            VectorStore::open(&source.join(&ns.name), ns.embedding_dim as usize).unwrap();
        vectors.free_slot(row.vector_slot).unwrap();
        vectors.sync().unwrap();
        drop(vectors);
        let survivor = inspected
            .rows
            .iter()
            .map(|r| MemoryId::from_bytes(r.id))
            .find(|id| *id != dropped && *id != repaired)
            .unwrap();
        let edges = super::super::EdgeStore::open(&source.join("edges.db")).unwrap();
        edges
            .add_edge(
                repaired,
                dropped,
                crate::model::EdgeType::from_u8(1).unwrap(),
                0.5,
                false,
                123,
            )
            .unwrap();
        edges
            .add_edge(
                repaired,
                survivor,
                crate::model::EdgeType::from_u8(1).unwrap(),
                0.75,
                false,
                124,
            )
            .unwrap();
        let expected_edges: Vec<_> = edges
            .load_all_edges()
            .unwrap()
            .into_iter()
            .filter(|e| e.source != dropped && e.target != dropped)
            .collect();
        drop(edges);
        let fts = crate::search::FtsIndex::new(&source).unwrap();
        fts.add(
            crate::model::NamespaceId::new(row.namespace_id),
            dropped,
            "synthetic dropped search text",
            None,
            &[],
        )
        .unwrap();
        drop(fts);
        drop(inspected);
        let approved = plan(&source).unwrap();
        assert_eq!(approved.drop_ids.len(), 1);
        assert_eq!(approved.repair_ids.len(), 1);
        let docs = documents(&source, &approved).unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].document.contains("repairable synthetic text"));
        let mut v = vec![0.; docs[0].dimension as usize];
        v[0] = 1.;
        let replacements = BTreeMap::from([(docs[0].id, v)]);
        let before = snapshot(&source, &approved.inventory).unwrap();
        let target = root.path().join("repair");
        let mut bad = replacements.clone();
        bad.get_mut(&docs[0].id).unwrap()[0] = f32::NAN;
        assert!(apply(&source, &target, &approved, &bad).is_err());
        assert!(!target.exists());
        let result = apply(&source, &target, &approved, &replacements).unwrap();
        assert_eq!(result.records, approved.records - 1);
        assert_eq!(inventory(&source).unwrap(), approved.inventory);
        assert_eq!(
            inventory(&target.join("backup")).unwrap(),
            approved.inventory
        );
        let after = snapshot(&target.join("candidate"), &result.inventory).unwrap();
        let edges =
            super::super::EdgeStore::open(&after._temp.path().join("copy/edges.db")).unwrap();
        assert_eq!(edges.load_all_edges().unwrap(), expected_edges);
        drop(edges);
        for old in &before.rows {
            let id = MemoryId::from_bytes(old.id);
            if approved.drop_ids.contains(&id) {
                assert!(!after.rows.iter().any(|r| r.id == old.id));
                continue;
            }
            let new = after.rows.iter().find(|r| r.id == old.id).unwrap();
            let mut expected = old.clone();
            expected.vector_slot = new.vector_slot;
            expected.edge_count = new.edge_count;
            assert_eq!(expected.to_bytes(), new.to_bytes());
            if !approved.repair_ids.contains(&id) && old.phase != DecayPhase::Tombstone {
                assert_eq!(
                    before.vectors[&old.namespace_id]
                        .get_vector(old.vector_slot)
                        .unwrap(),
                    after.vectors[&new.namespace_id]
                        .get_vector(new.vector_slot)
                        .unwrap()
                );
            }
        }
        assert!(target.join("validated.json").is_file());
        assert!(
            crate::search::FtsIndex::new(&target.join("candidate"))
                .unwrap()
                .is_empty()
                .unwrap()
        );
    }
    #[test]
    fn changed_manifest_and_invalid_replacements_leave_no_candidate() {
        let (root, source) = fixture();
        let mut approved = plan(&source).unwrap();
        approved.drop_ids.push(MemoryId::from_bytes([9; 16]));
        let target = root.path().join("repair");
        assert!(apply(&source, &target, &approved, &BTreeMap::new()).is_err());
        assert!(!target.exists());
        let approved = plan(&source).unwrap();
        fs::write(source.join("changed"), b"new").unwrap();
        assert!(documents(&source, &approved).is_err());
    }
}
