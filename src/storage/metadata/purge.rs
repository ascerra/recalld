//! Purge receipts and mutable progress share the record-removal transaction.
use super::*;
use crate::storage::purge::{PurgeProgress, PurgeStage};
use redb::{MultimapTableHandle, TableHandle};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) const SCHEMA: TableDefinition<&str, u32> =
    TableDefinition::new("namespace_purge_schema");
pub(crate) const PROGRESS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("namespace_purge_progress");
pub(crate) const CHUNK_LIMIT: usize = 1000;

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidControl(format!("namespace purge: {message}"))
}
fn pending() -> StorageError {
    StorageError::RecoveryRequired(
        "namespace purge pending; resume canonical deletion before admission".into(),
    )
}
fn is_purge(outcome: &MutationOutcome) -> bool {
    matches!(
        outcome,
        MutationOutcome::NamespacePurgeStarted { .. }
            | MutationOutcome::NamespacePurgeAdvanced { .. }
            | MutationOutcome::NativeNamespacePurgeStarted { .. }
            | MutationOutcome::NativeNamespacePurgeAdvanced { .. }
    )
}

fn step_fields(
    r: &WriteReceipt,
) -> Option<(
    Uuid,
    NamespaceId,
    u64,
    PurgeStage,
    u64,
    u64,
    u64,
    Option<MemoryId>,
)> {
    match &r.outcome {
        MutationOutcome::NamespacePurgeAdvanced {
            parent_operation_id,
            namespace_id,
            sequence,
            stage,
            removed,
            tombstones_removed,
            edges_removed,
            last_id,
        } => Some((
            *parent_operation_id,
            *namespace_id,
            *sequence,
            *stage,
            *removed,
            *tombstones_removed,
            *edges_removed,
            *last_id,
        )),
        MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } => {
            let (n, x, e, last) = t.counts();
            Some((
                t.parent_operation_id,
                t.namespace_id,
                t.sequence,
                t.stage,
                n,
                x,
                e,
                last,
            ))
        }
        _ => None,
    }
}
pub(super) fn start_matches(p: &PurgeProgress) -> bool {
    match &p.start_receipt.outcome {
        MutationOutcome::NamespacePurgeStarted {
            namespace_id,
            name_hash,
        } => {
            p.version == 1
                && *namespace_id == p.namespace_id
                && *name_hash == <[u8; 32]>::from(Sha256::digest(p.name.as_bytes()))
        }
        MutationOutcome::NativeNamespacePurgeStarted { transition: t } => {
            p.version == 2
                && t.before_namespace.namespace_id == p.namespace_id
                && t.before_namespace.name == p.name
        }
        _ => false,
    }
}

/// Validate canonical start identity before deriving a recovery request context.
fn start_contract(p: &PurgeProgress, bytes: &[u8]) -> Result<u32, StorageError> {
    let stored = control::validate_receipt(bytes)?;
    if stored.receipt != p.start_receipt || !start_matches(p) {
        return Err(invalid("purge canonical start differs"));
    }
    match &stored.receipt.outcome {
        MutationOutcome::NativeNamespacePurgeStarted { transition } => {
            transition.validate_request(&stored)
        }
        MutationOutcome::NamespacePurgeStarted { .. } if p.version == 1 => Ok(1),
        _ => Err(invalid("unsupported purge start codec")),
    }
}
fn start_contract_write(
    tx: &redb::WriteTransaction,
    p: &PurgeProgress,
) -> Result<u32, StorageError> {
    let table = tx.open_table(control::RECEIPTS)?;
    let key = control::scoped_key(
        p.store_id,
        MutationScope::Namespace(p.namespace_id),
        p.start_receipt.operation_id.as_bytes(),
    );
    let row = table
        .get(key.as_slice())?
        .ok_or_else(|| invalid("purge start receipt missing"))?;
    start_contract(p, row.value())
}
fn validate_child_hash(
    p: &PurgeProgress,
    contract: u32,
    sequence: u64,
    stored: &control::StoredReceipt,
) -> Result<(), StorageError> {
    if p.version != 2 {
        return Ok(());
    }
    let ctx = MutationContext {
        contract_version: contract,
        ..MutationContext::unkeyed(p.namespace_id)
    };
    if control::request_hash(
        &ctx,
        "namespace_purge_child_v2",
        &[
            p.start_receipt.operation_id.as_bytes(),
            &sequence.to_be_bytes(),
        ],
    ) != stored.request_hash
    {
        return Err(invalid("purge child request hash differs"));
    }
    Ok(())
}

