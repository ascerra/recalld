//! Resolved v1 context. Configuration identity is not remote-weight provenance,
//! a namespace attestation, a ReadView or a readiness declaration.
use crate::embedding::{EmbeddingError, EmbeddingProvider};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "identity", rename_all = "camelCase", deny_unknown_fields)]
pub enum ActiveEmbedding {
    Unknown {
        dimension: u32,
    },
    Configuration {
        dimension: u32,
        #[serde(rename = "profileId")]
        profile_id: String,
    },
}
impl ActiveEmbedding {
    pub(crate) fn from_provider(provider: &dyn EmbeddingProvider) -> Result<Self, EmbeddingError> {
        let invalid = || EmbeddingError::Unavailable("active embedding identity is invalid".into());
        let dimension = u32::try_from(provider.dimensions()).map_err(|_| invalid())?;
        if !(1..=8192).contains(&dimension) {
            return Err(invalid());
        }
        match provider.profile() {
            None => Ok(Self::Unknown { dimension }),
            Some(profile) => {
                if profile.dimension != dimension {
                    return Err(invalid());
                }
                let profile_id = profile.profile_id().map_err(|_| invalid())?;
                Ok(Self::Configuration {
                    dimension,
                    profile_id,
                })
            }
        }
    }
    fn validate(&self) -> bool {
        let (dimension, profile) = match self {
            Self::Unknown { dimension } => (*dimension, None),
            Self::Configuration {
                dimension,
                profile_id,
            } => (*dimension, Some(profile_id)),
        };
        (1..=8192).contains(&dimension)
            && profile.is_none_or(|id| {
                id.strip_prefix("ep1:").is_some_and(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendContext {
    context_version: u32,
    pub store_id: Uuid,
    pub active_embedding: ActiveEmbedding,
}
impl BackendContext {
    pub(crate) fn new(
        store_id: Uuid,
        active_embedding: ActiveEmbedding,
    ) -> Result<Self, &'static str> {
        let context = Self {
            context_version: 1,
            store_id,
            active_embedding,
        };
        context.validate()?;
        Ok(context)
    }
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.context_version != 1 || self.store_id.is_nil() || !self.active_embedding.validate()
        {
            return Err("invalid or unsupported backend context");
        }
        Ok(())
    }
}

/// Preserve PR19's default, limit and sampled coverage fields on both surfaces.
pub(crate) fn document(
    requested: &str,
    source: &str,
    resolved: Option<(String, u32, u32)>,
    backend: Option<BackendContext>,
) -> Value {
    use crate::mcp::bridge::{DUPLICATE_NEIGHBOR_CAP, DUPLICATE_SAMPLE_CAP};
    use crate::model::constants::{MAX_BATCH_MEMORIES, MAX_LIST_LIMIT, MAX_TAG_LIMIT};
    let mut value = json!({
        "supportedContractVersions":[1],
        "defaultNamespace":{
            "requestedName":requested,"source":source,"exists":resolved.is_some(),
            "resolved":resolved.map(|(name,id,dimension)|json!({"name":name,"id":id,"embeddingDim":dimension}))
        },
        "namespacesAreAccessControls":false,
        "limits":{"batchMemories":MAX_BATCH_MEMORIES,"listMemories":MAX_LIST_LIMIT,"tagsPerBucket":MAX_TAG_LIMIT},
        "duplicateScan":{"algorithm":"sampledNearestNeighbors","sampleCap":DUPLICATE_SAMPLE_CAP,"neighborCap":DUPLICATE_NEIGHBOR_CAP,"complete":false}
    });
    if let Some(backend) = backend {
        value["backendContext"] = json!(backend);
    } else {
        value["backendContextUnavailable"] = json!(true);
    }
    value
}
