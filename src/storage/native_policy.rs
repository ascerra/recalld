//! Numeric policy replacement on the existing canonical owner. No clock or
//! provider is sampled, and a receipt does not certify derived publication.
use serde::{Deserialize, Serialize};

use super::StorageError;
use super::catalog::NamespacePolicy;
use super::control::{self, Generation, GenerationRef, MutationContext, WriteReceipt};
use super::native_catalog::NamespacePolicyState;
use super::native_lifecycle::{CapturedLifecycle, LifecycleScope, invalid};
use crate::model::{NamespaceConfig, NamespaceId, schema::LifecycleVersion};

#[derive(Debug, Clone)]
pub struct NativePolicyIntent {
    pub(crate) scope: LifecycleScope,
    pub(crate) original: NamespacePolicy,
    pub(crate) capture: CapturedLifecycle,
}
impl NativePolicyIntent {
    pub fn capture_generation(&self) -> GenerationRef {
        self.capture.generation
    }
}

pub(crate) fn policy_state(input: &NamespacePolicy) -> Result<NamespacePolicyState, StorageError> {
    input.validate()?;
    let mut config = NamespaceConfig::default_namespace(0);
    input.apply(&mut config);
    NamespacePolicyState::capture(&config)
}
pub(crate) fn request_hash(
    ctx: &MutationContext,
    input: &NamespacePolicy,
) -> Result<[u8; 32], StorageError> {
    ctx.validate()?;
    if !matches!(ctx.scope, control::MutationScope::Namespace(_)) {
        return Err(StorageError::InvalidMutationContext(
            "native policy requires namespace scope".into(),
        ));
    }
    Ok(control::request_hash(
        ctx,
        "namespace_policy_update_v2",
        &[&control::encode(&policy_state(input)?)?],
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNamespacePolicyUpdated {
    pub namespace_id: NamespaceId,
    pub lifecycle_version: LifecycleVersion,
    pub previous_revision: Generation,
    pub next_revision: Generation,
    pub previous_generation: Generation,
    pub capture_generation: GenerationRef,
    pub before: NamespacePolicyState,
    pub after: NamespacePolicyState,
    /// An explicit-to-inherited transition requires fresh adoption. Preserve the
    /// old binding only in its proof, never reactivate stale inputs implicitly.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::request_context::deserialize_present"
    )]
    pub cleared_inherited_decay: Option<super::inherited_policy::InheritedDecayBinding>,
    /// Returning to inheritance binds the owner's current inherited multiplier
    /// in this same receipt, so dependent operations need no separate adoption.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::request_context::deserialize_present"
    )]
    pub adopted_inherited_decay: Option<super::inherited_policy::InheritedDecayBinding>,
    pub record_count: u64,
    pub record_token_digest: String,
}
/// A keyed no-op binds lifetime retry identity without rewriting row tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNamespacePolicyUnchanged {
    pub namespace_id: NamespaceId,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: Generation,
    pub previous_generation: Generation,
    pub capture_generation: GenerationRef,
    pub policy: NamespacePolicyState,
}
impl NativeNamespacePolicyUnchanged {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.namespace_id.is_unset()
            || receipt.committed_generation != self.previous_generation.next()?
            || self.capture_generation.store_id != receipt.store_id
            || self.capture_generation.generation > self.previous_generation
        {
            return Err(invalid("invalid native policy no-change proof"));
        }
        let mut ns = NamespaceConfig::default_namespace(0);
        self.policy.apply(&mut ns);
        NamespacePolicy::from(&ns).validate()
    }
}

impl NativeNamespacePolicyUpdated {
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.namespace_id.is_unset()
            || self.next_revision != self.previous_revision.next()?
            || receipt.committed_generation != self.previous_generation.next()?
            || self.capture_generation.store_id != receipt.store_id
            || self.capture_generation.generation > self.previous_generation
            || self.before == self.after
            || self.record_token_digest.len() != 64
            || !self
                .record_token_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("invalid native numeric policy proof"));
        }
        if let Some(binding) = &self.cleared_inherited_decay {
            binding.validate()?;
            if self.before.decay_rate_multiplier.is_none()
                || self.after.decay_rate_multiplier.is_some()
            {
                return Err(invalid("invalid inherited binding clearance"));
            }
        }
        if let Some(binding) = &self.adopted_inherited_decay {
            binding.validate()?;
            if self.before.decay_rate_multiplier.is_none()
                || self.after.decay_rate_multiplier.is_some()
                || !binding.matches_receipt(receipt)
            {
                return Err(invalid(
                    "invalid inherited binding on return to inheritance",
                ));
            }
        }
        for state in [&self.before, &self.after] {
            let mut ns = NamespaceConfig::default_namespace(0);
            state.apply(&mut ns);
            NamespacePolicy::from(&ns).validate()?;
        }
        Ok(())
    }
    /// The existing revision-chain validator also handles numeric-only revisions.
    pub(crate) fn revision_transition(&self) -> super::lifecycle_migration::PolicyTransition {
        super::lifecycle_migration::PolicyTransition {
            namespace_id: self.namespace_id,
            from: self.lifecycle_version,
            to: self.lifecycle_version,
            previous_revision: self.previous_revision,
            next_revision: self.next_revision,
            previous_generation: self.previous_generation,
            record_count: self.record_count,
            record_token_digest: self.record_token_digest.clone(),
        }
    }
}

impl super::native_lifecycle::NativeLifecycleSession {
    pub fn capture_policy(
        &self,
        scope: LifecycleScope,
        replacement: &NamespacePolicy,
    ) -> Result<NativePolicyIntent, StorageError> {
        self.metadata()
            .capture_native_policy(scope, replacement, &self.policy, None)
    }
    pub fn apply_policy(
        &mut self,
        ctx: &MutationContext,
        intent: &NativePolicyIntent,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        self.engine
            .apply_native_policy(ctx, intent, &self.policy, None)
    }
    /// Matching replay precedes fresh namespace/policy capture; None is an exact
    /// unkeyed no-op. Keyed no-ops commit their original outcome/generation once.
    pub fn update_policy(
        &mut self,
        ctx: &MutationContext,
        scope: LifecycleScope,
        replacement: &NamespacePolicy,
    ) -> Result<Option<WriteReceipt>, StorageError> {
        let root = self.metadata().work_tracker().begin()?;
        let result = (|| {
            if let Some(receipt) =
                self.metadata()
                    .lookup_native_policy(ctx, replacement, Some(&root))?
            {
                return Ok(Some(receipt));
            }
            let intent = self.metadata().capture_native_policy(
                scope,
                replacement,
                &self.policy,
                Some(&root),
            )?;
            self.engine
                .apply_native_policy(ctx, &intent, &self.policy, Some(&root))
        })();
        self.metadata().finish_work(root);
        result
    }
}
