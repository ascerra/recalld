//! Shared application-wire preparation. Only v1 is admitted by live transports.
//! These value types are not a ReadView, readiness proof, or native writer API.
use crate::storage::control::{
    Generation, GenerationRef, MutationContext, MutationScope, WriteReceipt,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

pub const VERSION_HEADER: &str = "recalld-contract-version";
pub const KEY_HEADER: &str = "idempotency-key";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code} at {field}: {message}")]
pub struct InputError {
    pub code: &'static str,
    pub field: String,
    pub message: &'static str,
}
fn invalid(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: field.into(),
        message,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractVersion {
    V1,
    V2,
}
impl ContractVersion {
    pub fn from_mcp(arguments: &Value) -> Result<Self, InputError> {
        let object = arguments
            .as_object()
            .ok_or_else(|| invalid("INVALID_TYPE", "", "arguments must be an object"))?;
        match object.get("contractVersion") {
            None => Ok(Self::V1),
            Some(value) => match value.as_u64() {
                Some(1) => Ok(Self::V1),
                Some(2) => Ok(Self::V2),
                Some(_) => Err(invalid(
                    "UNSUPPORTED_CONTRACT_VERSION",
                    "/contractVersion",
                    "unsupported application contract version",
                )),
                None => Err(invalid(
                    "INVALID_TYPE",
                    "/contractVersion",
                    "contractVersion must be an integer",
                )),
            },
        }
    }
    pub fn from_http(value: Option<&str>) -> Result<Self, InputError> {
        match value {
            None | Some("1") => Ok(Self::V1),
            Some("2") => Ok(Self::V2),
            Some(_) => Err(invalid(
                "UNSUPPORTED_CONTRACT_VERSION",
                "/contractVersion",
                "contract version header must be 1 or 2",
            )),
        }
    }
    /// Deliberately not controlled by a configurable boolean. Activation needs
    /// the actual writer/read-view interfaces and their transport conformance.
    pub fn require_supported(self) -> Result<(), InputError> {
        match self {
            Self::V1 => Ok(()),
            Self::V2 => Err(invalid(
                "UNSUPPORTED_CONTRACT_VERSION",
                "/contractVersion",
                "contractVersion 2 is not supported by this server",
            )),
        }
    }
    pub fn number(self) -> u32 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }
}

/// Strict v2 object validation; callers pass the published operation's fields.
/// V1 paths do not call this helper and retain unknown-field tolerance.
pub fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), InputError> {
    if let Some(name) = object.keys().find(|name| !allowed.contains(&name.as_str())) {
        let pointer = format!("/{}", name.replace('~', "~0").replace('/', "~1"));
        return Err(invalid(
            "UNKNOWN_FIELD",
            &pointer,
            "field is not supported by this operation",
        ));
    }
    Ok(())
}