/// Current completed native row on an owner whose startup history proof passed.
/// Point-check its two receipt anchors; never rescan lifetime history here.
/// Startup proved ancestry/count chains and participating writers preserve them.
pub(super) fn check_completed_native_progress(
    read: &redb::ReadTransaction,
    p: &PurgeProgress,
) -> Result<(), StorageError> {
    let MutationOutcome::NativeNamespacePurgeStarted { transition: start } =
        &p.start_receipt.outcome
    else {
        return Err(invalid("current native purge start kind"));
    };
    let MutationOutcome::NativeNamespacePurgeAdvanced { transition: last } =
        &p.last_receipt.outcome
    else {
        return Err(invalid("current native purge final kind"));
    };
    if p.version != 2
        || p.stage != PurgeStage::Complete
        || p.next_operation_id.is_some()
        || p.namespace_id.is_unset()
        || !start_matches(p)
        || last.namespace_id != p.namespace_id
        || last.parent_operation_id != p.start_receipt.operation_id
        || last.stage != p.stage
        || last.sequence != p.sequence
        || p.last_receipt.committed_generation <= p.start_receipt.committed_generation
        || p.tombstones_removed > p.removed
        || (p.removed == 0) != p.cursor.is_none()
        || p.cursor.is_some_and(|id| id == MemoryId::nil())
    {
        return Err(invalid("current native purge row/proof mismatch"));
    }
    start.validate(&p.start_receipt)?;
    last.validate(&p.last_receipt)?;
    let receipts = read.open_table(control::RECEIPTS)?;
    let mut contract = None;
    for receipt in [&p.start_receipt, &p.last_receipt] {
        if receipt.store_id != p.store_id
            || receipt.operation_id.is_nil()
            || receipt.receipt_version != 1
            || receipt.replayed
        {
            return Err(invalid("current native purge receipt identity"));
        }
        // A completed restored row is anchored in its original lineage, never
        // rewritten or looked up under the new current store ID.
        let key = control::scoped_key(
            p.store_id,
            MutationScope::Namespace(p.namespace_id),
            receipt.operation_id.as_bytes(),
        );
        let stored = receipts
            .get(key.as_slice())?
            .ok_or_else(|| invalid("current native purge receipt missing"))?;
        let checked = control::validate_receipt(stored.value())?;
        if checked.receipt != *receipt {
            return Err(invalid("current native purge receipt differs"));
        }
        if receipt.operation_id == p.start_receipt.operation_id {
            contract = Some(start_contract(p, stored.value())?);
        } else {
            validate_child_hash(
                p,
                contract.ok_or_else(|| invalid("purge start unchecked"))?,
                p.sequence,
                &checked,
            )?;
        }
    }
    Ok(())
}

