use super::*;
use crate::storage::control::GenerationRef;
use crate::storage::inherited_policy::{
    self, AdoptInheritedDecay, InheritedDecayAdopted, InheritedDecayBinding,
};
use sha2::{Digest, Sha256};

impl MetadataStore {
    pub(crate) fn adopt_inherited_decay(
        &self,
        guard: &MutationGuard<'_>,
        request: &AdoptInheritedDecay,
        edges: &crate::storage::edges::EdgeStore,
    ) -> Result<WriteReceipt, StorageError> {
        guard.check(self)?;
        let hash = request.hash()?;
        let ctx = &request.context;
        let tx = self.db.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            return Ok(receipt);
        }
        let state = control::read_control(&tx)?;
        if state.generation_ref() != request.preview.source {
            return Err(StorageError::LifecycleConflict(
                "adoption preview source changed".into(),
            ));
        }
        let mut ns = namespace_state::require_namespace_active(&tx, request.preview.namespace_id)?;
        let actual = {
            let rows = tx.open_table(META_TABLE)?;
            let edge_read = edges.read_transaction()?;
            let edge_table = edge_read.open_table(crate::storage::edges::FORWARD_EDGES)?;
            inherited_policy::preview(
                state.generation_ref(),
                &ns,
                request.preview.multiplier_bits,
                request.preview.sampled_at,
                rows.iter()?.map(|r| {
                    let (k, v) = r?;
                    inherited_policy::decode_row(k.value(), v.value())
                }),
                edge_table.iter()?.map(|r| {
                    let (k, v) = r?;
                    crate::storage::PersistedEdge::from_forward(k.value(), v.value())
                }),
            )?
        };
        if actual != request.preview {
            return Err(StorageError::LifecycleConflict(
                "adoption preview canonical evidence changed".into(),
            ));
        }
        let changed =
            actual.prior.as_ref().map(|p| p.multiplier_bits) != Some(actual.multiplier_bits);
        let next_revision = if changed {
            actual.policy_revision.next()?
        } else {
            actual.policy_revision
        };
        let next_generation = state.committed_generation.next()?;
        let operation_id = self
            .id_source()
            .uuid(crate::storage::ids::IdKind::Operation)?;
        let after = if changed {
            InheritedDecayBinding {
                version: 1,
                multiplier_bits: actual.multiplier_bits,
                adoption: GenerationRef {
                    store_id: state.store_id,
                    generation: next_generation,
                },
                operation_id,
            }
        } else {
            actual.prior.clone().unwrap()
        };
        // Preview already bounded the namespace. Keep only IDs, never retained payload.
        let mut ids = Vec::new();
        {
            let table = tx.open_table(META_TABLE)?;
            for row in table.iter()? {
                let (k, v) = row?;
                let record = inherited_policy::decode_row(k.value(), v.value())?;
                if record.namespace_id == ns.id.get() {
                    ids.push(record.id);
                }
            }
        }
        let mut tokens = Sha256::new();
        {
            let mut table = tx.open_table(META_TABLE)?;
            for id in ids {
                let mut record = {
                    let value = table.get(id.as_slice())?.ok_or_else(|| {
                        StorageError::InvalidControl("adoption row disappeared".into())
                    })?;
                    DiskRecord::from_bytes(value.value())?
                };
                let fields = &mut record.schema.as_mut().unwrap().lifecycle;
                if changed {
                    fields.policy_revision = next_revision;
                    fields.record_generation = fields.record_generation.next()?;
                }
                tokens.update(id);
                tokens.update(fields.record_generation.get().to_be_bytes());
                if changed {
                    table.insert(id.as_slice(), record.encode()?.as_slice())?;
                }
            }
        }
        if changed {
            let schema = ns.schema.as_mut().unwrap();
            schema.policy_revision = next_revision;
            schema.inherited_decay = Some(after.clone());
            tx.open_table(NAMESPACE_TABLE)?
                .insert(ns.id.get(), control::encode(&ns)?.as_slice())?;
        }
        let transition = InheritedDecayAdopted {
            contract_version: ctx.contract_version,
            preview: actual,
            after,
            next_revision,
            record_token_digest: tokens
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        };
        let receipt = control::record_reserved_receipt(
            &tx,
            ctx,
            hash,
            MutationOutcome::InheritedDecayAdopted { transition },
            operation_id,
        )?;
        self.commit_catalog(tx, &receipt)?;
        Ok(receipt)
    }
}
