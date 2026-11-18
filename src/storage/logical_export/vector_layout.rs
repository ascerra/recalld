//! Import preserves references protected by immutable native proofs. It creates
//! only zero placeholders for sparse historical slots, never old log payload.
use super::*;
use crate::model::NamespaceId;
use std::collections::{HashMap, HashSet};

pub(super) struct VectorLayout {
    version: u32,
    namespaces: HashMap<u32, (bool, u32)>,
    high: HashMap<NamespaceId, u32>,
    counts: HashMap<u32, u64>,
    capacities: HashMap<u32, u64>,
    positions: HashSet<(u32, u32)>,
    records: u64,
    encoded_bytes: u64,
    capacity: u64,
    history_rows: u64,
    history_bytes: u64,
    certified_history: bool,
    prepared: bool,
}
impl VectorLayout {
    pub(super) fn new(version: u32) -> Self {
        Self {
            version,
            namespaces: HashMap::new(),
            high: HashMap::new(),
            counts: HashMap::new(),
            capacities: HashMap::new(),
            positions: HashSet::new(),
            records: 0,
            encoded_bytes: 0,
            capacity: 0,
            history_rows: 0,
            history_bytes: 0,
            certified_history: false,
            prepared: false,
        }
    }
    pub(super) fn capacity(&self) -> u64 {
        self.capacity
    }
    pub(super) fn finish(&self) -> Result<(), StorageError> {
        if self.version == 2 && !self.certified_history {
            return Err(invalid("format2 lacks retained certification history"));
        }
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), StorageError> {
        self.encoded_bytes = self
            .encoded_bytes
            .checked_add(bytes as u64)
            .ok_or_else(|| invalid("format2 accounting overflow"))?;
        if self.encoded_bytes > crate::storage::vector_certification::MAX_BYTES {
            return Err(invalid("format2 encoded inventory exceeds 256 MiB"));
        }
        Ok(())
    }
    pub(super) fn observe(&mut self, item: &Item, table: &str) -> Result<(), StorageError> {
        if let Item::Row { key, value } = item {
            if self.version == 2 {
                self.history_rows += 1;
                self.history_bytes = self
                    .history_bytes
                    .checked_add(key.len() as u64)
                    .and_then(|n| n.checked_add(value.len() as u64))
                    .ok_or_else(|| invalid("format2 history accounting overflow"))?;
                if self.history_rows > crate::storage::vector_certification::MAX_HISTORY_ROWS
                    || self.history_bytes > crate::storage::vector_certification::MAX_HISTORY_BYTES
                {
                    return Err(invalid(
                        "format2 canonical table prepass exceeds bounded history limit",
                    ));
                }
            }
            if table == "mutation_receipts" {
                let receipt = crate::storage::control::validate_receipt(value)?.receipt;
                use crate::storage::control::MutationOutcome;
                let certified = match receipt.outcome {
                    MutationOutcome::VectorsReembedded { .. } => true,
                    MutationOutcome::NativeNamespaceCreated { transition } => {
                        transition.vector_certification.is_some()
                    }
                    _ => false,
                };
                if certified && self.version != 2 {
                    return Err(invalid("certified history requires export format2"));
                }
                self.certified_history |= certified;
            }
        }
        match item {
            Item::Namespace(ns) => {
                let certified = ns
                    .schema
                    .as_ref()
                    .is_some_and(|s| s.vector_certification.is_some());
                if self.version == 1 {
                    if certified {
                        return Err(invalid("certified namespace requires export format2"));
                    }
                    return Ok(());
                }
                self.charge(encoded(ns)?.len())?;
                if self.namespaces.len() >= crate::storage::vector_certification::MAX_ROWS as usize
                {
                    return Err(invalid("format2 namespace count limit exceeded"));
                }
                self.namespaces
                    .insert(ns.id.get(), (certified, ns.embedding_dim));
                self.capacity = self
                    .capacity
                    .checked_add(crate::storage::vectors::VectorFileHeader::SIZE as u64)
                    .ok_or_else(|| invalid("format2 capacity overflow"))?;
                self.capacities.insert(
                    ns.id.get(),
                    crate::storage::vectors::VectorFileHeader::SIZE as u64,
                );
            }
            Item::Memory(memory) => {
                if self.version == 1 {
                    if memory.vector_placement.is_some() {
                        return Err(invalid("format1 forbids vector placement"));
                    }
                    return Ok(());
                }
                self.records += 1;
                if self.records > crate::storage::vector_certification::MAX_ROWS {
                    return Err(invalid("format2 record count limit exceeded"));
                }
                self.charge(memory.record.len())?;
                if let Some(text) = &memory.full_text {
                    self.charge(text.len())?;
                }
                let record = DiskRecord::from_bytes(&memory.record)?;
                let (certified, dimension) = *self
                    .namespaces
                    .get(&record.namespace_id)
                    .ok_or_else(|| invalid("import namespace missing"))?;
                let reference = record.schema.as_ref().and_then(|s| s.vector.as_ref());
                if reference.is_some() != memory.embedding_bits.is_some()
                    || memory.vector_placement.is_some()
                        != (certified && memory.embedding_bits.is_some())
                {
                    return Err(invalid(
                        "certified vector placement/payload cardinality missing or unmarked",
                    ));
                }
                if let Some(bits) = &memory.embedding_bits {
                    if bits.len() != dimension as usize {
                        return Err(invalid("format2 vector dimension differs from namespace"));
                    }
                    self.charge(
                        bits.len()
                            .checked_mul(4)
                            .ok_or_else(|| invalid("format2 payload size overflow"))?,
                    )?;
                    let norm: f64 = bits
                        .iter()
                        .map(|b| f64::from(f32::from_bits(*b)).powi(2))
                        .sum();
                    if certified && (!norm.is_finite() || (norm - 1.).abs() > 1e-5) {
                        return Err(invalid("format2 certified vector is not normalized"));
                    }
                    let count = self.counts.entry(record.namespace_id).or_default();
                    *count += 1;
                    let extent = if let Some(slot) = memory.vector_placement {
                        if !self.positions.insert((record.namespace_id, slot)) {
                            return Err(invalid("duplicate protected vector slot"));
                        }
                        let high = self
                            .high
                            .entry(NamespaceId::new(record.namespace_id))
                            .or_insert(slot);
                        *high = (*high).max(slot);
                        u64::from(*high) + 1
                    } else {
                        *count
                    };
                    let chunk = u64::from(crate::storage::vectors::GROWTH_CHUNK);
                    let slots = extent
                        .checked_add(chunk - 1)
                        .and_then(|v| (v / chunk).checked_mul(chunk))
                        .ok_or_else(|| invalid("format2 rounded capacity overflow"))?;
                    let capacity = slots
                        .checked_mul(u64::from(dimension))
                        .and_then(|v| v.checked_mul(4))
                        .and_then(|v| {
                            v.checked_add(crate::storage::vectors::VectorFileHeader::SIZE as u64)
                        })
                        .ok_or_else(|| invalid("format2 vector capacity overflow"))?;
                    let previous = self
                        .capacities
                        .insert(record.namespace_id, capacity)
                        .ok_or_else(|| invalid("format2 namespace capacity missing"))?;
                    self.capacity = self
                        .capacity
                        .checked_sub(previous)
                        .and_then(|v| v.checked_add(capacity))
                        .ok_or_else(|| invalid("format2 aggregate capacity overflow"))?;
                    if self.capacity > crate::storage::vector_certification::MAX_BYTES {
                        return Err(invalid("format2 reconstructed capacity exceeds 256 MiB"));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn prepare(
        &mut self,
        vectors: &mut crate::storage::VectorManager,
    ) -> Result<(), StorageError> {
        if self.prepared {
            return Ok(());
        }
        let high = &self.high;
        let mut bytes = 0_u64;
        for (namespace, slot) in high {
            let store = vectors
                .get(*namespace)
                .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
            bytes = bytes
                .checked_add((u64::from(*slot) + 1) * u64::from(store.dimensions()) * 4)
                .ok_or_else(|| invalid("import physical reference size overflow"))?;
            if bytes > crate::storage::vector_certification::MAX_BYTES {
                return Err(invalid("import protected vector layout exceeds 256 MiB"));
            }
        }
        for (namespace, last) in high {
            let store = vectors
                .get_mut(*namespace)
                .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
            let zero = vec![0.; store.dimensions() as usize];
            for slot in 0..=*last {
                if slot >= store.slot_count() || store.is_free(slot) {
                    if store.insert_vector(&zero)? != slot {
                        return Err(invalid("resumed protected vector layout differs"));
                    }
                    #[cfg(test)]
                    {
                        store.sync()?;
                        super::checkpoint("vector_reserved");
                    }
                }
            }
            store.sync()?;
        }
        self.prepared = true;
        Ok(())
    }
    pub(super) fn reclaim_placeholders(
        &self,
        meta: &redb::Database,
        vectors: &mut crate::storage::VectorManager,
    ) -> Result<(), StorageError> {
        if self.version != 2 {
            return Ok(());
        }
        let read = meta.begin_read()?;
        let mut live = HashSet::new();
        for row in read
            .open_table(crate::storage::metadata::META_TABLE)?
            .iter()?
        {
            let (_, bytes) = row?;
            let record = DiskRecord::from_bytes(bytes.value())?;
            if record
                .schema
                .as_ref()
                .and_then(|s| s.vector.as_ref())
                .is_some()
            {
                live.insert((record.namespace_id, record.vector_slot));
            }
        }
        let namespaces: Vec<_> = self.high.keys().copied().collect();
        for namespace in namespaces {
            let store = vectors
                .get_mut(namespace)
                .ok_or(StorageError::NamespaceNotFound(namespace.get()))?;
            for slot in 0..store.slot_count() {
                if !store.is_free(slot) && !live.contains(&(namespace.get(), slot)) {
                    store.free_slot(slot)?;
                    #[cfg(test)]
                    {
                        store.sync()?;
                        super::checkpoint("vector_reclaimed");
                    }
                }
            }
            store.sync()?;
        }
        Ok(())
    }
}