/// Pure retry-option parser. No key or CAS support is exposed by this prep slice.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteOptions {
    pub store_id: Option<Uuid>,
    pub idempotency_key: Option<String>,
    pub expected_generation: Option<Generation>,
}
impl WriteOptions {
    pub fn parse(
        object: &Map<String, Value>,
        header_key: Option<&str>,
    ) -> Result<Self, InputError> {
        let store_id = object
            .get("storeId")
            .map(|value| {
                let text = value.as_str().ok_or_else(|| {
                    invalid("INVALID_TYPE", "/storeId", "storeId must be a UUID string")
                })?;
                Uuid::parse_str(text)
                    .ok()
                    .filter(|id| !id.is_nil())
                    .ok_or_else(|| {
                        invalid("INVALID_TYPE", "/storeId", "storeId must be a non-nil UUID")
                    })
            })
            .transpose()?;
        let body_key = object
            .get("idempotencyKey")
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    invalid(
                        "INVALID_TYPE",
                        "/idempotencyKey",
                        "idempotencyKey must be a string",
                    )
                })
            })
            .transpose()?;
        if let (Some(body), Some(header)) = (body_key, header_key) {
            if body != header {
                return Err(invalid(
                    "INVALID_COMBINATION",
                    "/idempotencyKey",
                    "body and header idempotency keys must agree",
                ));
            }
        }
        let idempotency_key = body_key.or(header_key).map(str::to_owned);
        let expected_generation = object.get("expectedGeneration").map(|value| {
            if !value.is_string() {
                return Err(invalid("INVALID_TYPE", "/expectedGeneration", "expectedGeneration must be a decimal string"));
            }
            serde_json::from_value::<Generation>(value.clone())
                .map_err(|_| invalid("OUT_OF_RANGE", "/expectedGeneration", "expectedGeneration must be a canonical decimal string in 0..9223372036854775807"))
        }).transpose()?;
        let options = Self {
            store_id,
            idempotency_key,
            expected_generation,
        };
        // Reuse A1's key/lineage invariants without reserving a receipt or CAS.
        let context = options.context(MutationScope::Catalog, ContractVersion::V2);
        context.validate().map_err(|_| {
            if options.store_id.is_none() {
                invalid(
                    "INVALID_COMBINATION",
                    "/storeId",
                    "storeId is required with an idempotency key or expectedGeneration",
                )
            } else {
                invalid(
                    "OUT_OF_RANGE",
                    "/idempotencyKey",
                    "idempotencyKey must contain 1–128 printable ASCII bytes",
                )
            }
        })?;
        Ok(options)
    }
    /// The caller must resolve immutable scope before handing this to A1.
    /// A1 owns replay, request hashing and CAS; this performs no backend work.
    pub fn context(&self, scope: MutationScope, version: ContractVersion) -> MutationContext {
        MutationContext {
            store_id: self.store_id,
            scope,
            idempotency_key: self.idempotency_key.clone(),
            expected_generation: self.expected_generation,
            contract_version: version.number(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommitState {
    NotCommitted,
    Committed,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    InvalidInput,
    NotFound,
    Conflict,
    Unavailable,
    Partial,
    Internal,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionError {
    pub category: ErrorCategory,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub retryable: bool,
    pub commit_state: CommitState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<Uuid>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Scope {
    Namespace {
        #[serde(rename = "namespaceId")]
        namespace_id: crate::model::NamespaceId,
        namespace: String,
    },
    Catalog {
        kind: CatalogKind,
    },
    Namespaces {
        #[serde(rename = "namespaceIds")]
        namespace_ids: Vec<crate::model::NamespaceId>,
    },
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogKind {
    Catalog,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ok,
    Partial,
    Error,
}

/// Wire representation only. Populating this requires A1's actual readiness
/// interface; no conversion from an epoch, metadata lease, or latest counter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub store_id: Uuid,
    pub committed_generation: Generation,
    pub ready: bool,
    pub pending_work: usize,
    pub derived: Vec<DerivedReadiness>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedReadiness {
    pub name: String,
    pub applied_generation: Generation,
    pub ready: bool,
}

/// Not returned by live handlers yet. A read generation must eventually come
/// from the coherent ReadView covering selection and hydration, not a fresh read
/// of the control table. Unknown read generations are not synthesized here.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T> {
    contract_version: u32,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<WriteReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_generation: Option<GenerationRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Readiness>,
    pub warnings: Vec<String>,
}
impl<T> Envelope<T> {
    pub fn success(scope: Scope, data: T) -> Self {
        Self {
            contract_version: 2,
            status: Status::Ok,
            scope: Some(scope),
            data: Some(data),
            receipt: None,
            read_generation: None,
            error: None,
            readiness: None,
            warnings: vec![],
        }
    }
}

/// Shared batch classification only, not an executor/manifest implementation.
/// Committed-but-unready items use Ok{ready:false} and count as succeeded.
#[derive(Debug, Clone, Copy)]
pub enum ItemState {
    Ok { ready: bool },
    Error,
    NotAttempted,
    Unknown,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCounts {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub unresolved: usize,
}
impl BatchCounts {
    pub fn classify(items: &[ItemState]) -> Result<(Self, Status, u16), InputError> {
        if items.is_empty() {
            return Err(invalid(
                "OUT_OF_RANGE",
                "/memories",
                "batch must not be empty",
            ));
        }
        let counts = Self {
            total: items.len(),
            succeeded: items
                .iter()
                .filter(|s| matches!(s, ItemState::Ok { .. }))
                .count(),
            failed: items
                .iter()
                .filter(|s| matches!(s, ItemState::Error))
                .count(),
            unresolved: items
                .iter()
                .filter(|s| matches!(s, ItemState::Unknown | ItemState::NotAttempted))
                .count(),
        };
        let (status, http) = if items
            .iter()
            .all(|s| matches!(s, ItemState::Ok { ready: true }))
        {
            (Status::Ok, 200)
        } else if counts.succeeded > 0 {
            (Status::Partial, 207)
        } else if counts.unresolved > 0 {
            (Status::Error, 503)
        } else {
            (Status::Error, 422)
        };
        Ok((counts, status, http))
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
