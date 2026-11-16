//! Frozen same-insertion planning and replay. No provider or payload allocation.
use super::*;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs, invalid};
use crate::storage::native_records::{NativeInsertIntent, profile_id};
use crate::storage::native_store::*;
use std::collections::BTreeSet;

pub(super) fn replay_store(
    tx: &redb::WriteTransaction,
    ctx: &MutationContext,
    input: &NativeStoreInput,
) -> Result<Option<WriteReceipt>, StorageError> {
    ctx.validate()?;
    if !matches!(ctx.scope, MutationScope::Namespace(_)) {
        return Err(StorageError::InvalidMutationContext(
            "native linked store requires namespace scope".into(),
        ));
    }
    let Some(stored) = control::keyed_receipt(tx, ctx)? else {
        return Ok(None);
    };
    let MutationOutcome::NativeMemoryInsertedWithLinks { transition } = &stored.receipt.outcome
    else {
        return Err(StorageError::IdempotencyConflict);
    };
    let primary = &transition.primary()?.after;
    let hash = input.request_hash(
        ctx,
        primary.lifecycle.stability.get(),
        primary.lifecycle.difficulty.get(),
        transition.embedding_dim,
        primary
            .content
            .profile_id
            .as_deref()
            .ok_or_else(|| invalid("linked creation profile missing"))?,
        &transition.settings,
    )?;
    control::replay_or_check(tx, ctx, hash)
}

impl MetadataStore {
    pub(crate) fn lookup_native_store_child(
        &self,
        child: &crate::storage::batch::BatchChild,
        input: &NativeStoreInput,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let _guard = self.native_guard_with_work(parent)?;
        if child.version != 3 {
            return Err(StorageError::InvalidMutationContext(
                "linked store requires batch 3".into(),
            ));
        }
        let tx = self.db.begin_write()?;
        let hash =
            crate::storage::batch::native::store_hash(&child.context(), input, &child.defaults)?;
        let replay = super::batch::replay_child(&tx, child, hash)?;
        if replay.is_none() {
            super::batch::check_policy(self, &tx, child)?;
        }
        Ok(replay)
    }

    pub(crate) fn lookup_native_store(
        &self,
        ctx: &MutationContext,
        input: &NativeStoreInput,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let _guard = self.native_guard_with_work(parent)?;
        replay_store(&self.db.begin_write()?, ctx, input)
    }
    pub(crate) fn capture_native_store(
        &self,
        scope: LifecycleScope,
        input: &NativeStoreInput,
        settings: &NativeStoreSettings,
        policy: &NativePolicyInputs,
    ) -> Result<NativeStoreIntent, StorageError> {
        settings.validate()?;
        Ok(NativeStoreIntent {
            insert: self.capture_native_insert(scope, &input.insert, policy)?,
            check_duplicates: input.check_duplicates,
            settings: settings.clone(),
        })
    }
    pub(crate) fn prepare_native_store_links(
        &self,
        intent: &NativeStoreIntent,
        links: &FrozenInsertLinks,
        policy: &NativePolicyInputs,
    ) -> Result<PreparedNativeStore, StorageError> {
        self.ensure_available()?;
        let _publication = self.publication.read().unwrap_or_else(|e| e.into_inner());
        self.ensure_available()?;
        let read = self.db.begin_read()?;
        if links.generation != intent.insert.capture.generation
            || self
                .capture_at(&read, &intent.insert.scope, &[], policy, false)?
                .generation
                != links.generation
        {
            return Err(StorageError::LifecycleConflict(
                "linked store candidate generation changed".into(),
            ));
        }
        if profile_id(&intent.insert.namespace).as_deref() != Some(&links.profile_id) {
            return Err(StorageError::InvalidMutation(
                "linked store candidate profile differs".into(),
            ));
        }
        if links.static_links.len() > crate::storage::native_graph::MAX_NATIVE_GRAPH_EDGES
            || links.advisory.is_some() != intent.check_duplicates
            || (!intent.settings.autolink_enabled && !links.static_links.is_empty())
        {
            return Err(StorageError::InvalidMutation(
                "invalid frozen linked store selection".into(),
            ));
        }
        let mut selected = BTreeSet::new();
        let mut live = BTreeSet::new();
        let mut semantic = 0;
        let mut entities = 0;
        for &(id, kind, weight) in &links.static_links {
            if !weight.is_finite()
                || !(0.0..=1.0).contains(&weight)
                || !selected.insert((id, kind.as_u8()))
            {
                return Err(StorageError::InvalidMutation(
                    "invalid frozen static link".into(),
                ));
            }
            match kind {
                crate::model::EdgeType::Associative => semantic += 1,
                crate::model::EdgeType::Entity => entities += 1,
                _ => {
                    return Err(StorageError::InvalidMutation(
                        "unsupported frozen static link kind".into(),
                    ));
                }
            }
            live.insert(id);
        }
        if semantic > intent.settings.max_auto_links || entities > intent.settings.max_entity_links
        {
            return Err(StorageError::InvalidMutation(
                "frozen static links exceed settings".into(),
            ));
        }
        let table = read.open_table(META_TABLE)?;
        let mut temporal = BTreeSet::new();
        for &(id, created) in &links.temporal {
            if !temporal.insert(id) {
                return Err(StorageError::InvalidMutation(
                    "duplicate temporal candidate".into(),
                ));
            }
            let value = table
                .get(id.as_bytes().as_slice())?
                .ok_or(StorageError::NotFound(id.into_inner()))?;
            let record = DiskRecord::from_bytes(value.value())?;
            if record.created_at != created {
                return Err(StorageError::LifecycleConflict(
                    "temporal candidate creation time differs".into(),
                ));
            }
            live.insert(id);
        }
        if let Some(advisory) = &links.advisory {
            if advisory.len() > crate::model::constants::MAX_NEAR_DUPLICATE_MATCHES {
                return Err(StorageError::InvalidMutation(
                    "advisory exceeds limit".into(),
                ));
            }
            let mut seen = BTreeSet::new();
            let mut previous: Option<(MemoryId, f32)> = None;
            for &(id, score) in advisory {
                if !seen.insert(id)
                    || !score.is_finite()
                    || !(crate::model::constants::DUPLICATE_SIMILARITY_THRESHOLD as f32..=1.)
                        .contains(&score)
                    || previous.is_some_and(|(prev, s)| {
                        s.total_cmp(&score).is_lt() || (s.total_cmp(&score).is_eq() && prev >= id)
                    })
                {
                    return Err(StorageError::InvalidMutation(
                        "invalid frozen advisory".into(),
                    ));
                }
                previous = Some((id, score));
                live.insert(id);
            }
        }
        let live_ids: Vec<_> = live.iter().copied().collect();
        self.capture_at(&read, &intent.insert.scope, &live_ids, policy, true)?;
        let mut all = live;
        all.extend(intent.insert.capture.records.iter().map(|(id, _)| *id));
        if intent
            .insert
            .original
            .memory
            .id
            .is_some_and(|id| all.contains(&id))
        {
            return Err(StorageError::InvalidMutation(
                "new record used as existing candidate".into(),
            ));
        }
        let capture = self.capture_at(
            &read,
            &intent.insert.scope,
            &all.into_iter().collect::<Vec<_>>(),
            policy,
            false,
        )?;
        let insert = NativeInsertIntent {
            capture,
            ..intent.insert.clone()
        };
        Ok(PreparedNativeStore {
            intent: NativeStoreIntent {
                insert,
                ..intent.clone()
            },
            links: links.clone(),
        })
    }
}
