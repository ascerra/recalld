use super::{
    canonical_scope::{current, resolve_namespace},
    *,
};
use crate::{
    catalog_page::*,
    model::schema::LabelProvenance,
    serialization::{
        catalog_page::{CatalogCursor, CatalogKind, CatalogPageRequest},
        contract::{CatalogKind as WireCatalogKind, Scope},
    },
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

fn add(n: &mut u64) -> Result<(), CatalogError> {
    *n = n
        .checked_add(1)
        .filter(|n| *n <= i64::MAX as u64)
        .ok_or(CatalogError::CatalogTooLarge)?;
    Ok(())
}

impl MetadataStore {
    pub(crate) fn canonical_catalog_page(
        &self,
        request: &CatalogPageRequest,
        cursor: Option<&CatalogCursor>,
        root: &WorkTicket,
    ) -> Result<CatalogPage, CatalogError> {
        self.work_tracker
            .check_ticket(root)
            .map_err(StorageError::from)?;
        let _publication = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        let tx = self.db.begin_read().map_err(StorageError::from)?;
        let state = if self.native_lifecycle_only {
            self.native_control_at(&tx)?
        } else {
            self.ensure_legacy_operation()?;
            current(&tx)?
        };
        crate::storage::lineage::ensure_serving(&tx, &state)?;
        if cursor.is_some_and(|c| c.at != state.generation_ref()) {
            return Err(CatalogError::Stale);
        }
        // Native history was fully validated by owned reopen; native_control_at
        // already required that validation at this exact generation.
        let schema = if self.native_lifecycle_only {
            Some(self.owned_schema_at(&tx, state.generation_ref())?)
        } else {
            crate::storage::canonical_schema::read_state(&tx, &state)?
        };
        let table = tx.open_table(NAMESPACE_TABLE).map_err(StorageError::from)?;
        let mut namespaces = BTreeMap::new();
        let mut catalog_bytes = 0usize;
        let mut catalog_hash = Sha256::new();
        for row in table.iter().map_err(StorageError::from)? {
            let (key, raw) = row.map_err(StorageError::from)?;
            if raw.value().len() > crate::canonical_list::MAX_ROW_BYTES
                || namespaces.len() == MAX_NAMESPACE_ROWS
            {
                return Err(CatalogError::CatalogTooLarge);
            }
            catalog_bytes = catalog_bytes
                .checked_add(raw.value().len())
                .filter(|n| *n <= MAX_CATALOG_BYTES)
                .ok_or(CatalogError::CatalogTooLarge)?;
            let config = NamespaceConfig::decode(raw.value()).map_err(StorageError::from)?;
            if config.id.get() != key.value()
                || config.id.is_unset()
                || config.schema.is_some() != schema.is_some()
            {
                return Err(CatalogError::Unavailable);
            }
            if let Some(s) = &config.schema {
                s.validate(&config).map_err(|_| CatalogError::Unavailable)?;
            }
            catalog_hash.update(key.value().to_be_bytes());
            catalog_hash.update((raw.value().len() as u64).to_be_bytes());
            catalog_hash.update(raw.value());
            namespaces.insert(key.value(), (config, 0u64));
        }
        let catalog_sha256 = catalog_hash
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let selected = if request.all_namespaces {
            None
        } else {
            let name = request
                .namespace
                .as_deref()
                .ok_or(CatalogError::Unavailable)?;
            let (_, config) = resolve_namespace(
                &tx,
                &state,
                name,
                None,
                crate::canonical_list::MAX_ROW_BYTES,
            )
            .map_err(|e| {
                if cursor.is_some() {
                    CatalogError::Stale
                } else {
                    e.into()
                }
            })?;
            Some(config.id)
        };
        if cursor.is_some_and(|c| c.catalog_sha256 != catalog_sha256 || c.namespace_id != selected)
        {
            return Err(CatalogError::Stale);
        }
        let scope = match selected {
            Some(id) => Scope::Namespace {
                namespace_id: id,
                namespace: namespaces
                    .get(&id.get())
                    .ok_or(CatalogError::Unavailable)?
                    .0
                    .name
                    .clone(),
            },
            None => Scope::Catalog {
                kind: WireCatalogKind::Catalog,
            },
        };
        let mut next = CatalogCursor {
            version: 1,
            contract_version: 2,
            ordering_version: 1,
            kind: request.kind,
            at: state.generation_ref(),
            namespace_id: selected,
            catalog_sha256,
            namespace_after: cursor.and_then(|c| c.namespace_after),
            tag_after: cursor.map(|c| c.tag_after.clone()).unwrap_or_default(),
        };
        let mut vocabulary: BTreeMap<(usize, String), [u64; 3]> = BTreeMap::new();
        let mut label_bytes = 0usize;
        let mut memories_counted = 0u64;
        let records = tx.open_table(META_TABLE).map_err(StorageError::from)?;
        for row in records.iter().map_err(StorageError::from)? {
            let (key, raw) = row.map_err(StorageError::from)?;
            if raw.value().len() > crate::canonical_list::MAX_ROW_BYTES {
                return Err(CatalogError::CatalogTooLarge);
            }
            let record = DiskRecord::from_bytes(raw.value()).map_err(StorageError::from)?;
            if record.id.as_slice() != key.value() {
                return Err(CatalogError::Unavailable);
            }
            if selected.is_some_and(|id| id.get() != record.namespace_id) {
                continue;
            }
            let (namespace, count) = namespaces
                .get_mut(&record.namespace_id)
                .ok_or(CatalogError::Unavailable)?;
            crate::storage::canonical_schema::validate_record_namespace(
                &record,
                namespace,
                schema.as_ref(),
            )?;
            if record.phase == DecayPhase::Tombstone {
                continue;
            }
            add(count)?;
            add(&mut memories_counted)?;
            if request.kind == CatalogKind::Namespaces {
                continue;
            }
            // One memory contributes once per exact display label. Canonical
            // empty labels suppress old structured tags rather than falling back.
            let mut labels = BTreeSet::new();
            for tag in &record.tags {
                let derived = ["entity/", "topic/", "emotion/"]
                    .into_iter()
                    .enumerate()
                    .find_map(|(i, prefix)| {
                        tag.as_str()
                            .strip_prefix(prefix)
                            .filter(|s| !s.is_empty())
                            .map(|s| (i + 1, s))
                    });
                match derived {
                    Some((bucket, label)) if record.schema.is_none() => {
                        labels.insert((bucket, label.to_owned(), 0usize));
                    }
                    Some(_) => {}
                    None => {
                        labels.insert((0, tag.to_string(), 0));
                    }
                }
            }
            if let Some(schema) = &record.schema {
                let source = match schema.labels.provenance {
                    LabelProvenance::LegacyRetainedOnly => 1,
                    LabelProvenance::NativeV2 => 2,
                };
                for (i, values) in [
                    &schema.labels.entities,
                    &schema.labels.topics,
                    &schema.labels.emotions,
                ]
                .into_iter()
                .enumerate()
                {
                    for label in values {
                        labels.insert((i + 1, label.clone(), source));
                    }
                }
            }
            for (bucket, label, source) in labels {
                let count = vocabulary.len();
                let values = match vocabulary.entry((bucket, label)) {
                    std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        if count == MAX_LABELS {
                            return Err(CatalogError::CatalogTooLarge);
                        }
                        label_bytes = label_bytes
                            .checked_add(entry.key().1.len())
                            .filter(|n| *n <= MAX_LABEL_BYTES)
                            .ok_or(CatalogError::CatalogTooLarge)?;
                        entry.insert([0; 3])
                    }
                };
                add(&mut values[source])?;
            }
        }
        let mut retained = 0usize;
        let data = match request.kind {
            CatalogKind::Namespaces => {
                if cursor
                    .and_then(|c| c.namespace_after)
                    .is_some_and(|id| !namespaces.contains_key(&id))
                {
                    return Err(CatalogCursor::invalid().into());
                }
                let total = counter(namespaces.len() as u64)?;
                let mut entries = Vec::new();
                let mut more = false;
                for (id, (config, count)) in &namespaces {
                    if cursor
                        .and_then(|c| c.namespace_after)
                        .is_some_and(|after| *id <= after)
                    {
                        continue;
                    }
                    if entries.len() == request.limit {
                        more = true;
                        break;
                    }
                    let entry = NamespaceEntry::from_config(config, *count)?;
                    retain(&entry, &mut retained)?;
                    entries.push(entry);
                    next.namespace_after = Some(*id);
                }
                CatalogData::Namespaces(NamespaceData {
                    namespaces: entries,
                    total,
                    limit: request.limit,
                    complete: !more,
                    next_cursor: more.then(|| next.encode()).transpose()?,
                    configured_embedding: None,
                })
            }
            CatalogKind::Tags => {
                let mut buckets: [Vec<LabelEntry>; 4] = std::array::from_fn(|_| Vec::new());
                for ((bucket, name), sources) in vocabulary {
                    let count = sources
                        .iter()
                        .try_fold(0u64, |sum, n| sum.checked_add(*n))
                        .ok_or(CatalogError::CatalogTooLarge)?;
                    buckets[bucket].push(LabelEntry {
                        name,
                        count: counter(count)?,
                        sources: LabelSources {
                            raw_tags: counter(sources[0])?,
                            canonical_legacy_retained_only: counter(sources[1])?,
                            canonical_native_v2: counter(sources[2])?,
                        },
                    });
                }
                let mut output = Vec::new();
                let mut more = false;
                for (bucket, mut entries) in buckets.into_iter().enumerate() {
                    entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
                    let total = counter(entries.len() as u64)?;
                    let start =
                        if let Some(anchor) = cursor.and_then(|c| c.tag_after[bucket].as_ref()) {
                            let mut matching = entries.iter().enumerate().filter(|(_, e)| {
                                CatalogCursor::label_anchor(bucket, &e.name) == *anchor
                            });
                            let (index, _) = matching.next().ok_or_else(CatalogCursor::invalid)?;
                            if matching.next().is_some() {
                                return Err(CatalogCursor::invalid().into());
                            }
                            index + 1
                        } else {
                            0
                        };
                    let truncated = entries.len().saturating_sub(start) > request.limit;
                    more |= truncated;
                    let mut items = Vec::new();
                    for entry in entries.into_iter().skip(start).take(request.limit) {
                        retain(&entry, &mut retained)?;
                        next.tag_after[bucket] =
                            Some(CatalogCursor::label_anchor(bucket, &entry.name));
                        items.push(entry);
                    }
                    output.push(LabelBucket {
                        total,
                        truncated,
                        items,
                    });
                }
                let [tags, entities, topics, emotions]: [LabelBucket; 4] =
                    output.try_into().map_err(|_| CatalogError::Unavailable)?;
                CatalogData::Tags(TagData {
                    memories_counted: counter(memories_counted)?,
                    limit: request.limit,
                    tags,
                    entities,
                    topics,
                    emotions,
                    complete: !more,
                    next_cursor: more.then(|| next.encode()).transpose()?,
                })
            }
        };
        Ok(CatalogPage {
            at: state.generation_ref(),
            scope,
            data,
        })
    }
}