/// Pure validation, also called before any writable source open. Unknown tables
/// are not adoption; unknown versions and inconsistent history are corruption.
pub(crate) fn validate_existing(
    tx: &redb::ReadTransaction,
    state: Option<&ControlState>,
) -> Result<Vec<PurgeProgress>, StorageError> {
    let reserved = [SCHEMA.name(), PROGRESS.name()];
    if tx
        .list_multimap_tables()?
        .any(|t| reserved.contains(&t.name()))
    {
        return Err(invalid("table is a multimap"));
    }
    let names: Vec<_> = tx.list_tables()?.map(|t| t.name().to_owned()).collect();
    let present = reserved.map(|n| names.iter().any(|s| s == n));
    let mut histories = Vec::new();
    let mut all_operations = std::collections::HashSet::new();
    if names.iter().any(|n| n == control::RECEIPTS.name()) {
        for row in tx.open_table(control::RECEIPTS)?.iter()? {
            let (_, value) = row?;
            let receipt = control::validate_receipt(value.value())?.receipt;
            all_operations.insert(receipt.operation_id);
            if is_purge(&receipt.outcome) {
                histories.push(receipt);
            }
        }
    }
    if !present.iter().any(|v| *v) {
        if !histories.is_empty() {
            return Err(invalid("receipts without progress tables"));
        }
        return Ok(vec![]);
    }
    if !present.iter().all(|v| *v) {
        return Err(invalid("partial schema"));
    }
    let state = state.ok_or_else(|| invalid("progress without lineage"))?;
    let schema = tx.open_table(SCHEMA)?;
    if schema.len()? != 1 || schema.get("version")?.is_none_or(|v| v.value() != 1) {
        return Err(invalid("unsupported schema"));
    }
    let table = tx.open_table(PROGRESS)?;
    let receipt_table = tx.open_table(control::RECEIPTS)?;
    let namespaces = tx.open_table(NAMESPACE_TABLE)?;
    let records = tx.open_table(META_TABLE)?;
    let mut found = Vec::new();
    let mut claimed = std::collections::HashSet::new();
    let mut namespace_ids = std::collections::HashSet::new();
    let mut leased_names = std::collections::HashSet::new();
    let mut reservations = std::collections::HashSet::new();
    for row in table.iter()? {
        let (key, bytes) = row?;
        let p: PurgeProgress = control::decode(bytes.value())?;
        crate::model::validation::validate_namespace_name(&p.name)
            .map_err(|_| invalid("invalid name lease"))?;
        // Active progress belongs to the current writer. Completed immutable
        // history may survive a restore fork only under its verified bound.
        let permitted_origin = crate::storage::lineage::permits_origin(
            tx,
            state,
            control::GenerationRef {
                store_id: p.store_id,
                generation: p.last_receipt.committed_generation,
            },
        )? && (p.store_id == state.store_id
            || p.stage == PurgeStage::Complete);
        if !matches!(p.version, 1 | 2)
            || !permitted_origin
            || p.start_receipt.store_id != p.store_id
            || p.namespace_id.is_unset()
            || p.start_receipt.replayed
            || p.last_receipt.replayed
            || p.name
                .eq_ignore_ascii_case(crate::model::constants::DEFAULT_NAMESPACE_NAME)
            || key.value() != p.start_receipt.operation_id.as_bytes()
            || !namespace_ids.insert(p.namespace_id)
            || !start_matches(&p)
            || !histories.contains(&p.start_receipt)
        {
            return Err(invalid("progress identity/start receipt mismatch"));
        }
        let parent_key = control::scoped_key(
            p.store_id,
            MutationScope::Namespace(p.namespace_id),
            p.start_receipt.operation_id.as_bytes(),
        );
        let stored_start = receipt_table
            .get(parent_key.as_slice())?
            .ok_or_else(|| invalid("purge start missing"))?;
        let contract = start_contract(&p, stored_start.value())?;
        claimed.insert(p.start_receipt.operation_id);
        let mut steps: Vec<_> = histories
            .iter()
            .filter(|r| step_fields(r).is_some_and(|v| v.0 == p.start_receipt.operation_id))
            .collect();
        steps.sort_by_key(|r| r.committed_generation);
        let mut sequence = 0;
        let mut removed = 0u64;
        let mut tombstones = 0u64;
        let mut edges = 0u64;
        let mut cursor = None;
        let mut stage = PurgeStage::Deleting;
        let mut last = &p.start_receipt;
        for receipt in steps {
            let (_, namespace_id, seq, next, n, t, e, last_id) = step_fields(receipt).unwrap();
            if (p.version == 2)
                != matches!(
                    receipt.outcome,
                    MutationOutcome::NativeNamespacePurgeAdvanced { .. }
                )
            {
                return Err(invalid("mixed purge receipt versions"));
            }
            sequence += 1;
            let child_key = control::scoped_key(
                p.store_id,
                MutationScope::Namespace(p.namespace_id),
                receipt.operation_id.as_bytes(),
            );
            let stored_child = receipt_table
                .get(child_key.as_slice())?
                .ok_or_else(|| invalid("purge child missing"))?;
            validate_child_hash(
                &p,
                contract,
                sequence,
                &control::validate_receipt(stored_child.value())?,
            )?;
            if seq != sequence
                || namespace_id != p.namespace_id
                || receipt.committed_generation <= last.committed_generation
                || receipt.store_id != p.store_id
            {
                return Err(invalid("child receipt chain"));
            }
            match (stage, next) {
                (PurgeStage::Deleting, PurgeStage::Deleting)
                    if n > 0
                        && n <= CHUNK_LIMIT as u64
                        && t <= n
                        && last_id.is_some_and(|id| {
                            id != MemoryId::nil() && cursor.is_none_or(|old| id > old)
                        }) =>
                {
                    cursor = last_id;
                }
                (PurgeStage::Deleting, PurgeStage::Cleanup)
                | (PurgeStage::Cleanup, PurgeStage::Complete)
                    if n == 0 && t == 0 && e == 0 && last_id.is_none() => {}
                _ => return Err(invalid("invalid child transition/count/cursor")),
            }
            removed = removed
                .checked_add(n)
                .ok_or_else(|| invalid("count overflow"))?;
            tombstones = tombstones
                .checked_add(t)
                .ok_or_else(|| invalid("count overflow"))?;
            edges = edges
                .checked_add(e)
                .ok_or_else(|| invalid("count overflow"))?;
            stage = next;
            last = receipt;
            claimed.insert(receipt.operation_id);
        }
        if p.sequence != sequence
            || p.stage != stage
            || p.removed != removed
            || p.tombstones_removed != tombstones
            || p.edges_removed != edges
            || p.cursor != cursor
            || &p.last_receipt != last
        {
            return Err(invalid("progress disagrees with immutable receipts"));
        }
        match (p.stage, p.next_operation_id) {
            (PurgeStage::Complete, None) => {}
            (PurgeStage::Deleting | PurgeStage::Cleanup, Some(id))
                if !id.is_nil() && !all_operations.contains(&id) && reservations.insert(id) => {}
            _ => return Err(invalid("missing, reused or invalid reserved child ID")),
        }
        let config = namespaces
            .get(p.namespace_id.get())?
            .map(|v| NamespaceConfig::decode(v.value()))
            .transpose()?;
        if let MutationOutcome::NativeNamespacePurgeStarted { transition: t } =
            &p.start_receipt.outcome
        {
            if let Some(config) = &config {
                if crate::storage::native_purge::NativeNamespaceProof::capture(config)?
                    != t.before_namespace
                {
                    return Err(invalid("native purging namespace changed"));
                }
            }
        }
        match p.stage {
            PurgeStage::Deleting
                if config
                    .as_ref()
                    .is_some_and(|c| c.id == p.namespace_id && c.name == p.name) => {}
            PurgeStage::Cleanup | PurgeStage::Complete if config.is_none() => {}
            _ => return Err(invalid("namespace configuration disagrees with progress")),
        }
        if p.stage != PurgeStage::Complete {
            if !leased_names.insert(p.name.to_ascii_lowercase()) {
                return Err(invalid("duplicate name lease"));
            }
            for row in namespaces.iter()? {
                let (id, value) = row?;
                let config = NamespaceConfig::decode(value.value())?;
                if id.value() != p.namespace_id.get() && config.name.eq_ignore_ascii_case(&p.name) {
                    return Err(invalid("name lease belongs to another namespace"));
                }
            }
        }
        for row in records.iter()? {
            let (key, value) = row?;
            let record = DiskRecord::from_bytes(value.value())?;
            if record.namespace_id == p.namespace_id.get()
                && (p.stage != PurgeStage::Deleting
                    || p.cursor
                        .is_some_and(|id| key.value() <= id.as_bytes().as_slice()))
            {
                return Err(invalid(
                    "remaining record behind cursor or after finalization",
                ));
            }
        }
        let counter = tx.open_table(NAMESPACE_COUNTER)?;
        if counter
            .get("max_id")?
            .is_none_or(|v| v.value() < p.namespace_id.get())
        {
            return Err(invalid("namespace counter behind progress"));
        }
        found.push(p);
    }
    if histories.iter().any(|r| !claimed.contains(&r.operation_id)) {
        return Err(invalid("orphan purge receipt"));
    }
    Ok(found)
}

