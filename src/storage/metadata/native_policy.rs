use super::*;
use crate::storage::catalog::NamespacePolicy;
use crate::storage::native_catalog::NamespacePolicyState;
use crate::storage::native_lifecycle::{LifecycleScope, NativePolicyInputs};
use crate::storage::native_policy::{
    NativeNamespacePolicyUnchanged, NativeNamespacePolicyUpdated, NativePolicyIntent, request_hash,
};

impl MetadataStore {
    pub(crate) fn lookup_native_policy(
        &self,
        ctx: &MutationContext,
        input: &NamespacePolicy,
        parent: Option<&WorkTicket>,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let hash = request_hash(ctx, input)?;
        let _guard = self.native_guard_with_work(parent)?;
        control::replay_or_check(&self.db.begin_write()?, ctx, hash)
    }
    pub(crate) fn capture_native_policy(
        &self,
        scope: LifecycleScope,
        input: &NamespacePolicy,
        policy: &NativePolicyInputs,
        parent: Option<&WorkTicket>,
    ) -> Result<NativePolicyIntent, StorageError> {
        input.validate()?;
        let _guard = self.native_guard_with_work(parent)?;
        let capture = self.capture_at(&self.db.begin_read()?, &scope, &[], policy, false)?;
        Ok(NativePolicyIntent {
            scope,
            original: input.clone(),
            capture,
        })
    }
    pub(crate) fn apply_native_policy(
        &self,
        guard: &MutationGuard<'_>,
        ctx: &MutationContext,
        intent: &NativePolicyIntent,
        policy: &NativePolicyInputs,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        guard.check(self)?;
        let hash = request_hash(ctx, &intent.original)?;
        if ctx.scope != MutationScope::Namespace(intent.scope.namespace_id) {
            return Err(StorageError::MutationNamespaceMismatch);
        }
        let tx = self.db.begin_write()?;
        if let Some(receipt) = control::replay_or_check(&tx, ctx, hash)? {
            return Ok(Some(receipt));
        }
        let (state, mut namespace, _, _) =
            self.check_native_capture(&tx, &intent.scope, &intent.capture, policy, false)?;
        let before = NamespacePolicyState::capture(&namespace)?;
        intent.original.apply(&mut namespace);
        policy.resolve(&namespace)?;
        let after = NamespacePolicyState::capture(&namespace)?;
        if before == after {
            if ctx.idempotency_key.is_none() {
                return Ok(None);
            }
            let receipt = self.record_receipt(
                &tx,
                ctx,
                hash,
                MutationOutcome::NativeNamespacePolicyUnchanged {
                    transition: NativeNamespacePolicyUnchanged {
                        namespace_id: namespace.id,
                        lifecycle_version: intent.capture.version,
                        policy_revision: intent.capture.revision,
                        previous_generation: state.committed_generation,
                        capture_generation: intent.capture.generation,
                        policy: before,
                    },
                },
            )?;
            self.commit_catalog(tx, &receipt)?;
            return Ok(Some(receipt));
        }
        let next_revision = intent.capture.revision.next()?;
        namespace.schema.as_mut().unwrap().policy_revision = next_revision;
        // No live index: terminal identities participate in the same revision.
        let mut records = Vec::new();
        {
            let table = tx.open_table(META_TABLE)?;
            for row in table.iter()? {
                let (key, bytes) = row?;
                let mut record = DiskRecord::from_bytes(bytes.value())?;
                if record.namespace_id != namespace.id.get() {
                    continue;
                }
                if key.value() != record.id {
                    return Err(StorageError::InvalidControl(
                        "policy row key mismatch".into(),
                    ));
                }
                let fields = &mut record
                    .schema
                    .as_mut()
                    .ok_or_else(|| {
                        StorageError::InvalidControl("policy row schema missing".into())
                    })?
                    .lifecycle;
                if fields.lifecycle_version != intent.capture.version
                    || fields.policy_revision != intent.capture.revision
                {
                    return Err(StorageError::InvalidControl(
                        "policy row revision mismatch".into(),
                    ));
                }
                fields.policy_revision = next_revision;
                fields.record_generation = fields.record_generation.next()?;
                records.push(record);
            }
        }
        let transition = NativeNamespacePolicyUpdated {
            namespace_id: namespace.id,
            lifecycle_version: intent.capture.version,
            previous_revision: intent.capture.revision,
            next_revision,
            previous_generation: state.committed_generation,
            capture_generation: intent.capture.generation,
            before,
            after,
            record_count: records.len() as u64,
            record_token_digest: crate::storage::lifecycle_migration::token_digest_values(
                records.iter().map(|r| {
                    (
                        r.id,
                        r.schema.as_ref().unwrap().lifecycle.record_generation.get(),
                    )
                }),
            ),
        };
        {
            let mut table = tx.open_table(META_TABLE)?;
            for record in &records {
                table.insert(record.id.as_slice(), record.encode()?.as_slice())?;
            }
        }
        tx.open_table(NAMESPACE_TABLE)?
            .insert(namespace.id.get(), control::encode(&namespace)?.as_slice())?;
        let receipt = self.record_receipt(
            &tx,
            ctx,
            hash,
            MutationOutcome::NativeNamespacePolicyUpdated { transition },
        )?;
        self.commit_catalog(tx, &receipt)?;
        Ok(Some(receipt))
    }
}

#[cfg(test)]
#[path = "native_policy_tests.rs"]
mod tests;
