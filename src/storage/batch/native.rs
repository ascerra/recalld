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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_settings: Option<crate::storage::native_store::NativeStoreSettings>,
}
impl NativeBatchDefaults {
    pub(crate) fn validate(&self, dimension: u32, kind: BatchKind) -> Result<(), StorageError> {
        if let Some(settings) = &self.store_settings {
            settings.validate()?;
        }
        let difficulty = f32::from_bits(self.default_difficulty_bits);
        if !difficulty.is_finite() || !(1.0..=10.0).contains(&difficulty) {
            return Err(StorageError::InvalidControl(
                "invalid native batch difficulty".into(),
            ));
        }
        self.embedding_binding
            .validate(dimension)
            .map_err(StorageError::InvalidControl)?;
        let _ = kind;
        Ok(())
    }
    pub(crate) fn profile_id(&self) -> Result<Option<&str>, StorageError> {
        match &self.embedding_binding {
            EmbeddingBinding::Bound { profile_id, .. } => Ok(Some(profile_id)),
            EmbeddingBinding::UnknownLegacy => Ok(None),
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
        (1 | 4, BatchKind::Insert, MemoryInserted { .. })
            | (1 | 4, BatchKind::Forget, MemoryTombstoned { .. })
            | (2, BatchKind::Insert, NativeMemoryInserted { .. })
            | (2, BatchKind::Forget, NativeMemoryTombstoned { .. })
            | (3, BatchKind::Insert, NativeMemoryInsertedWithLinks { .. })
    )
}

pub(crate) fn validate_outcome(
    defaults: &BatchDefaults,
    outcome: &control::MutationOutcome,
) -> Result<(), StorageError> {
    let Some(native) = &defaults.native else {
        return Ok(());
    };
    if let control::MutationOutcome::NativeMemoryInsertedWithLinks { transition } = outcome {
        let after = &transition.primary()?.after;
        if transition.namespace_id != defaults.namespace_id
            || transition.embedding_dim != defaults.embedding_dim
            || transition.policy_revision != native.policy_revision
            || transition.lifecycle_version != native.lifecycle_version
            || native.store_settings.as_ref() != Some(&transition.settings)
            || after.lifecycle.stability.get().to_bits() != defaults.initial_stability_bits
            || after.lifecycle.difficulty.get().to_bits() != native.default_difficulty_bits
            || after.content.profile_id.as_deref() != native.profile_id()?
        {
            return Err(StorageError::InvalidControl(
                "linked child disagrees with frozen defaults".into(),
            ));
        }
        return Ok(());
    }
    if native.store_settings.is_some() {
        return Err(StorageError::InvalidControl(
            "old child contains linked-store defaults".into(),
        ));
    }
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
                || after.content.profile_id.as_deref() != native.profile_id()?)
    {
        return Err(StorageError::InvalidControl(
            "native child disagrees with frozen defaults".into(),
        ));
    }
    Ok(())
}

pub(crate) fn store_hash(
    ctx: &MutationContext,
    input: &crate::storage::native_store::NativeStoreInput,
    defaults: &BatchDefaults,
) -> Result<[u8; 32], StorageError> {
    let native = defaults
        .native
        .as_ref()
        .ok_or_else(|| StorageError::InvalidControl("native store defaults missing".into()))?;
    let settings = native
        .store_settings
        .as_ref()
        .ok_or_else(|| StorageError::InvalidControl("native store settings missing".into()))?;
    input.request_hash(
        ctx,
        f32::from_bits(defaults.initial_stability_bits),
        f32::from_bits(native.default_difficulty_bits),
        defaults.embedding_dim,
        native.profile_id()?,
        settings,
    )
}

/// Fresh callers pass defaults captured in the reservation transaction; resumed
/// children pass the immutable manifest defaults. Never consult a provider or
/// the current namespace while resolving a retained child.
pub(crate) fn resolve_store_profile(
    input: &crate::storage::native_store::NativeStoreInput,
    supplied_profile_id: Option<&str>,
    defaults: &BatchDefaults,
) -> Result<crate::storage::native_store::NativeStoreInput, StorageError> {
    let mut resolved = input.clone();
    if input.insert.memory.embedding.is_some()
        && supplied_profile_id.is_none()
        && input.insert.embedding_profile.is_none()
        && defaults
            .native
            .as_ref()
            .is_none_or(|n| !matches!(n.embedding_binding, EmbeddingBinding::UnknownLegacy))
    {
        return Err(StorageError::EmbeddingProfileRequired);
    }
    if let Some(requested) = supplied_profile_id {
        if input.insert.embedding_profile.is_some()
            || input.insert.memory.embedding.is_none()
            || !crate::embedding::profile::valid_profile_id(requested)
        {
            return Err(StorageError::InvalidMutation(
                "supplied vector requires only a valid profile ID".into(),
            ));
        }
        let native = defaults
            .native
            .as_ref()
            .ok_or_else(|| StorageError::InvalidControl("native batch defaults missing".into()))?;
        native.validate(defaults.embedding_dim, BatchKind::Insert)?;
        let EmbeddingBinding::Bound {
            profile_id,
            descriptor,
            ..
        } = &native.embedding_binding
        else {
            return Err(StorageError::InvalidMutation(
                "unknown native profile provenance".into(),
            ));
        };
        if requested != profile_id {
            return Err(StorageError::EmbeddingProfileMismatch);
        }
        resolved.insert.embedding_profile = Some(descriptor.clone());
    }
    resolved.insert.validate()?;
    Ok(resolved)
}
