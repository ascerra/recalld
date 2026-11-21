use super::canonical_scope::{current, resolve_namespace};
use super::*;
use crate::{
    canonical_list::{ListData, ListEntry, ListError, ListPage, MAX_PAGE_BYTES, MAX_ROW_BYTES},
    serialization::list::{ListCursor, ListPageRequest},
    storage::{canonical_schema, canonical_vector_snapshot::SnapshotBinding},
};
impl MetadataStore {
    pub(crate) fn canonical_list_page(
        &self,
        name: &str,
        request: &ListPageRequest,
        cursor: Option<&ListCursor>,
        root: &WorkTicket,
    ) -> Result<ListPage, ListError> {
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
        let expected = cursor.map(|c| SnapshotBinding {
            at: c.at,
            namespace_id: c.namespace_id,
            namespace_sha256: c.namespace_sha256,
        });
        let (binding, namespace) =
            resolve_namespace(&tx, &state, name, expected.as_ref(), MAX_ROW_BYTES)?;
        // Native history was fully validated by owned reopen; native_control_at
        // already required that validation at this exact generation.
        let schema = if self.native_lifecycle_only {
            Some(self.owned_schema_at(&tx, state.generation_ref())?)
        } else {
            canonical_schema::read_state(&tx, &state)?
        };
        if namespace.schema.is_none() {
            for (field, prefix, values) in [
                ("entities", "entity/", &request.filters.labels.entities),
                ("topics", "topic/", &request.filters.labels.topics),
                ("emotions", "emotion/", &request.filters.labels.emotions),
            ] {
                crate::model::tag::parse_filter_tags(field, prefix, values).map_err(|_| {
                    crate::serialization::list::invalid(
                        "OUT_OF_RANGE",
                        &format!("/{field}"),
                        "structured filter cannot be encoded by this legacy namespace",
                    )
                })?;
            }
        }
        let labels = request.filters.labels.compile();
        let table = tx.open_table(META_TABLE).map_err(StorageError::from)?;
        let mut memories = Vec::new();
        let mut total = 0u64;
        let mut retained = 0usize;
        let mut anchor_seen = cursor.is_none();
        let mut more = false;
        let mut last = None;
        // redb byte keys are UUID order. Inspect canonical rows, not a potentially
        // incomplete derived namespace/tag index. No candidate cap precedes filters.
        for row in table.iter().map_err(StorageError::from)? {
            let (key, bytes) = row.map_err(StorageError::from)?;
            if bytes.value().len() > MAX_ROW_BYTES {
                return Err(ListError::TooLarge);
            }
            let record = DiskRecord::from_bytes(bytes.value()).map_err(StorageError::from)?;
            if record.id.as_slice() != key.value() {
                return Err(ListError::Unavailable);
            }
            if record.namespace_id != namespace.id.get() {
                continue;
            }
            canonical_schema::validate_record_namespace(&record, &namespace, schema.as_ref())?;
            if record.phase == DecayPhase::Tombstone
                || request
                    .filters
                    .phase
                    .is_some_and(|phase| phase != record.phase)
                || request
                    .filters
                    .time_range_start
                    .is_some_and(|t| record.created_at < t)
                || request
                    .filters
                    .time_range_end
                    .is_some_and(|t| record.created_at > t)
                || !request
                    .filters
                    .tags
                    .iter()
                    .all(|tag| record.tags.contains(tag))
                || !labels.matches(&record.tags, record.schema.as_ref().map(|s| &s.labels))
            {
                continue;
            }
            total = total
                .checked_add(1)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or(ListError::TooLarge)?;
            let id = MemoryId::from_bytes(record.id);
            if let Some(cursor) = cursor {
                if id == cursor.after {
                    anchor_seen = true;
                }
                if id.as_bytes() <= cursor.after.as_bytes() {
                    continue;
                }
            }
            if memories.len() == request.limit {
                more = true;
                continue;
            }
            retained = retained
                .checked_add(bytes.value().len())
                .filter(|n| *n <= MAX_PAGE_BYTES)
                .ok_or(ListError::TooLarge)?;
            last = Some(id);
            memories.push(ListEntry::from_record(record));
        }
        if !anchor_seen {
            return Err(ListCursor::invalid().into());
        }
        let next_cursor = if more {
            Some(
                ListCursor {
                    version: 1,
                    contract_version: 2,
                    eligibility_version: 1,
                    at: binding.at,
                    namespace_id: namespace.id,
                    namespace_sha256: binding.namespace_sha256,
                    filter_sha256: request.filter_hash(),
                    after: last.ok_or(ListError::Unavailable)?,
                }
                .encode()?,
            )
        } else {
            None
        };
        Ok(ListPage {
            binding,
            namespace: namespace.name,
            data: ListData {
                memories,
                total: total.to_string().try_into()?,
                limit: request.limit,
                complete: !more,
                next_cursor,
            },
        })
    }
}
