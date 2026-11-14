//! Native catalog participation on the existing owning engine. Internal native
//! entry does not activate ordinary schema-v2 serving or certify derived views.
use serde::{Deserialize, Serialize};

use super::control::{self, Generation, MutationContext, MutationScope, WriteReceipt};
use super::native_lifecycle::{FiniteF32, invalid};
use super::{StorageEngine, StorageError};
use crate::model::schema::{
    BindingProvenance, EmbeddingBinding, EmbeddingProfile, NamespaceSchemaV2,
};
use crate::model::{NamespaceConfig, NamespaceId};

/// Original caller selectors. A profile ID selects the active configuration;
/// callers cannot supply a descriptor as evidence of the active provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNamespaceInput {
    pub name: String,
    pub embedding_dim: Option<u32>,
    pub embedding_profile_id: Option<String>,
    pub initial_stability: Option<f32>,
    pub desired_retention: Option<f32>,
    pub decay_rate_multiplier: Option<f32>,
}

impl NativeNamespaceInput {
    fn policy_config(&self) -> NamespaceConfig {
        let mut config = NamespaceConfig::default_namespace(0);
        config.name = self.name.clone();
        if let Some(value) = self.initial_stability {
            config.initial_stability = value;
        }
        if let Some(value) = self.desired_retention {
            config.desired_retention = value;
        }
        config.decay_rate_multiplier = self.decay_rate_multiplier;
        config
    }

    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        crate::model::validation::validate_namespace_name(&self.name).map_err(|e| {
            StorageError::InvalidNamespaceName {
                name: self.name.clone(),
                reason: e.to_string(),
            }
        })?;
        if self.embedding_dim.is_some_and(|d| !(1..=8192).contains(&d)) {
            return Err(StorageError::InvalidMutation(
                "embeddingDim outside 1..8192".into(),
            ));
        }
        if self.embedding_profile_id.as_ref().is_some_and(|id| {
            id.len() != 68
                || !id.starts_with("ep1:")
                || !id.as_bytes()[4..]
                    .iter()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
        }) {
            return Err(StorageError::InvalidMutation(
                "invalid embeddingProfileId".into(),
            ));
        }
        super::catalog::NamespacePolicy::from(&self.policy_config()).validate()
    }

    pub(crate) fn hash(&self, ctx: &MutationContext) -> Result<[u8; 32], StorageError> {
        self.validate()?;
        // Fixed v2 numeric defaults are expanded here. Generated ID/time and
        // sampled provider identity/dimension are outcomes, not caller intent.
        let fields =
            control::encode(&(self, NamespacePolicyState::capture(&self.policy_config())?))?;
        Ok(control::request_hash(
            ctx,
            "namespace_create_v2",
            &[&fields],
        ))
    }

    pub(crate) fn prepare(
        &self,
        profile: &EmbeddingProfile,
        created_at: i64,
    ) -> Result<NamespaceConfig, StorageError> {
        self.validate()?;
        let profile_id = profile
            .profile_id()
            .map_err(StorageError::InvalidMutation)?;
        if self.embedding_dim.is_some_and(|d| d != profile.dimension)
            || self
                .embedding_profile_id
                .as_ref()
                .is_some_and(|id| *id != profile_id)
        {
            return Err(StorageError::InvalidMutation(
                "active embedding profile mismatch".into(),
            ));
        }
        if chrono::DateTime::from_timestamp_millis(created_at).is_none() {
            return Err(StorageError::InvalidMutation(
                "invalid creation timestamp".into(),
            ));
        }
        let mut config = self.policy_config();
        config.created_at = created_at;
        config.embedding_dim = profile.dimension;
        let mut schema = NamespaceSchemaV2::from_legacy();
        schema.embedding_binding = EmbeddingBinding::Bound {
            profile_id,
            descriptor: profile.clone(),
            provenance: BindingProvenance::NativeV2,
        };
        schema
            .validate(&config)
            .map_err(StorageError::InvalidMutation)?;
        config.schema = Some(schema);
        Ok(config)
    }
}