pub(crate) fn require_no_active_write(tx: &redb::WriteTransaction) -> Result<(), StorageError> {
    if !tx.list_tables()?.any(|t| t.name() == PROGRESS.name()) {
        return Ok(());
    }
    for row in tx.open_table(PROGRESS)?.iter()? {
        let (_, bytes) = row?;
        let p: PurgeProgress = control::decode(bytes.value())?;
        if p.stage != PurgeStage::Complete {
            return Err(pending());
        }
    }
    Ok(())
}

impl MetadataStore {
    pub(crate) fn purge_progress(&self) -> Result<Vec<PurgeProgress>, StorageError> {
        let tx = self.db.begin_read()?;
        if !tx.list_tables()?.any(|t| t.name() == PROGRESS.name()) {
            return Ok(vec![]);
        }
        tx.open_table(PROGRESS)?
            .iter()?
            .map(|row| {
                let (_, bytes) = row?;
                control::decode(bytes.value())
            })
            .collect()
    }
    pub(crate) fn require_no_active_purge(&self) -> Result<(), StorageError> {
        if self.purge_pending.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(pending());
        }
        Ok(())
    }
}

fn load(tx: &redb::WriteTransaction, parent: Uuid) -> Result<PurgeProgress, StorageError> {
    let table = tx.open_table(PROGRESS)?;
    let value = table
        .get(parent.as_bytes().as_slice())?
        .ok_or_else(|| invalid("unknown parent operation"))?;
    control::decode(value.value())
}
fn save(tx: &redb::WriteTransaction, p: &PurgeProgress) -> Result<(), StorageError> {
    tx.open_table(PROGRESS)?.insert(
        p.start_receipt.operation_id.as_bytes().as_slice(),
        control::encode(p)?.as_slice(),
    )?;
    Ok(())
}

