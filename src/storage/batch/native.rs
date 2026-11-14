//! Version-two batch identity. This does not change historical single-write or
//! v1 batch hashes. Mutable defaults are frozen once, before any child runs.
use super::*;
use crate::model::schema::{EmbeddingBinding, LifecycleVersion};
use crate::storage::{StorageError, control, native_records::NativeInsert};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NativeBatchDefaults {
    pub default_difficulty_bits: u32,
    pub lifecycle_version: LifecycleVersion,
    pub policy_revision: control::Generation,
    pub embedding_binding: EmbeddingBinding,
}
impl NativeBatchDefaults {
    pub(crate) fn validate(&self, dimension: u32) -> Result<(), StorageError> {
        let difficulty = f32::from_bits(self.default_difficulty_bits);
        if !difficulty.is_finite() || !(1.0..=10.0).contains(&difficulty) {
            return Err(StorageError::InvalidControl(
                "invalid native batch difficulty".into(),
            ));
        }
        self.embedding_binding
            .validate(dimension)
            .map_err(StorageError::InvalidControl)?;
        self.profile_id()?;
        Ok(())
    }
    pub(crate) fn profile_id(&self) -> Result<&str, StorageError> {
        match &self.embedding_binding {
            EmbeddingBinding::Bound { profile_id, .. } => Ok(profile_id),
            EmbeddingBinding::UnknownLegacy => Err(StorageError::InvalidMutation(
                "native batch needs a bound profile".into(),
            )),
        }
    }
}

/// Expanded numeric values appear once. IDs/times are caller options (None is
/// server-generated); provider output never participates. Supplied vectors keep
/// their original bits even when normalization yields an identical stored vector.
pub(crate) fn insert_hash(
    ctx: &MutationContext,
    input: &NativeInsert,
    defaults: &BatchDefaults,
) -> Result<[u8; 32], StorageError> {
    let native = defaults
        .native
        .as_ref()
        .ok_or_else(|| StorageError::InvalidControl("native batch defaults missing".into()))?;
    let m = &input.memory;
    let fields = control::encode(&(
        m.id,
        &m.summary,
        &m.full_text,
        &m.tags,
        m.initial_stability
            .unwrap_or(f32::from_bits(defaults.initial_stability_bits))
            .to_bits(),
        native.default_difficulty_bits,
        defaults.embedding_dim,
        m.created_at,
        input.observed_at,
        m.embedding
            .as_ref()
            .map(|v| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>()),
        m.parent_id,
        m.supersedes,
        (&input.entities, &input.topics, &input.emotions),
        input
            .embedding_profile
            .as_ref()
            .map(|p| p.profile_id())
            .transpose()
            .map_err(StorageError::InvalidMutation)?,
        native.profile_id()?,
        (native.lifecycle_version, native.policy_revision),
    ))?;
    Ok(control::request_hash(
        ctx,
        "batch_native_insert_v2",
        &[&fields],
    ))
}

pub(crate) fn child_matches(
    version: u32,
    kind: BatchKind,
    outcome: &control::MutationOutcome,
) -> bool {
    use control::MutationOutcome::*;
    matches!(
        (version, kind, outcome),
        (1, BatchKind::Insert, MemoryInserted { .. })
            | (1, BatchKind::Forget, MemoryTombstoned { .. })
            | (2, BatchKind::Insert, NativeMemoryInserted { .. })
            | (2, BatchKind::Forget, NativeMemoryTombstoned { .. })
    )
}

pub(crate) fn validate_outcome(
    defaults: &BatchDefaults,
    outcome: &control::MutationOutcome,
) -> Result<(), StorageError> {
    let Some(native) = &defaults.native else {
        return Ok(());
    };
    let (transition, inserted) = match outcome {
        control::MutationOutcome::NativeMemoryInserted { transition } => (transition, true),
        control::MutationOutcome::NativeMemoryTombstoned { transition } => (transition, false),
        _ => {
            return Err(StorageError::InvalidControl(
                "non-native batch child".into(),
            ));
        }
    };
    let after = &transition.primary()?.after;
    if transition.namespace_id != defaults.namespace_id
        || transition.embedding_dim != defaults.embedding_dim
        || transition.policy_revision != native.policy_revision
        || transition.lifecycle_version != native.lifecycle_version
        || inserted
            && (after.lifecycle.stability.get().to_bits() != defaults.initial_stability_bits
                || after.lifecycle.difficulty.get().to_bits() != native.default_difficulty_bits
                || after.content.profile_id.as_deref() != Some(native.profile_id()?))
    {
        return Err(StorageError::InvalidControl(
            "native child disagrees with frozen defaults".into(),
        ));
    }
    Ok(())
}