/// Only mutable numeric policy fields, with finite validated receipt encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamespacePolicyState {
    initial_stability: FiniteF32,
    default_difficulty: FiniteF32,
    thresholds: [FiniteF32; 3],
    permastore_threshold: FiniteF32,
    desired_retention: FiniteF32,
    decay_rate_multiplier: Option<FiniteF32>,
}
impl NamespacePolicyState {
    pub(crate) fn capture(ns: &NamespaceConfig) -> Result<Self, StorageError> {
        fn finite(v: f32) -> Result<FiniteF32, StorageError> {
            v.try_into().map_err(StorageError::InvalidMutation)
        }
        Ok(Self {
            initial_stability: finite(ns.initial_stability)?,
            default_difficulty: finite(ns.default_difficulty)?,
            thresholds: [
                finite(ns.phase_thresholds.full_to_summary)?,
                finite(ns.phase_thresholds.summary_to_ghost)?,
                finite(ns.phase_thresholds.ghost_to_delete)?,
            ],
            permastore_threshold: finite(ns.permastore_threshold)?,
            desired_retention: finite(ns.desired_retention)?,
            decay_rate_multiplier: ns.decay_rate_multiplier.map(finite).transpose()?,
        })
    }
    pub(crate) fn apply(&self, ns: &mut NamespaceConfig) {
        ns.initial_stability = self.initial_stability.get();
        ns.default_difficulty = self.default_difficulty.get();
        ns.phase_thresholds = crate::model::namespace::PhaseThresholds {
            full_to_summary: self.thresholds[0].get(),
            summary_to_ghost: self.thresholds[1].get(),
            ghost_to_delete: self.thresholds[2].get(),
        };
        ns.permastore_threshold = self.permastore_threshold.get();
        ns.desired_retention = self.desired_retention.get();
        ns.decay_rate_multiplier = self.decay_rate_multiplier.map(FiniteF32::get);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNamespaceCreated {
    pub namespace_id: NamespaceId,
    pub previous_max_id: u32,
    pub previous_generation: Generation,
    pub name: String,
    pub created_at: i64,
    pub profile: EmbeddingProfile,
    pub policy: NamespacePolicyState,
}
impl NativeNamespaceCreated {
    pub(crate) fn namespace(&self) -> Result<NamespaceConfig, StorageError> {
        let input = NativeNamespaceInput {
            name: self.name.clone(),
            embedding_dim: Some(self.profile.dimension),
            embedding_profile_id: None,
            initial_stability: None,
            desired_retention: None,
            decay_rate_multiplier: None,
        };
        let mut ns = input.prepare(&self.profile, self.created_at)?;
        ns.id = self.namespace_id;
        self.policy.apply(&mut ns);
        super::catalog::NamespacePolicy::from(&ns).validate()?;
        ns.schema
            .as_ref()
            .unwrap()
            .validate(&ns)
            .map_err(StorageError::InvalidControl)?;
        Ok(ns)
    }
    pub(crate) fn validate(&self, receipt: &WriteReceipt) -> Result<(), StorageError> {
        if self.namespace_id.is_unset()
            || self.previous_max_id.checked_add(1) != Some(self.namespace_id.get())
            || self.previous_generation.next()? != receipt.committed_generation
        {
            return Err(invalid("invalid native namespace creation boundary"));
        }
        self.namespace()?;
        Ok(())
    }
    pub(crate) fn matches_namespace(&self, ns: &NamespaceConfig) -> Result<bool, StorageError> {
        let expected = self.namespace()?;
        let mut current = ns.clone();
        let schema = current
            .schema
            .as_mut()
            .ok_or_else(|| invalid("native namespace schema missing"))?;
        // The existing explicit lifecycle migration proves these two fields.
        schema.lifecycle_version = crate::model::schema::LifecycleVersion::LegacyAccessV1;
        schema.policy_revision = Generation::default();
        Ok(control::encode(&current)? == control::encode(&expected)?)
    }
}

impl super::native_lifecycle::NativeLifecycleSession {
    /// Actual provider configuration is sampled only after historical replay.
    /// This binds configuration identity, never claims to verify remote weights.
    pub fn create_namespace(
        &mut self,
        ctx: &MutationContext,
        input: &NativeNamespaceInput,
        provider: &dyn crate::embedding::EmbeddingProvider,
    ) -> Result<WriteReceipt, StorageError> {
        if let Some(receipt) = self
            .engine
            .meta_store()
            .lookup_native_namespace_replay(ctx, input)?
        {
            return Ok(receipt);
        }
        let profile = provider.profile().ok_or_else(|| {
            StorageError::InvalidMutation("active embedding profile is unknown".into())
        })?;
        if provider.dimensions() != profile.dimension as usize {
            return Err(StorageError::InvalidMutation(
                "active provider dimension mismatch".into(),
            ));
        }
        self.engine
            .create_native_namespace(ctx, input, &profile, self.clock.as_ref(), None)
    }
}

#[cfg(test)]
#[path = "native_catalog_tests.rs"]
mod tests;
