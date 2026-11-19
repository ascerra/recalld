use super::*;
use crate::storage::control::{ControlState, MutationOutcome};
use crate::storage::metadata::{META_TABLE, NAMESPACE_TABLE, removal_ledger};
use redb::ReadableTable;
use std::collections::{BTreeMap, HashMap, HashSet};

fn retained_matches(
    record: &DiskRecord,
    namespace: &crate::storage::native_purge::NativeNamespaceProof,
    state: &NativeRecordState,
) -> bool {
    namespace.namespace().ok().is_some_and(|ns| {
        crate::storage::native_records::matches_retained_vector(
            record,
            &ns,
            namespace.namespace_id.get(),
            state,
        )
    })
}

/// Follow exact owned vector identities, not receipt order or numerical
/// generations from incomparable restore origins. Forks may retain alternatives.
fn reachable_move_record(
    namespace: u32,
    root: &NativeRecordState,
    id: MemoryId,
    record: &DiskRecord,
    moves: &[crate::storage::native_move::NativeMoveWrite],
    allow_removed_vector: bool,
) -> bool {
    let mut states = vec![(namespace, root)];
    let mut used = HashSet::new();
    loop {
        let mut added = false;
        for (index, movement) in moves.iter().enumerate() {
            if used.contains(&index) {
                continue;
            }
            let Some(row) = movement.records.iter().find(|r| r.change.memory_id == id) else {
                continue;
            };
            let Some(before) = &row.change.before else {
                continue;
            };
            if !states.iter().any(|(ns, state)| {
                *ns == movement.source_namespace_id.get()
                    && state.content.vector == before.content.vector
                    && state.content.profile_id == before.content.profile_id
                    && state.content.embedding_origin == before.content.embedding_origin
                    && state.lifecycle.lifecycle.record_generation
                        <= before.lifecycle.lifecycle.record_generation
            }) {
                continue;
            }
            let mut selected = record.clone();
            if allow_removed_vector && selected.schema.as_ref().is_some_and(|s| s.vector.is_none())
            {
                selected.schema.as_mut().unwrap().vector = row.change.after.content.vector.clone();
            }
            if retained_matches(&selected, &movement.destination, &row.change.after) {
                return true;
            }
            states.push((movement.destination_namespace_id.get(), &row.change.after));
            used.insert(index);
            added = true;
        }
        if !added {
            return false;
        }
    }
}

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
    let mut moves = Vec::new();
    for row in read.open_table(control::RECEIPTS)?.iter()? {
        let (key, value) = row?;
        examined += 1;
        bytes = bytes
            .checked_add(value.value().len() as u64)
            .ok_or_else(|| invalid("certification history size overflow"))?;
        let receipt = control::validate_receipt(value.value())?.receipt;
        match &receipt.outcome {
            MutationOutcome::NativeMemoriesMoved { transition } => {
                if key.value()
                    != control::scoped_key(
                        receipt.store_id,
                        MutationScope::Catalog,
                        receipt.operation_id.as_bytes(),
                    )
                    || !control::permits_generation_origin(
                        read,
                        control,
                        GenerationRef {
                            store_id: receipt.store_id,
                            generation: receipt.committed_generation,
                        },
                    )?
                {
                    return Err(invalid("move certification origin/scope mismatch"));
                }
                has_execution = true;
                moves.push(transition.clone());
            }
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
    for movement in &moves {
        for proof in [&movement.source, &movement.destination] {
            let namespace = namespaces
                .get(&proof.namespace_id.get())
                .ok_or_else(|| invalid("move historical namespace missing"))?;
            if namespace.name != proof.name
                || namespace.embedding_dim != proof.embedding_dim
                || namespace.schema.as_ref().map(|s| &s.embedding_binding)
                    != Some(&proof.schema.embedding_binding)
            {
                return Err(invalid("move historical namespace binding differs"));
            }
        }
        if movement.destination.schema.vector_certification.as_ref()
            != summary
                .selected
                .get(&movement.destination_namespace_id.get())
        {
            return Err(invalid(
                "move vector execution lacks destination certification",
            ));
        }
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
    let mut current = HashMap::new();
    let mut retained = HashMap::new();
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
            // Legacy sources have no native creation receipt. The move itself
            // must therefore bind their retained non-vector fields too; merely
            // finding a matching regenerated vector would admit label/text edits.
            for movement in &moves {
                for moved in movement.records.iter().filter(|m| m.change.memory_id == id) {
                    let after = &moved.change.after;
                    if !retained_matches(&record, &movement.destination, after) {
                        continue;
                    }
                    let namespace = namespaces
                        .get(&record.namespace_id)
                        .ok_or_else(|| invalid("move retained namespace missing"))?;
                    let actual =
                        NativeRecordState::capture(&record, namespace, after.content.text_sha256)?;
                    let mask = ledger
                        .iter()
                        .flat_map(|l| &l.entries)
                        .filter(|e| e.memory_uuid == id.into_inner())
                        .fold(0u8, |mask, e| mask | e.removed_fields_mask);
                    if actual.lifecycle.created_at != after.lifecycle.created_at
                        || actual.lifecycle.lifecycle.observed_at
                            != after.lifecycle.lifecycle.observed_at
                        || actual.lifecycle.lifecycle.timestamp_origin
                            != after.lifecycle.lifecycle.timestamp_origin
                        || actual.lifecycle.difficulty != after.lifecycle.difficulty
                        || actual.content.extensions_sha256 != after.content.extensions_sha256
                        || actual.content.label_provenance != after.content.label_provenance
                        || (mask & removal_ledger::SUMMARY == 0
                            && actual.content.summary_sha256 != after.content.summary_sha256)
                        || (mask & removal_ledger::TAGS == 0
                            && (actual.content.tags_sha256 != after.content.tags_sha256
                                || actual.content.labels_sha256 != after.content.labels_sha256))
                        || (mask & removal_ledger::FULL_TEXT == 0
                            && actual.lifecycle.has_text != after.lifecycle.has_text)
                    {
                        return Err(invalid(
                            "retained moved record differs without validated loss",
                        ));
                    }
                }
            }
            if vector.is_some()
                && !certified_creations.contains(&id)
                && !moves.iter().any(|movement| {
                    movement.destination_namespace_id.get() == record.namespace_id
                        && movement.records.iter().any(|row| {
                            row.change.memory_id == id
                                && retained_matches(
                                    &record,
                                    &movement.destination,
                                    &row.change.after,
                                )
                        })
                })
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
                    let mask = ledger
                        .iter()
                        .flat_map(|l| &l.entries)
                        .filter(|e| e.memory_uuid == id.into_inner())
                        .fold(0u8, |mask, e| mask | e.removed_fields_mask);
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
            retained.insert(id, record);
        }
    }
    for (namespace, cutover) in &cutovers {
        for row in &cutover.records {
            let id = row.record.memory_id;
            if let Some(record) = retained.get(&id) {
                let ns = namespaces
                    .get(&record.namespace_id)
                    .ok_or_else(|| invalid("moved cutover namespace missing"))?;
                let actual = super::super::native_records::ContentProof::capture(record, ns, None)?;
                let original = &row.record.after.content;
                // read_export has already validated every per-ID removal scope,
                // including historical scopes connected by typed moves.
                let mask = ledger
                    .iter()
                    .flat_map(|l| &l.entries)
                    .filter(|e| e.memory_uuid == id.into_inner())
                    .fold(0u8, |mask, e| mask | e.removed_fields_mask);
                if actual.extensions_sha256 != original.extensions_sha256
                    || actual.label_provenance != original.label_provenance
                    || (mask & removal_ledger::SUMMARY == 0
                        && actual.summary_sha256 != original.summary_sha256)
                    || (mask & removal_ledger::TAGS == 0
                        && (actual.tags_sha256 != original.tags_sha256
                            || actual.labels_sha256 != original.labels_sha256))
                    || (mask & removal_ledger::FULL_TEXT == 0
                        && (record.text_length != 0) != row.record.after.lifecycle.has_text)
                {
                    return Err(invalid(
                        "moved cutover document differs without validated removal",
                    ));
                }
            }
            match current.get(&id) {
                Some((ns, Some(vector)))
                    if ns == namespace
                        && Some(vector) == row.record.after.content.vector.as_ref() => {}
                Some((_, Some(_)))
                    if retained.get(&id).is_some_and(|record| {
                        reachable_move_record(
                            *namespace,
                            &row.record.after,
                            id,
                            record,
                            &moves,
                            false,
                        )
                    }) => {}
                Some((ns, None))
                    if ledger.iter().flat_map(|l| &l.entries).any(|e| {
                        e.memory_uuid == id.into_inner()
                            && e.removed_fields_mask & removal_ledger::EMBEDDING != 0
                    }) && (ns == namespace
                        || retained.get(&id).is_some_and(|record| {
                            reachable_move_record(
                                *namespace,
                                &row.record.after,
                                id,
                                record,
                                &moves,
                                true,
                            )
                        })) => {}
                None if ledger.iter().flat_map(|l| &l.entries).any(|e| {
                    e.memory_uuid == id.into_inner() && e.removed_fields_mask == removal_ledger::ALL
                }) => {}
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
    // A legacy source has no creation text hash. Its first regenerated move
    // commits the actual retained text, and subsequent moves must preserve it.
    // A later Summary witness must not erase an earlier Present commitment.
    for row in read.open_table(control::RECEIPTS)?.iter()? {
        let (_, value) = row?;
        if let MutationOutcome::NativeMemoriesMoved { transition } =
            control::validate_receipt(value.value())?.receipt.outcome
        {
            for moved in transition.records {
                let id = moved.change.memory_id;
                if let RetainedTextInput::Present { sha256 } = moved.document.full_text {
                    if result.get(&id).is_some_and(|old| {
                        matches!(old,
                        RetainedTextInput::Present { sha256: previous } if *previous != sha256)
                    }) {
                        return Err(invalid("move retained text commitments disagree"));
                    }
                    result.insert(id, RetainedTextInput::Present { sha256 });
                }
            }
        }
    }
    Ok(result)
}
