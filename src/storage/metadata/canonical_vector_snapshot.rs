use super::*;
use crate::storage::canonical_vector_snapshot::*;
use crate::storage::{canonical_schema, vectors::VectorManager};
use sha2::{Digest, Sha256};

fn unavailable(message: &str) -> SnapshotError {
    StorageError::OwnershipMismatch(message.into()).into()
}
fn current(tx: &redb::ReadTransaction) -> Result<ControlState, StorageError> {
    let table = tx.open_table(control::CONTROL)?;
    let value = table
        .get("state")?
        .ok_or_else(|| StorageError::InvalidControl("missing control".into()))?;
    let state = ControlState::decode(value.value())?;
    Ok(state)
}
fn binding(state: &ControlState, namespace: NamespaceId, bytes: &[u8]) -> SnapshotBinding {
    SnapshotBinding {
        at: state.generation_ref(),
        namespace_id: namespace,
        namespace_sha256: Sha256::digest(bytes).into(),
    }
}
fn charge(total: &mut usize, amount: usize, limit: usize) -> Result<(), SnapshotError> {
    *total = total
        .checked_add(amount)
        .filter(|v| *v <= limit)
        .ok_or(SnapshotError::LimitExceeded)?;
    Ok(())
}
fn validate_reference(record: &DiskRecord, values: &[f32]) -> Result<(), SnapshotError> {
    if let Some(reference) = record.schema.as_ref().and_then(|s| s.vector.as_ref()) {
        let mut hash = Sha256::new();
        for value in values {
            hash.update(value.to_le_bytes());
        }
        if reference.slot != record.vector_slot
            || reference.sha256 != <[u8; 32]>::from(hash.finalize())
        {
            return Err(unavailable("canonical vector reference/hash mismatch"));
        }
    }
    Ok(())
}
impl MetadataStore {
    pub(crate) fn capture_canonical_vectors(
        &self,
        vectors: &VectorManager,
        name: &str,
        expected: Option<&SnapshotBinding>,
        limits: SnapshotLimits,
    ) -> Result<CanonicalVectorSnapshot, SnapshotError> {
        if limits.max_records == 0 || limits.max_bytes == 0 {
            return Err(SnapshotError::LimitExceeded);
        }
        let _publication = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        // One transaction supplies generation, namespace policy and every record.
        let tx = self.db.begin_read().map_err(StorageError::from)?;
        let state = current(&tx)?;
        let namespaces = tx.open_table(NAMESPACE_TABLE).map_err(StorageError::from)?;
        let mut selected = None;
        for row in namespaces.iter().map_err(StorageError::from)? {
            let (key, value) = row.map_err(StorageError::from)?;
            if value.value().len() > limits.max_bytes {
                return Err(SnapshotError::LimitExceeded);
            }
            let config = NamespaceConfig::decode(value.value()).map_err(StorageError::from)?;
            if config.id.get() != key.value() {
                return Err(unavailable("namespace key/ID mismatch"));
            }
            if config.name == name {
                if selected.is_some() {
                    return Err(unavailable("duplicate namespace name"));
                }
                selected = Some((binding(&state, config.id, value.value()), config));
            }
        }
        let Some((binding, namespace)) = selected else {
            return Err(if expected.is_some() {
                SnapshotError::Stale
            } else {
                SnapshotError::NamespaceNotFound
            });
        };
        if expected.is_some_and(|value| value != &binding) {
            return Err(SnapshotError::Stale);
        }
        crate::storage::lineage::ensure_serving(&tx, &state)?;
        let schema = canonical_schema::read_state(&tx, &state)?;
        let store = vectors
            .get(namespace.id)
            .ok_or_else(|| unavailable("namespace vector store missing"))?;
        if store.dimensions() != namespace.embedding_dim {
            return Err(unavailable("namespace vector dimension mismatch"));
        }
        let table = tx.open_table(META_TABLE).map_err(StorageError::from)?;
        let mut rows = Vec::new();
        let mut used = 0usize;
        let mut accounting = SnapshotAccounting::default();
        let mut owners = std::collections::HashSet::new();
        for row in table.iter().map_err(StorageError::from)? {
            let (key, bytes) = row.map_err(StorageError::from)?;
            // Bound a single decode even for an unrelated namespace. Never decode
            // an unbounded row and then discover that its allocation exceeded budget.
            if used
                .checked_add(bytes.value().len())
                .is_none_or(|n| n > limits.max_bytes)
            {
                return Err(SnapshotError::LimitExceeded);
            }
            let record = DiskRecord::from_bytes(bytes.value()).map_err(StorageError::from)?;
            if record.id.as_slice() != key.value() {
                return Err(unavailable("record key/ID mismatch"));
            }
            if record.namespace_id != namespace.id.get() {
                continue;
            }
            if rows.len() >= limits.max_records {
                return Err(SnapshotError::LimitExceeded);
            }
            charge(&mut used, bytes.value().len(), limits.max_bytes)?;
            accounting.record_encoded_bytes += bytes.value().len();
            canonical_schema::validate_record_namespace(&record, &namespace, schema.as_ref())?;
            let vector = if record.phase == DecayPhase::Tombstone {
                SnapshotVector::Missing
            } else {
                if !owners.insert(record.vector_slot) || store.is_free(record.vector_slot) {
                    return Err(unavailable("duplicate or freed canonical vector slot"));
                }
                // Borrow the mmap first. Budget BEFORE any to_vec/allocation.
                let values = store
                    .get_vector(record.vector_slot)
                    .ok_or_else(|| unavailable("referenced vector slot missing"))?;
                let size = values
                    .len()
                    .checked_mul(std::mem::size_of::<f32>())
                    .ok_or(SnapshotError::LimitExceeded)?;
                charge(&mut used, size, limits.max_bytes)?;
                accounting.vector_examined_bytes += size;
                validate_reference(&record, values)?;
                if values.len() != namespace.embedding_dim as usize {
                    SnapshotVector::Invalid(InvalidVectorReason::DimensionMismatch)
                } else if values.iter().any(|v| !v.is_finite()) {
                    SnapshotVector::Invalid(InvalidVectorReason::NonFinite)
                } else if values.iter().all(|v| *v == 0.) {
                    SnapshotVector::Invalid(InvalidVectorReason::ZeroNorm)
                } else {
                    if matches!(
                        namespace.schema.as_ref().map(|s| &s.embedding_binding),
                        Some(crate::model::schema::EmbeddingBinding::Bound { .. })
                    ) {
                        canonical_schema::validate_bound_vector(values)?;
                    }
                    accounting.vector_copied_bytes += size;
                    SnapshotVector::Available(values.to_vec())
                }
            };
            rows.push(CanonicalVectorRow {
                id: MemoryId::from_bytes(record.id),
                record,
                vector,
            });
        }
        // redb byte keys already provide this order; keep it explicit to consumers.
        rows.sort_unstable_by_key(|row| *row.id.as_bytes());
        Ok(CanonicalVectorSnapshot {
            binding,
            namespace,
            rows,
            accounting,
        })
    }