impl MetadataStore {
    /// The purge recovery path alone may enter while durable deletion admission
    /// is closed. Uncertain commits/canonical effects still block it.
    pub(crate) fn purge_guard(
        &self,
        parent: Option<&WorkTicket>,
    ) -> Result<MutationGuard<'_>, StorageError> {
        let guard = self.publication_guard(parent)?;
        self.ensure_handle_available()?;
        Ok(guard)
    }

    /// Entry-mode check only: the shared purge transaction independently checks
    /// current control schema under its mutation guard. Ordinary reads remain
    /// blocked during active purge, including on this native owner.
    pub(crate) fn require_native_purge_owner(&self) -> Result<(), StorageError> {
        if !self.native_lifecycle_only {
            return Err(StorageError::InvalidMutationContext(
                "native purge requires native owner".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn begin_purge(
        &self,
        ctx: &MutationContext,
        force: bool,
    ) -> Result<PurgeProgress, StorageError> {
        self.begin_purge_with_work(ctx, force, None)
    }
    pub(crate) fn begin_purge_with_work(
        &self,
        ctx: &MutationContext,
        force: bool,
        parent: Option<&WorkTicket>,
    ) -> Result<PurgeProgress, StorageError> {
        ctx.validate()?;
        let MutationScope::Namespace(namespace) = ctx.scope else {
            return Err(StorageError::InvalidMutationContext(
                "purge requires namespace scope".into(),
            ));
        };
        if !matches!(ctx.contract_version, 1 | 2) {
            return Err(StorageError::InvalidMutationContext(
                "unsupported purge contract".into(),
            ));
        }
        let native = self.native_lifecycle_only;
        if !native && ctx.contract_version != 1 {
            return Err(StorageError::InvalidMutationContext(
                "legacy purge requires contract1".into(),
            ));
        }
        let hash = control::request_hash(
            ctx,
            if native {
                "namespace_purge_v2"
            } else {
                "namespace_purge_v1"
            },
            &[&[u8::from(force)]],
        );
        let _guard = self.purge_guard(parent)?;
        let tx = self.db.begin_write()?;
        if (control::read_control(&tx)?.control_schema_version == 2) != native {
            return Err(invalid("purge owner/schema mismatch"));
        }
        if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            if !matches!(
                receipt.outcome,
                MutationOutcome::NamespacePurgeStarted { .. }
                    | MutationOutcome::NativeNamespacePurgeStarted { .. }
            ) {
                return Err(StorageError::IdempotencyConflict);
            }
            let mut p = load(&tx, receipt.operation_id)?;
            start_contract_write(&tx, &p)?;
            p.start_receipt = receipt;
            return Ok(p);
        }
        let config = super::namespace_state::require_namespace_active(&tx, namespace)?;
        if config.id != namespace
            || native != config.schema.is_some()
            || config
                .name
                .eq_ignore_ascii_case(crate::model::constants::DEFAULT_NAMESPACE_NAME)
        {
            return Err(StorageError::InvalidMutation(
                "default namespace or incompatible purge contract".into(),
            ));
        }
        crate::model::validation::validate_namespace_name(&config.name)
            .map_err(|_| StorageError::InvalidPath)?;
        if !force || native {
            for row in tx.open_table(META_TABLE)?.iter()? {
                let (_, value) = row?;
                let r = DiskRecord::from_bytes(value.value())?;
                if r.namespace_id != namespace.get() {
                    continue;
                }
                if native && r.schema.as_ref().is_none_or(|s| !s.extensions.is_empty()) {
                    return Err(StorageError::RecoveryRequired(
                        "native purge requires a consumer for unknown record extensions".into(),
                    ));
                }
                if !force && r.phase != DecayPhase::Tombstone {
                    return Err(StorageError::InvalidMutation(
                        "namespace contains live memories; force is required".into(),
                    ));
                }
            }
        }
        tx.open_table(SCHEMA)?.insert("version", 1)?;
        tx.open_table(PROGRESS)?;
        let outcome = if native {
            let state = ControlState::decode(
                tx.open_table(control::CONTROL)?
                    .get("state")?
                    .ok_or_else(|| invalid("control missing"))?
                    .value(),
            )?;
            if state.control_schema_version != 2 {
                return Err(invalid("native purge requires schema2"));
            }
            MutationOutcome::NativeNamespacePurgeStarted {
                transition: crate::storage::native_purge::NativePurgeStarted {
                    request:
                        crate::storage::request_context::RetainedRequestContext::from_contract(
                            ctx.contract_version,
                        )?
                        .map(|context| {
                            crate::storage::native_purge::NativePurgeRequest {
                                context,
                                force,
                                expected_generation: ctx.expected_generation,
                            }
                        }),
                    previous_generation: state.committed_generation,
                    before_namespace: crate::storage::native_purge::NativeNamespaceProof::capture(
                        &config,
                    )?,
                },
            }
        } else {
            MutationOutcome::NamespacePurgeStarted {
                namespace_id: namespace,
                name_hash: Sha256::digest(config.name.as_bytes()).into(),
            }
        };
        let receipt = control::record_receipt(&tx, ctx, hash, outcome)?;
        let p = PurgeProgress {
            version: if native { 2 } else { 1 },
            store_id: receipt.store_id,
            namespace_id: namespace,
            name: config.name,
            start_receipt: receipt.clone(),
            last_receipt: receipt.clone(),
            sequence: 0,
            stage: PurgeStage::Deleting,
            next_operation_id: Some(Uuid::new_v4()),
            cursor: None,
            removed: 0,
            tombstones_removed: 0,
            edges_removed: 0,
        };
        save(&tx, &p)?;
        self.purge_pending
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.commit_purge(tx, &receipt)?;
        Ok(p)
    }

    fn commit_purge(
        &self,
        tx: redb::WriteTransaction,
        receipt: &WriteReceipt,
    ) -> Result<(), StorageError> {
        super::effects::crash_point("before_purge_commit");
        let result = tx.commit();
        #[cfg(test)]
        let result = result.and_then(|()| {
            if super::effects::FAIL_AFTER_RECORD_COMMIT.with(|f| f.replace(false)) {
                Err(redb::CommitError::Storage(redb::StorageError::Io(
                    std::io::Error::other("injected unknown purge commit"),
                )))
            } else {
                Ok(())
            }
        });
        result.map_err(|source| {
            self.mark_unavailable();
            StorageError::CommitOutcomeUnknown {
                operation_id: receipt.operation_id,
                source,
            }
        })?;
        self.view_publication.canonical_committed();
        super::effects::crash_point("after_purge_commit");
        Ok(())
    }
}

/// Validate the durable reservation, consume it once, and reserve its successor
/// in the SAME transaction as the child mutation. No caller-selected ID API.
pub(super) fn record_next(
    tx: &redb::WriteTransaction,
    expected: &PurgeProgress,
    stage: PurgeStage,
    removed: &[(MemoryId, DiskRecord)],
    edges_removed: usize,
) -> Result<WriteReceipt, StorageError> {
    record_next_native(tx, expected, stage, removed, edges_removed, None)
}
pub(super) fn record_next_native(
    tx: &redb::WriteTransaction,
    expected: &PurgeProgress,
    stage: PurgeStage,
    removed: &[(MemoryId, DiskRecord)],
    edges_removed: usize,
    chunk: Option<crate::storage::native_purge::NativePurgeChunk>,
) -> Result<WriteReceipt, StorageError> {
    let mut p = load(tx, expected.start_receipt.operation_id)?;
    if &p != expected || p.stage == PurgeStage::Complete {
        return Err(invalid("stale progress reservation"));
    }
    let operation = p
        .next_operation_id
        .ok_or_else(|| invalid("unreserved child"))?;
    match (p.stage, stage) {
        (PurgeStage::Deleting, PurgeStage::Deleting)
            if !removed.is_empty() && removed.len() <= CHUNK_LIMIT => {}
        (PurgeStage::Deleting, PurgeStage::Cleanup)
        | (PurgeStage::Cleanup, PurgeStage::Complete)
            if removed.is_empty() && edges_removed == 0 => {}
        _ => return Err(invalid("invalid child transition")),
    }
    let last_id = removed.last().map(|(id, _)| *id);
    if removed.windows(2).any(|w| w[0].0 >= w[1].0)
        || removed.iter().any(|(id, r)| {
            r.namespace_id != p.namespace_id.get()
                || r.id != *id.as_bytes()
                || p.cursor.is_some_and(|old| *id <= old)
        })
    {
        return Err(invalid("child records disagree with reservation"));
    }
    let tombstones = removed
        .iter()
        .filter(|(_, r)| r.phase == DecayPhase::Tombstone)
        .count() as u64;
    let sequence = p
        .sequence
        .checked_add(1)
        .ok_or(StorageError::GenerationExhausted)?;
    let outcome = if p.version == 2 {
        let state = ControlState::decode(
            tx.open_table(control::CONTROL)?
                .get("state")?
                .ok_or_else(|| invalid("control missing"))?
                .value(),
        )?;
        MutationOutcome::NativeNamespacePurgeAdvanced {
            transition: crate::storage::native_purge::NativePurgeAdvanced {
                parent_operation_id: p.start_receipt.operation_id,
                namespace_id: p.namespace_id,
                previous_generation: state.committed_generation,
                sequence,
                stage,
                chunk,
            },
        }
    } else {
        MutationOutcome::NamespacePurgeAdvanced {
            parent_operation_id: p.start_receipt.operation_id,
            namespace_id: p.namespace_id,
            sequence,
            stage,
            removed: removed.len() as u64,
            tombstones_removed: tombstones,
            edges_removed: edges_removed as u64,
            last_id,
        }
    };
    let mut ctx = MutationContext::unkeyed(p.namespace_id);
    ctx.contract_version = start_contract_write(tx, &p)?;
    let hash = control::request_hash(
        &ctx,
        if p.version == 2 {
            "namespace_purge_child_v2"
        } else {
            "namespace_purge_child_v1"
        },
        &[
            p.start_receipt.operation_id.as_bytes(),
            &sequence.to_be_bytes(),
        ],
    );
    let receipt = control::record_reserved_receipt(tx, &ctx, hash, outcome, operation)?;
    if let MutationOutcome::NativeNamespacePurgeAdvanced { transition: t } = &receipt.outcome {
        t.validate(&receipt)?;
        if t.counts()
            != (
                removed.len() as u64,
                tombstones,
                edges_removed as u64,
                last_id,
            )
        {
            return Err(invalid("native chunk count differs from records"));
        }
    }
    p.sequence = sequence;
    p.stage = stage;
    p.last_receipt = receipt.clone();
    p.removed = p
        .removed
        .checked_add(removed.len() as u64)
        .ok_or(StorageError::GenerationExhausted)?;
    p.tombstones_removed = p
        .tombstones_removed
        .checked_add(tombstones)
        .ok_or(StorageError::GenerationExhausted)?;
    p.edges_removed = p
        .edges_removed
        .checked_add(edges_removed as u64)
        .ok_or(StorageError::GenerationExhausted)?;
    if last_id.is_some() {
        p.cursor = last_id;
    }
    p.next_operation_id = if stage == PurgeStage::Complete {
        None
    } else {
        Some(Uuid::new_v4())
    };
    save(tx, &p)?;
    Ok(receipt)
}

impl MetadataStore {
    pub fn lookup_namespace_purge(
        &self,
        store_id: Uuid,
        namespace: NamespaceId,
        parent: Uuid,
    ) -> Result<Option<PurgeProgress>, StorageError> {
        // Lookup is evidence, not data admission; permitted while recovery blocks
        // the serving handle, just like operation lookup after an unknown commit.
        let tx = self.db.begin_read()?;
        let state = ControlState::decode(
            tx.open_table(control::CONTROL)?
                .get("state")?
                .ok_or_else(|| invalid("missing control"))?
                .value(),
        )?;
        if store_id != state.store_id {
            return Err(StorageError::StoreLineageMismatch);
        }
        if !tx
            .list_tables()?
            .any(|table| table.name() == PROGRESS.name())
        {
            return Ok(None);
        }
        let Some(value) = tx.open_table(PROGRESS)?.get(parent.as_bytes().as_slice())? else {
            return Ok(None);
        };
        let p: PurgeProgress = control::decode(value.value())?;
        if p.namespace_id != namespace || p.store_id != store_id {
            return Ok(None);
        }
        let receipts = tx.open_table(control::RECEIPTS)?;
        let key = control::scoped_key(
            p.store_id,
            MutationScope::Namespace(p.namespace_id),
            p.start_receipt.operation_id.as_bytes(),
        );
        let stored = receipts
            .get(key.as_slice())?
            .ok_or_else(|| invalid("purge start receipt missing"))?;
        start_contract(&p, stored.value())?;
        Ok(Some(p))
    }

    pub(crate) fn advance_purge(
        &self,
        guard: &MutationGuard<'_>,
        p: &PurgeProgress,
        limit: usize,
        edges: &[crate::storage::PersistedEdge],
        mut text_hash: impl FnMut(&DiskRecord) -> Result<Option<[u8; 32]>, StorageError>,
    ) -> Result<Option<super::effects::EffectResult>, StorageError> {
        if !(1..=CHUNK_LIMIT).contains(&limit) {
            return Err(StorageError::InvalidMutation(
                "purge chunk limit must be 1..=1000".into(),
            ));
        }
        guard.check(self)?;
        let tx = self.db.begin_write()?;
        start_contract_write(&tx, p)?;
        if load(&tx, p.start_receipt.operation_id)? != *p || p.stage != PurgeStage::Deleting {
            return Err(invalid("stale deletion progress"));
        }
        let mut ids = Vec::new();
        for row in tx.open_table(META_TABLE)?.iter()? {
            let (key, bytes) = row?;
            let r = DiskRecord::from_bytes(bytes.value())?;
            if r.namespace_id != p.namespace_id.get() {
                continue;
            }
            if key.value() != r.id.as_slice() {
                return Err(invalid("record key disagrees with identity"));
            }
            let id = MemoryId::from_bytes(r.id);
            if p.cursor.is_some_and(|old| id <= old) {
                return Err(invalid("record behind deletion cursor"));
            }
            ids.push(id);
            if ids.len() == limit {
                break;
            }
        }
        if !ids.is_empty() {
            #[cfg(test)]
            if self
                .fail_purge_child_commit
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                super::effects::FAIL_AFTER_RECORD_COMMIT.with(|f| f.set(true));
            }
            let mut ctx = MutationContext::unkeyed(p.namespace_id);
            ctx.contract_version = start_contract_write(&tx, p)?;
            let mutation = if p.version == 2 {
                super::effects::RecordMutation::NativePurgeChunk(
                    p,
                    capture_chunk(&tx, &ids, edges, &mut text_hash)?,
                )
            } else {
                super::effects::RecordMutation::PurgeChunk(p)
            };
            let result = self.finish_record_effects(
                guard,
                tx,
                &ctx,
                [0; 32],
                edges,
                &[],
                &ids,
                true,
                false,
                mutation,
                None,
            )?;
            return Ok(Some(result));
        }
        // Canonical emptiness and config removal are decided under this same
        // writer guard/transaction, never from the preceding chunk's snapshot.
        let mut namespaces = tx.open_table(NAMESPACE_TABLE)?;
        let config = namespaces
            .remove(p.namespace_id.get())?
            .ok_or_else(|| invalid("missing deleting namespace"))?;
        if NamespaceConfig::decode(config.value())?.name != p.name {
            return Err(invalid("deleting namespace name changed"));
        }
        drop(config);
        drop(namespaces);
        let receipt = record_next(&tx, p, PurgeStage::Cleanup, &[], 0)?;
        self.commit_purge(tx, &receipt)?;
        Ok(None)
    }

    pub(crate) fn finish_purge_cleanup(
        &self,
        guard: &MutationGuard<'_>,
        p: &PurgeProgress,
        cleanup: impl FnOnce() -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        guard.check(self)?;
        let tx = self.db.begin_write()?;
        start_contract_write(&tx, p)?;
        if load(&tx, p.start_receipt.operation_id)? != *p || p.stage != PurgeStage::Cleanup {
            return Err(invalid("stale directory cleanup reservation"));
        }
        if tx
            .open_table(NAMESPACE_TABLE)?
            .get(p.namespace_id.get())?
            .is_some()
        {
            return Err(invalid("cleanup still has a namespace"));
        }
        // The name lease stays durable if cleanup or its acknowledgement fails.
        cleanup()?;
        let receipt = record_next(&tx, p, PurgeStage::Complete, &[], 0)?;
        let pending = super::effects::PendingEffects {
            version: 1,
            receipt: receipt.clone(),
            scope: MutationScope::Namespace(p.namespace_id),
            puts: vec![],
            deletes: vec![],
            releases: vec![],
            invalidated_ids: vec![],
            removed_ids: vec![],
            peer_counts: vec![],
            canonical_applied: true,
        };
        tx.open_table(super::effects::EFFECTS)?.insert(
            receipt.operation_id.as_bytes().as_slice(),
            control::encode(&pending)?.as_slice(),
        )?;
        let epoch = self
            .effects_epoch
            .load(std::sync::atomic::Ordering::SeqCst)
            .checked_add(1)
            .ok_or(StorageError::GenerationExhausted)?;
        self.derived_dirty
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.effects_epoch
            .store(epoch, std::sync::atomic::Ordering::SeqCst);
        self.commit_purge(tx, &receipt)?;
        self.purge_pending
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

fn capture_chunk(
    tx: &redb::WriteTransaction,
    ids: &[MemoryId],
    edges: &[crate::storage::PersistedEdge],
    text_hash: &mut impl FnMut(&DiskRecord) -> Result<Option<[u8; 32]>, StorageError>,
) -> Result<crate::storage::native_purge::NativePurgeChunk, StorageError> {
    use crate::storage::native_maintenance::{NativeDeletionWitness, NativePeerChange};
    use crate::storage::native_records::{NativeEdgeProof, NativeRecordChange, NativeRecordState};
    let records = tx.open_table(META_TABLE)?;
    let namespaces = tx.open_table(NAMESPACE_TABLE)?;
    let mut capture = |id: MemoryId| -> Result<(DiskRecord, NativeRecordState), StorageError> {
        let r = DiskRecord::from_bytes(
            records
                .get(id.as_bytes().as_slice())?
                .ok_or(StorageError::NotFound(id.into_inner()))?
                .value(),
        )?;
        let ns = NamespaceConfig::decode(
            namespaces
                .get(r.namespace_id)?
                .ok_or(StorageError::NamespaceNotFound(r.namespace_id))?
                .value(),
        )?;
        let state = NativeRecordState::capture(&r, &ns, text_hash(&r)?)?;
        Ok((r, state))
    };
    let mut deletions = Vec::new();
    for id in ids {
        let (r, before) = capture(*id)?;
        deletions.push(NativeDeletionWitness {
            memory_id: *id,
            namespace_id: NamespaceId::new(r.namespace_id),
            before,
            history: r.access_history,
        });
    }
    let mut removed_edges = edges
        .iter()
        .filter(|e| ids.contains(&e.source) || ids.contains(&e.target))
        .map(NativeEdgeProof::from)
        .collect::<Vec<_>>();
    removed_edges.sort_by_key(|e| (e.source, e.target, e.edge_type.as_u8()));
    let mut peer_ids = removed_edges
        .iter()
        .filter(|e| !ids.contains(&e.source))
        .map(|e| e.source)
        .collect::<Vec<_>>();
    peer_ids.sort();
    peer_ids.dedup();
    let mut peers = Vec::new();
    for id in peer_ids {
        let (r, before) = capture(id)?;
        peers.push(NativePeerChange {
            namespace_id: NamespaceId::new(r.namespace_id),
            change: NativeRecordChange {
                memory_id: id,
                before: Some(before.clone()),
                after: before,
            },
        });
    }
    Ok(crate::storage::native_purge::NativePurgeChunk {
        deletions,
        peers,
        removed_edges,
    })
}

#[cfg(test)]
#[path = "purge_context_tests.rs"]
mod context_tests;
