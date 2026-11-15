//! Physical relocation evidence. Internal request identity remains compactText/API1.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    StorageError,
    control::{self, GenerationRef, MutationContext, MutationScope, StoredReceipt, WriteReceipt},
    metadata::text_generation::TextGeneration,
    native_records::{NativeRecordState, ProofRecord},
    text::RefMapping,
};
use crate::model::{DiskRecord, MemoryId, NamespaceConfig};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeTextRelocation {
    pub(crate) proof_version: u32,
    pub(crate) source: GenerationRef,
    pub(crate) before: TextGeneration,
    pub(crate) after: TextGeneration,
    pub(crate) record_count: u64,
    pub(crate) retained_count: u64,
    pub(crate) before_refs_sha256: [u8; 32],
    pub(crate) after_refs_sha256: [u8; 32],
    pub(crate) records_sha256: [u8; 32],
    pub(crate) copied_text_sha256: [u8; 32],
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidControl(message.into())
}

pub(crate) fn request_context(source: GenerationRef) -> MutationContext {
    MutationContext {
        store_id: Some(source.store_id),
        scope: MutationScope::Catalog,
        idempotency_key: None,
        expected_generation: Some(source.generation),
        contract_version: 1,
    }
}

impl NativeTextRelocation {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        self.before.validate(receipt.store_id)?;
        self.after.validate(receipt.store_id)?;
        if self.proof_version != 1
            || self.source.store_id != receipt.store_id
            || self.source.generation.next()? != receipt.committed_generation
            || self.before.id == self.after.id
            || self.before.format_version != 2
            || self.after.format_version != 2
            || self.retained_count > self.record_count
        {
            return Err(invalid("invalid native text relocation proof"));
        }
        Ok(())
    }
    pub(crate) fn validate_stored(&self, stored: &StoredReceipt) -> Result<(), StorageError> {
        self.validate(&stored.receipt)?;
        if stored.request_hash
            != control::request_hash(&request_context(self.source), "compactText", &[])
        {
            return Err(invalid("native text relocation request hash mismatch"));
        }
        Ok(())
    }
    pub(crate) fn validate_records<'a>(
        &self,
        rows: impl Iterator<Item = &'a ProofRecord>,
    ) -> Result<(), StorageError> {
        let (count, digest) = proof_digest(rows.map(|r| (r.namespace, r.id, &r.state)))?;
        if count != self.record_count || digest != self.records_sha256 {
            return Err(invalid(
                "native text relocation changed logical record proof",
            ));
        }
        Ok(())
    }
}

fn feed(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn proof_digest<'a>(
    rows: impl Iterator<Item = (u32, MemoryId, &'a NativeRecordState)>,
) -> Result<(u64, [u8; 32]), StorageError> {
    let mut rows: Vec<_> = rows.collect();
    rows.sort_by_key(|r| (r.0, r.1));
    let mut hash = Sha256::new();
    feed(&mut hash, b"native-text-records-v1");
    for row in &rows {
        feed(&mut hash, &control::encode(row)?);
    }
    Ok((rows.len() as u64, hash.finalize().into()))
}

pub(crate) fn record_digest(
    rows: &[(MemoryId, DiskRecord)],
    namespaces: &[NamespaceConfig],
    text_hashes: &BTreeMap<MemoryId, [u8; 32]>,
) -> Result<(u64, [u8; 32]), StorageError> {
    let namespaces: BTreeMap<_, _> = namespaces.iter().map(|ns| (ns.id.get(), ns)).collect();
    let states: Vec<_> = rows
        .iter()
        .map(|(id, r)| {
            let ns = namespaces
                .get(&r.namespace_id)
                .ok_or_else(|| invalid("text relocation namespace missing"))?;
            Ok((
                r.namespace_id,
                *id,
                NativeRecordState::capture(r, ns, text_hashes.get(id).copied())?,
            ))
        })
        .collect::<Result<_, StorageError>>()?;
    proof_digest(states.iter().map(|(ns, id, state)| (*ns, *id, state)))
}

pub(crate) fn reference_digest(refs: &RefMapping) -> Result<[u8; 32], StorageError> {
    let mut refs = refs.clone();
    refs.sort_by_key(|(id, _)| *id);
    let mut hash = Sha256::new();
    feed(&mut hash, b"native-text-references-v1");
    for (i, (id, r)) in refs.iter().enumerate() {
        if !r.is_some() || (i > 0 && refs[i - 1].0 == *id) {
            return Err(StorageError::InvalidTextRef);
        }
        feed(&mut hash, id.as_bytes());
        feed(&mut hash, &r.file_offset.to_be_bytes());
        feed(&mut hash, &r.length.to_be_bytes());
    }
    Ok(hash.finalize().into())
}

pub(crate) fn copy_digest(
    refs: &RefMapping,
    hashes: &BTreeMap<MemoryId, [u8; 32]>,
) -> Result<[u8; 32], StorageError> {
    let mut refs = refs.clone();
    refs.sort_by_key(|(id, _)| *id);
    let mut hash = Sha256::new();
    feed(&mut hash, b"native-text-copy-v1");
    if refs.len() != hashes.len() {
        return Err(StorageError::InvalidTextRef);
    }
    for (id, r) in refs {
        feed(&mut hash, id.as_bytes());
        feed(&mut hash, &r.length.to_be_bytes());
        feed(
            &mut hash,
            hashes.get(&id).ok_or(StorageError::InvalidTextRef)?,
        );
    }
    Ok(hash.finalize().into())
}