    pub(crate) fn validate_canonical_vectors(
        &self,
        expected: &SnapshotBinding,
    ) -> Result<(), SnapshotError> {
        let _publication = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        let tx = self.db.begin_read().map_err(StorageError::from)?;
        let state = current(&tx)?;
        if state.generation_ref() != expected.at {
            return Err(SnapshotError::Stale);
        }
        crate::storage::lineage::ensure_serving(&tx, &state)?;
        let namespaces = tx.open_table(NAMESPACE_TABLE).map_err(StorageError::from)?;
        let value = namespaces
            .get(expected.namespace_id.get())
            .map_err(StorageError::from)?
            .ok_or(SnapshotError::Stale)?;
        if binding(&state, expected.namespace_id, value.value()) != *expected {
            return Err(SnapshotError::Stale);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::bridge::StorageEngine as _;
    use crate::storage::{StorageEngine, catalog::NamespacePolicy};
    use crate::test_support::Fixture;
    fn limits() -> SnapshotLimits {
        SnapshotLimits {
            max_records: 100,
            max_bytes: 1024 * 1024,
        }
    }
    async fn insert(f: &Fixture) -> MemoryId {
        let input = serde_json::from_value(serde_json::json!({
            "summary":"snapshot", "tags":[], "namespace":"default",
            "embedding":[1.,-0.,0.,0.], "checkDuplicates":false,
        }))
        .unwrap();
        let stored = f.storage_adapter().store_memory(input).await.unwrap();
        MemoryId::from_uuid(uuid::Uuid::parse_str(&stored.id).unwrap())
    }
    #[tokio::test]
    async fn canonical_vector_snapshot_owned_bits_order_and_actual_mutation_stale() {
        let f = Fixture::new();
        insert(&f).await;
        insert(&f).await;
        let snapshot = f
            .storage
            .read()
            .unwrap()
            .canonical_vector_snapshot("default", None, limits())
            .unwrap();
        assert_eq!(snapshot.namespace.id, NamespaceId::new(1));
        assert!(
            snapshot.namespace.schema.is_none(),
            "unknown legacy profile is never blessed"
        );
        assert!(
            snapshot
                .rows
                .windows(2)
                .all(|r| r[0].id.as_bytes() < r[1].id.as_bytes())
        );
        let SnapshotVector::Available(vector) = &snapshot.rows[0].vector else {
            panic!()
        };
        let bits: Vec<_> = vector.iter().map(|v| v.to_bits()).collect();
        assert_eq!(bits, vec![1f32.to_bits(), (-0f32).to_bits(), 0, 0]);
        f.storage
            .read()
            .unwrap()
            .validate_vector_snapshot(&snapshot.binding)
            .unwrap();
        insert(&f).await;
        let engine = f.storage.read().unwrap();
        assert!(matches!(
            engine.validate_vector_snapshot(&snapshot.binding),
            Err(SnapshotError::Stale)
        ));
        assert!(matches!(
            engine.canonical_vector_snapshot("default", Some(&snapshot.binding), limits()),
            Err(SnapshotError::Stale)
        ));
        assert_eq!(vector.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), bits);
    }
    #[tokio::test]
    async fn canonical_vector_snapshot_catalog_and_observed_writes_invalidate_without_effect_epoch()
    {
        let f = Fixture::new();
        let id = insert(&f).await;
        let engine = f.storage.read().unwrap();
        let snapshot = engine
            .canonical_vector_snapshot("default", None, limits())
            .unwrap();
        let epoch = engine.meta_store().effects_stamp().unwrap();
        let mut policy = NamespacePolicy::from(&snapshot.namespace);
        policy.initial_stability += 1.;
        engine
            .update_namespace_policy_with_receipt(
                &MutationContext::unkeyed(snapshot.namespace.id),
                &policy,
            )
            .unwrap();
        assert_eq!(engine.meta_store().effects_stamp().unwrap(), epoch);
        assert!(matches!(
            engine.validate_vector_snapshot(&snapshot.binding),
            Err(SnapshotError::Stale)
        ));
        let snapshot = engine
            .canonical_vector_snapshot("default", None, limits())
            .unwrap();
        engine
            .record_exposure_batch(
                &MutationContext::unkeyed(snapshot.namespace.id),
                &[(id, AccessKind::DirectRetrieval)],
                123,
            )
            .unwrap();
        assert_eq!(engine.meta_store().effects_stamp().unwrap(), epoch);
        assert!(matches!(
            engine.validate_vector_snapshot(&snapshot.binding),
            Err(SnapshotError::Stale)
        ));
    }
    #[tokio::test]
    async fn canonical_vector_snapshot_counts_terminal_rows_and_checks_bytes_before_copy() {
        let f = Fixture::new();
        let id = insert(&f).await;
        insert(&f).await;
        let mut engine = f.storage.write().unwrap();
        engine
            .tombstone_memory_with_receipt(&MutationContext::unkeyed(NamespaceId::new(1)), id)
            .unwrap();
        let snapshot = engine
            .canonical_vector_snapshot("default", None, limits())
            .unwrap();
        assert_eq!(snapshot.rows.len(), 2);
        let terminal = snapshot.rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(terminal.record.phase, DecayPhase::Tombstone);
        assert!(matches!(terminal.vector, SnapshotVector::Missing));
        assert!(matches!(
            engine.canonical_vector_snapshot(
                "default",
                None,
                SnapshotLimits {
                    max_records: 1,
                    ..limits()
                }
            ),
            Err(SnapshotError::LimitExceeded)
        ));
        assert!(matches!(
            engine.canonical_vector_snapshot(
                "default",
                None,
                SnapshotLimits {
                    max_bytes: 1,
                    ..limits()
                }
            ),
            Err(SnapshotError::LimitExceeded)
        ));
        let mut used = usize::MAX;
        assert!(matches!(
            charge(&mut used, 1, usize::MAX),
            Err(SnapshotError::LimitExceeded)
        ));
        assert!(matches!(
            engine.canonical_vector_snapshot("absent", None, limits()),
            Err(SnapshotError::NamespaceNotFound)
        ));
        assert!(matches!(
            engine.canonical_vector_snapshot("absent", Some(&snapshot.binding), limits()),
            Err(SnapshotError::Stale)
        ));
    }
    #[tokio::test]
    async fn canonical_vector_snapshot_referenced_slot_corruption_and_pending_effects_are_whole_errors()
     {
        for duplicate in [false, true] {
            let f = Fixture::new();
            let first = insert(&f).await;
            let second = insert(&f).await;
            let engine = f.storage.read().unwrap();
            let mut record = engine.get_record(first).unwrap().unwrap();
            record.vector_slot = if duplicate {
                engine.get_record(second).unwrap().unwrap().vector_slot
            } else {
                u32::MAX
            };
            // Corruption injection only; no production bypass is added.
            let tx = engine.meta_store().db.begin_write().unwrap();
            tx.open_table(META_TABLE)
                .unwrap()
                .insert(first.as_bytes().as_slice(), record.to_bytes().as_slice())
                .unwrap();
            tx.commit().unwrap();
            assert!(matches!(
                engine.canonical_vector_snapshot("default", None, limits()),
                Err(SnapshotError::Unavailable(StorageError::OwnershipMismatch(
                    _
                )))
            ));
        }
        let f = Fixture::new();
        insert(&f).await;
        let engine = f.storage.read().unwrap();
        engine
            .meta_store()
            .canonical_pending
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            engine.canonical_vector_snapshot("default", None, limits()),
            Err(SnapshotError::Unavailable(_))
        ));
    }
    #[tokio::test]
    async fn canonical_vector_snapshot_binding_catches_unversioned_policy_and_lineage_corruption() {
        for lineage in [false, true] {
            let f = Fixture::new();
            insert(&f).await;
            let engine = f.storage.read().unwrap();
            let snapshot = engine
                .canonical_vector_snapshot("default", None, limits())
                .unwrap();
            let tx = engine.meta_store().db.begin_write().unwrap();
            if lineage {
                let mut state = engine.control_state().unwrap();
                state.store_id = uuid::Uuid::new_v4();
                tx.open_table(control::CONTROL)
                    .unwrap()
                    .insert("state", control::encode(&state).unwrap().as_slice())
                    .unwrap();
            } else {
                let mut namespace = snapshot.namespace.clone();
                namespace.initial_stability += 1.;
                tx.open_table(NAMESPACE_TABLE)
                    .unwrap()
                    .insert(
                        namespace.id.get(),
                        control::encode(&namespace).unwrap().as_slice(),
                    )
                    .unwrap();
            }
            tx.commit().unwrap();
            assert!(matches!(
                engine.validate_vector_snapshot(&snapshot.binding),
                Err(SnapshotError::Stale)
            ));
        }
    }
    #[tokio::test]
    async fn canonical_vector_snapshot_ghost_invalid_accounting_and_reference_digest() {
        use crate::test_support::MemorySpec;
        let f = Fixture::new();
        let ghost = f
            .insert_memory_with(MemorySpec {
                phase: DecayPhase::Ghost,
                embedding: Some(&[1., -0., 0., 0.]),
                ..MemorySpec::new(NamespaceId::new(1))
            })
            .await;
        f.insert_memory().await; // Explicit cfg(test) legacy zero vector.
        let engine = f.storage.read().unwrap();
        let snapshot = engine
            .canonical_vector_snapshot("default", None, limits())
            .unwrap();
        assert_eq!(snapshot.rows.len(), 2);
        let row = snapshot.rows.iter().find(|r| r.id == ghost).unwrap();
        assert_eq!(row.record.phase, DecayPhase::Ghost);
        assert!(matches!(row.vector, SnapshotVector::Available(_)));
        assert!(snapshot.rows.iter().any(|r| matches!(
            r.vector,
            SnapshotVector::Invalid(InvalidVectorReason::ZeroNorm)
        )));
        assert_eq!(snapshot.accounting.vector_examined_bytes, 32);
        assert_eq!(snapshot.accounting.vector_copied_bytes, 16);
        assert_eq!(
            snapshot.accounting.record_encoded_bytes,
            snapshot
                .rows
                .iter()
                .map(|r| r.record.to_bytes().len())
                .sum::<usize>()
        );
        // This exercises the same reference check used by capture, without
        // constructing or activating a fake native serving engine.
        let mut record = row.record.clone();
        record.schema = Some(crate::model::schema::RecordSchemaV2::from_legacy(
            &record,
            123,
            Some(crate::model::schema::VectorReference {
                slot: record.vector_slot,
                allocation_epoch: control::Generation::default(),
                sha256: [0; 32],
            }),
        ));
        assert!(matches!(
            validate_reference(&record, &[1., -0., 0., 0.]),
            Err(SnapshotError::Unavailable(StorageError::OwnershipMismatch(
                _
            )))
        ));
    }

    #[tokio::test]
    async fn canonical_vector_snapshot_waits_for_real_metadata_publication_guard() {
        let f = Fixture::new();
        insert(&f).await;
        let engine = f.storage.read().unwrap();
        let publication = engine.meta_store().publication.write().unwrap();
        let storage = f.storage.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let reader = storage.read().unwrap();
            started_tx.send(()).unwrap();
            let snapshot = reader.canonical_vector_snapshot("default", None, limits());
            done_tx.send(snapshot).unwrap();
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(matches!(
            done_rx.recv_timeout(std::time::Duration::from_millis(100)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        drop(publication);
        let snapshot = done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap();
        engine.validate_vector_snapshot(&snapshot.binding).unwrap();
        worker.join().unwrap();
    }
}
