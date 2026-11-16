use super::*;
use crate::storage::control::{ControlState, MutationOutcome};
use crate::storage::metadata::{META_TABLE, NAMESPACE_TABLE, removal_ledger};
use redb::ReadableTable;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CertificationSummary {
    /// Includes strictly validated retired namespace bindings after purge.
    pub selected: BTreeMap<u32, VectorCertification>,
}

pub(crate) fn validate_restore_pair(
    selected: Option<&CertificationSummary>,
    target: Option<&CertificationSummary>,
) -> Result<(), StorageError> {
    if let Some(target) = target {
        for (id, certification) in &target.selected {
            if selected.and_then(|s| s.selected.get(id)) != Some(certification) {
                return Err(StorageError::InvalidMutation(
                    "selected backup cannot prove the retained target's vector certification; restore separately as uncertified historical data or use a compatible snapshot".into(),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_current(
    read: &redb::ReadTransaction,
    control: &ControlState,
) -> Result<CertificationSummary, StorageError> {
    validate_current_inner(read, control, None)
}

pub(crate) fn validate_current_with_pending(
    read: &redb::ReadTransaction,
    control: &ControlState,
) -> Result<(CertificationSummary, PendingBindings), StorageError> {
    let mut bindings = PendingBindings::default();
    let summary = validate_current_inner(read, control, Some(&mut bindings))?;
    Ok((summary, bindings))
}

fn validate_current_inner(
    read: &redb::ReadTransaction,
    control: &ControlState,
    pending: Option<&mut PendingBindings>,
) -> Result<CertificationSummary, StorageError> {
    #[cfg(test)]
    control::certification_scan();
    let mut examined = 0_u64;
    let mut bytes = 0_u64;
    let mut cutovers = BTreeMap::new();
    let mut empty_creations = BTreeMap::new();
    let mut has_execution = false;
    for row in read.open_table(control::RECEIPTS)?.iter()? {
        let (key, value) = row?;
        examined += 1;
        bytes = bytes
            .checked_add(value.value().len() as u64)
            .ok_or_else(|| invalid("certification history size overflow"))?;
        let receipt = control::validate_receipt(value.value())?.receipt;
        match &receipt.outcome {
            MutationOutcome::NativeNamespaceCreated { transition } => {
                if let Some(certification) = &transition.vector_certification {
                    if empty_creations
                        .insert(transition.namespace_id.get(), certification.clone())
                        .is_some()
                        || !control::permits_generation_origin(read, control, certification.origin)?
                    {
                        return Err(invalid("ambiguous certified namespace creation"));
                    }
                }
            }
            MutationOutcome::VectorsReembedded { transition } => {
                let id = transition.before_namespace.namespace_id;
                if key.value()
                    != control::scoped_key(
                        receipt.store_id,
                        MutationScope::Namespace(id),
                        receipt.operation_id.as_bytes(),
                    )
                    || !control::permits_generation_origin(
                        read,
                        control,
                        transition.certification.origin,
                    )?
                    || cutovers.insert(id.get(), transition.clone()).is_some()
                {
                    return Err(invalid("ambiguous or foreign vector certification origin"));
                }
                for origin in &transition.source_origins {
                    if !control::permits_generation_origin(read, control, *origin)? {
                        return Err(invalid("cutover source ancestry is not retained"));
                    }
                }
            }
            MutationOutcome::NativeMemoryInserted { transition } => {
                has_execution |= transition.vector_execution.is_some();
            }
            MutationOutcome::NativeMemoryInsertedWithLinks { transition } => {
                has_execution |= transition.vector_execution.is_some();
            }
            _ => {}
        }
        if (!cutovers.is_empty() || !empty_creations.is_empty() || has_execution)
            && (examined > MAX_HISTORY_ROWS || bytes > MAX_HISTORY_BYTES)
        {
            return Err(invalid(
                "certification history exceeds bounded inspection limit",
            ));
        }
    }
    let mut namespaces = HashMap::new();
    for row in read.open_table(NAMESPACE_TABLE)?.iter()? {
        let (_, value) = row?;
        let namespace = NamespaceConfig::decode(value.value())?;
        namespaces.insert(namespace.id.get(), namespace);
    }
    let progress = crate::storage::metadata::purge::validate_existing(read, Some(control))?;
    for (id, retired) in crate::storage::native_purge::retired_namespace_bindings(&progress)? {
        let namespace = retired.namespace()?;
        if let Some(current) = namespaces.get(&id) {
            if crate::storage::native_purge::NativeNamespaceProof::capture(current)? != *retired {
                return Err(invalid("certification retired namespace mismatch"));
            }
        } else {
            namespaces.insert(id, namespace);
        }
    }
    let mut summary = CertificationSummary::default();
    for (&id, namespace) in &namespaces {
        let selected = namespace
            .schema
            .as_ref()
            .and_then(|s| s.vector_certification.as_ref());
        match (selected, cutovers.get(&id)) {
            (None, None) if !empty_creations.contains_key(&id) => {}
            (Some(binding), None) if empty_creations.get(&id) == Some(binding) => {
                summary.selected.insert(id, binding.clone());
            }
            (Some(binding), Some(cutover)) if *binding == cutover.certification => {
                if empty_creations.contains_key(&id) {
                    return Err(invalid(
                        "already certified namespace cannot have a second cutover",
                    ));
                }
                let original = cutover.before_namespace.namespace()?;
                if original.id != namespace.id
                    || original.name != namespace.name
                    || original.embedding_dim != namespace.embedding_dim
                    || original.schema.as_ref().unwrap().embedding_binding
                        != namespace.schema.as_ref().unwrap().embedding_binding
                {
                    return Err(invalid("certified namespace immutable binding changed"));
                }
                summary.selected.insert(id, binding.clone());
            }
            _ => {
                return Err(invalid(
                    "required vector certification anchor/feature missing or inconsistent",
                ));
            }
        }
    }
    if cutovers
        .keys()
        .chain(empty_creations.keys())
        .any(|id| !summary.selected.contains_key(id))
    {
        return Err(invalid(
            "vector cutover lacks live or terminal namespace witness",
        ));
    }
    // Absent certification retains the old supported limits and does not
    // accumulate every historical creation in a new in-memory index.
    if summary.selected.is_empty() {
        if has_execution {
            return Err(invalid("vector execution lacks its certification anchor"));
        }
        return Ok(summary);
    }
    if examined > MAX_HISTORY_ROWS
        || bytes > MAX_HISTORY_BYTES
        || namespaces.len() as u64 > MAX_ROWS
    {
        return Err(invalid(
            "certification history exceeds bounded inspection limit",
        ));
    }
    let mut certified_creations = HashSet::new();
    for row in read.open_table(control::RECEIPTS)?.iter()? {
        let (_, value) = row?;
        let stored = control::validate_receipt(value.value())?.receipt;
        let receipt = GenerationRef {
            store_id: stored.store_id,
            generation: stored.committed_generation,
        };
        let (namespace, memory, execution) = match stored.outcome {
            MutationOutcome::NativeMemoryInserted { transition } => (
                transition.namespace_id,
                transition.memory_id,
                transition.vector_execution,
            ),
            MutationOutcome::NativeMemoryInsertedWithLinks { transition } => (
                transition.namespace_id,
                transition.memory_id,
                transition.vector_execution,
            ),
            _ => continue,
        };
        match (summary.selected.get(&namespace.get()), execution) {
            (Some(binding), Some(execution)) if execution.certification == *binding => {
                if cutovers
                    .get(&namespace.get())
                    .is_some_and(|cutover| cutover.precedes_cutover(receipt))
                {
                    return Err(invalid("vector execution predates its cutover"));
                }
                certified_creations.insert(memory);
            }
            (Some(_), None)
                if cutovers
                    .get(&namespace.get())
                    .is_some_and(|cutover| cutover.precedes_cutover(receipt)) => {}
            (None, None) => {}
            _ => {
                return Err(invalid(
                    "post-cutover vector execution evidence missing or foreign",
                ));
            }
        }
    }
    let ledger = removal_ledger::read_export(read, Some(control))?;
    let mut loss = HashMap::<(u32, Uuid), u8>::new();
    for entry in ledger.iter().flat_map(|l| &l.entries) {
        *loss
            .entry((entry.namespace_id, entry.memory_uuid))
            .or_default() |= entry.removed_fields_mask;
    }
    let mut current = HashMap::new();
    let mut count = 0_u64;
    let mut row_bytes = 0_u64;
    for row in read.open_table(META_TABLE)?.iter()? {
        let (key, value) = row?;
        count += 1;
        row_bytes = row_bytes
            .checked_add(value.value().len() as u64)
            .ok_or_else(|| invalid("inventory size overflow"))?;
        if count > MAX_ROWS || row_bytes > MAX_BYTES {
            return Err(invalid(
                "vector certification inventory exceeds bounded inspection limit",
            ));
        }
        let record = DiskRecord::from_bytes(value.value())?;
        if record.id.as_slice() != key.value() {
            return Err(invalid("certification inventory key mismatch"));
        }
        if record
            .schema
            .as_ref()
            .and_then(|s| s.vector.as_ref())
            .is_some()
        {
            let namespace = namespaces
                .get(&record.namespace_id)
                .ok_or_else(|| invalid("inventory namespace missing"))?;
            row_bytes = row_bytes
                .checked_add(u64::from(namespace.embedding_dim) * 4)
                .ok_or_else(|| invalid("inventory vector accounting overflow"))?;
            if row_bytes > MAX_BYTES {
                return Err(invalid("vector certification payload budget exceeded"));
            }
        }
        let id = MemoryId::from_bytes(record.id);
        if summary.selected.contains_key(&record.namespace_id) {
            let vector = record.schema.as_ref().and_then(|s| s.vector.as_ref());
            if vector.is_some()
                && !certified_creations.contains(&id)
                && !cutovers.get(&record.namespace_id).is_some_and(|cutover| {
                    cutover
                        .records
                        .binary_search_by_key(&id, |r| r.record.memory_id)
                        .is_ok()
                })
            {
                return Err(invalid(
                    "current vector has no certified inventory/execution proof",
                ));
            }
            if let Some(cutover) = cutovers.get(&record.namespace_id) {
                if let Ok(index) = cutover
                    .records
                    .binary_search_by_key(&id, |r| r.record.memory_id)
                {
                    let expected = &cutover.records[index];
                    let namespace = namespaces
                        .get(&record.namespace_id)
                        .ok_or_else(|| invalid("cutover namespace missing"))?;
                    let actual = super::super::native_records::ContentProof::capture(
                        &record, namespace, None,
                    )?;
                    let original = &expected.record.after.content;
                    let mask = loss
                        .get(&(record.namespace_id, id.into_inner()))
                        .copied()
                        .unwrap_or(0);
                    use removal_ledger::{FULL_TEXT, SUMMARY, TAGS};
                    if actual.profile_id != original.profile_id
                        || actual.extensions_sha256 != original.extensions_sha256
                        || actual.label_provenance != original.label_provenance
                        || (mask & SUMMARY == 0 && actual.summary_sha256 != original.summary_sha256)
                        || (mask & TAGS == 0
                            && (actual.tags_sha256 != original.tags_sha256
                                || actual.labels_sha256 != original.labels_sha256))
                        || (mask & FULL_TEXT == 0
                            && (record.text_length != 0)
                                != expected.record.after.lifecycle.has_text)
                    {
                        return Err(invalid(
                            "retained cutover document components differ without validated loss",
                        ));
                    }
                }
            }
            current.insert(id, (record.namespace_id, vector.cloned()));
        }
    }
    for (namespace, cutover) in &cutovers {
        for row in &cutover.records {
            let id = row.record.memory_id;
            match current.get(&id) {
                Some((ns, Some(vector)))
                    if ns == namespace
                        && Some(vector) == row.record.after.content.vector.as_ref() => {}
                Some((ns, None))
                    if ns == namespace
                        && loss
                            .get(&(*namespace, id.into_inner()))
                            .is_some_and(|m| m & removal_ledger::EMBEDDING != 0) => {}
                None if loss
                    .get(&(*namespace, id.into_inner()))
                    .is_some_and(|m| *m == removal_ledger::ALL) => {}
                _ => {
                    return Err(invalid(
                        "certified inventory row/reference omitted without validated loss",
                    ));
                }
            }
        }
    }
    if let Some(pending) = pending {
        *pending = PendingBindings::from_validated(&cutovers)?;
    }
    Ok(summary)
}

/// Strict payload inspection only, after typed origin/root/history validation.
/// The cutover records actual retained text even when old creation provenance
/// is unknown. It never rewrites that old provenance or runs on a query path.
pub(crate) fn retained_text_proofs(
    read: &redb::ReadTransaction,
) -> Result<HashMap<MemoryId, RetainedTextInput>, StorageError> {
    let mut result = HashMap::new();
    for row in read.open_table(control::RECEIPTS)?.iter()? {
        let (_, value) = row?;
        if let MutationOutcome::VectorsReembedded { transition } =
            control::validate_receipt(value.value())?.receipt.outcome
        {
            for replacement in transition.records {
                if result
                    .insert(replacement.record.memory_id, replacement.document.full_text)
                    .is_some()
                {
                    return Err(invalid("duplicate cutover text commitment"));
                }
            }
        }
    }
    Ok(result)
}
