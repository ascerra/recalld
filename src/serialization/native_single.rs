//! Prepared explicit2 single insertion. Does not activate public contract2.
use super::contract::*;
use crate::{mcp::bridge::StoredMemory, model::NamespaceId, storage::control::WriteReceipt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Request {
    fields: Value,
    http: bool,
    default_namespace: String,
}
pub(crate) struct Parsed {
    pub options: WriteOptions,
    pub namespace_id: Option<NamespaceId>,
    pub asserted_name: Option<String>,
    pub store: crate::mcp::bridge::StoreInput,
    pub input: crate::storage::native_store::NativeStoreInput,
    pub profile_id: Option<String>,
}
fn invalid(field: &str, message: &'static str) -> InputError {
    InputError {
        code: "INVALID_INPUT",
        field: field.into(),
        message,
    }
}
impl Request {
    pub(crate) fn parse(
        value: &Value,
        http: bool,
        key: Option<&str>,
        default_namespace: &str,
    ) -> Result<Self, InputError> {
        let mut fields = value.clone();
        let object = fields
            .as_object_mut()
            .ok_or_else(|| invalid("", "expected an object"))?;
        // Resolve the header once without changing memory fields or vector bits.
        let options = WriteOptions::parse(object, key)?;
        if let Some(key) = options.idempotency_key {
            object.insert("idempotencyKey".into(), Value::String(key));
        }
        let out = Self {
            fields,
            http,
            default_namespace: default_namespace.into(),
        };
        out.prepare()?;
        Ok(out)
    }
    pub(crate) fn prepare(&self) -> Result<Parsed, InputError> {
        let object = self
            .fields
            .as_object()
            .ok_or_else(|| invalid("", "expected an object"))?;
        let mut allowed = vec![
            "summary",
            "fullText",
            "tags",
            "entities",
            "topics",
            "emotions",
            "namespace",
            "namespaceId",
            "parentId",
            "supersedes",
            "checkDuplicates",
            "embedding",
            "embeddingProfileId",
            "storeId",
            "idempotencyKey",
            "expectedGeneration",
        ];
        if self.http {
            allowed.extend(["initialStability", "createdAt"]);
        } else {
            allowed.push("contractVersion");
            if ContractVersion::from_mcp(&self.fields)? != ContractVersion::V2 {
                return Err(invalid(
                    "/contractVersion",
                    "explicit contractVersion 2 required",
                ));
            }
        }
        reject_unknown_fields(object, &allowed)?;
        let options = WriteOptions::parse(object, None)?;
        let namespace_id = object
            .get("namespaceId")
            .map(|v| {
                v.as_u64()
                    .filter(|id| (1..=u64::from(u32::MAX)).contains(id))
                    .map(|id| NamespaceId::new(id as u32))
                    .ok_or_else(|| {
                        invalid("/namespaceId", "namespaceId must be a positive u32 integer")
                    })
            })
            .transpose()?;
        let mut store =
            crate::mcp::args::parse_store_input_fields(&self.fields, &self.default_namespace)
                .map_err(|e| invalid(&format!("/{}", e.field), "invalid memory field"))?;
        let asserted_name = object
            .get("namespace")
            .filter(|v| !v.is_null())
            .map(|_| store.namespace.clone());
        let profile_id = object
            .get("embeddingProfileId")
            .map(|v| {
                v.as_str()
                    .filter(|id| crate::embedding::profile::valid_profile_id(id))
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("/embeddingProfileId", "expected ep1 profile ID"))
            })
            .transpose()?;
        if let Some(value) = object.get("embedding") {
            let values = value.as_array().ok_or_else(|| InputError {
                code: "INVALID_TYPE",
                field: "/embedding".into(),
                message: "embedding must be an array",
            })?;
            if !(1..=8192).contains(&values.len()) {
                return Err(InputError {
                    code: "OUT_OF_RANGE",
                    field: "/embedding".into(),
                    message: "embedding must contain 1–8192 values",
                });
            }
            let vector: Vec<f32> = serde_json::from_value(value.clone())
                .map_err(|_| invalid("/embedding", "embedding must be a finite f32 array"))?;
            let mut normalized = vector.clone();
            let dimension = normalized.len();
            crate::embedding::validation::normalize_vector(&mut normalized, dimension).map_err(
                |_| InputError {
                    code: "OUT_OF_RANGE",
                    field: "/embedding".into(),
                    message: "embedding must contain finite values with nonzero norm",
                },
            )?;
            store.embedding = Some(vector); // Original bits, not the validation copy.
        }
        if store.embedding.is_some() != profile_id.is_some() {
            return Err(invalid(
                "/embeddingProfileId",
                "embedding and embeddingProfileId must be supplied together",
            ));
        }
        if let Some(value) = object.get("initialStability") {
            let stability = value
                .as_f64()
                .filter(|n| n.is_finite() && (*n as f32) > 0. && (*n as f32).is_finite())
                .ok_or_else(|| {
                    invalid(
                        "/initialStability",
                        "initialStability must be finite and positive",
                    )
                })?;
            store.initial_stability = Some(stability as f32);
        }
        let mut insert = crate::mcp::batches::native_fields(&store, &self.fields)
            .map_err(|_| invalid("", "invalid native memory fields"))?;
        if let Some(value) = object.get("createdAt") {
            let millis = value
                .as_i64()
                .filter(|v| chrono::DateTime::from_timestamp_millis(*v).is_some())
                .ok_or_else(|| {
                    invalid(
                        "/createdAt",
                        "createdAt must be representable integer epoch milliseconds",
                    )
                })?;
            insert.memory.created_at = Some(millis);
        }
        Ok(Parsed {
            options,
            namespace_id,
            asserted_name,
            profile_id,
            input: crate::storage::native_store::NativeStoreInput {
                insert,
                check_duplicates: store.check_duplicates.unwrap_or(true),
            },
            store,
        })
    }
}

/// Full typed reply also used by the dedicated non-retrying daemon operation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Reply {
    pub namespace_id: Option<NamespaceId>,
    pub namespace: Option<String>,
    pub data: Option<StoredMemory>,
    pub receipt: Option<WriteReceipt>,
    pub readiness: Option<Readiness>,
    pub read_generation: Option<crate::storage::control::GenerationRef>,
    pub error: Option<ExecutionError>,
    pub warnings: Vec<String>,
}
impl Reply {
    pub(crate) fn uncertain() -> Self {
        let mut out = Self::failure(crate::storage::StorageError::RecoveryRequired(
            "native single reply unavailable; reconcile original request and key".into(),
        ));
        if let Some(error) = &mut out.error {
            error.code = "COMMIT_OUTCOME_UNKNOWN".into();
            error.commit_state = CommitState::Unknown;
            error.message =
                "native single reply unavailable; reconcile the original request and key".into();
        }
        out
    }
    pub(crate) fn failure(error: crate::storage::StorageError) -> Self {
        let uncertain = matches!(&error, crate::storage::StorageError::RecoveryRequired(_));
        let source = match &error {
            crate::storage::StorageError::CommittedMutation { source, .. } => source.as_ref(),
            other => other,
        };
        let specific = match source {
            crate::storage::StorageError::EmbeddingProfileMismatch => Some((
                ErrorCategory::Conflict,
                "EMBEDDING_PROFILE_MISMATCH",
                "/embeddingProfileId",
                "supplied profile ID differs from canonical binding",
            )),
            crate::storage::StorageError::MutationNamespaceMismatch => Some((
                ErrorCategory::Conflict,
                "NAMESPACE_MISMATCH",
                "/namespace",
                "namespace assertion differs from canonical binding",
            )),
            crate::storage::StorageError::NamespaceNotFound(_)
            | crate::storage::StorageError::NamespaceNameNotFound(_) => Some((
                ErrorCategory::NotFound,
                "NAMESPACE_NOT_FOUND",
                "/namespace",
                "namespace does not exist",
            )),
            crate::storage::StorageError::BatchResponseTooLarge {
                reservation_receipt: None,
            } => Some((
                ErrorCategory::InvalidInput,
                "RESPONSE_TOO_LARGE",
                "",
                "single worst-case full receipt exceeds transport budget; no write performed",
            )),
            _ => None,
        };
        let mut failure = super::native_batch::BatchFailure::from_storage(error);
        if failure.error.code == "STORAGE_UNAVAILABLE" {
            failure.error.message = "native single consumer unavailable".into();
        }
        if uncertain {
            failure.error.code = "COMMIT_OUTCOME_UNKNOWN".into();
            failure.error.commit_state = CommitState::Unknown;
            failure.error.message = "native single outcome unavailable; recover and reconcile the original request and key".into();
        }
        if let Some((category, code, field, message)) = specific {
            failure.error.category = category;
            failure.error.code = code.into();
            failure.error.field = (!field.is_empty()).then(|| field.into());
            failure.error.message = message.into();
        }
        Self {
            namespace_id: None,
            namespace: None,
            data: None,
            receipt: failure.receipt,
            readiness: failure.readiness,
            read_generation: None,
            error: Some(failure.error),
            warnings: failure.warnings,
        }
    }
    pub(crate) fn invalid(error: InputError) -> Self {
        Self {
            namespace_id: None,
            namespace: None,
            data: None,
            receipt: None,
            readiness: None,
            read_generation: None,
            error: Some(ExecutionError {
                category: ErrorCategory::InvalidInput,
                code: error.code.into(),
                message: error.message.into(),
                field: Some(error.field),
                retryable: false,
                commit_state: CommitState::NotCommitted,
                operation_id: None,
            }),
            warnings: vec![],
        }
    }
    pub(crate) fn response(self) -> (u16, Envelope<StoredMemory>) {
        let scope = self
            .namespace_id
            .zip(self.namespace)
            .map(|(namespace_id, namespace)| Scope::Namespace {
                namespace_id,
                namespace,
            });
        let (status, mut envelope) = if let Some(error) = self.error {
            let status = match error.category {
                ErrorCategory::InvalidInput
                    if error.code == "OUT_OF_RANGE"
                        && error.field.as_deref() == Some("/embedding") =>
                {
                    422
                }
                ErrorCategory::InvalidInput => 400,
                ErrorCategory::NotFound => 404,
                ErrorCategory::Conflict => 409,
                ErrorCategory::Unavailable => 503,
                _ => 500,
            };
            (status, Envelope::failure(scope, error))
        } else {
            match (scope, self.data) {
                (Some(scope), Some(data)) => (200, Envelope::success(scope, data)),
                (scope, _) => (
                    500,
                    Envelope::failure(
                        scope,
                        ExecutionError {
                            category: ErrorCategory::Internal,
                            code: "INVALID_RESPONSE".into(),
                            message: "single reply lacks canonical identity or result".into(),
                            field: None,
                            retryable: false,
                            commit_state: if self.receipt.is_some() {
                                CommitState::Committed
                            } else {
                                CommitState::Unknown
                            },
                            operation_id: self.receipt.as_ref().map(|receipt| receipt.operation_id),
                        },
                    ),
                ),
            }
        };
        envelope.receipt = self.receipt;
        envelope.readiness = self.readiness;
        envelope.read_generation = self.read_generation;
        envelope.warnings = self.warnings;
        (status, envelope)
    }
}
pub(crate) fn input_schema(http: bool) -> Value {
    use serde_json::json;
    let batch = super::native_batch::input_schema();
    let mut schema = batch["properties"]["memories"]["items"].clone();
    for name in ["storeId", "idempotencyKey", "expectedGeneration"] {
        schema["properties"][name] = batch["properties"][name].clone();
    }
    schema["properties"]["namespaceId"] = json!({"type":"integer","minimum":1,"maximum":u32::MAX,"description":"Immutable namespace ID, including retained binding on replay. Optional namespace is an assertion when an ID is supplied."});
    if http {
        schema["properties"]["createdAt"] =
            json!({"type":"integer","description":"Representable epoch milliseconds."});
        schema["properties"]["initialStability"] = json!({"type":"number","exclusiveMinimum":0});
    } else {
        schema["required"] = json!(["contractVersion", "summary"]);
        schema["properties"]["contractVersion"] = json!({"type":"integer","const":2});
    }
    schema
}
pub(crate) fn mcp_result(reply: Reply) -> crate::mcp::protocol::ToolCallResult {
    let (status, envelope) = reply.response();
    let text = serde_json::to_string(&envelope).expect("finite single envelope");
    let Value::Object(object) = serde_json::from_str(&text).expect("single JSON") else {
        unreachable!()
    };
    let mut result = crate::mcp::protocol::ToolCallResult::text(text);
    result.structured_content = Some(object);
    if status != 200 {
        result.is_error = Some(true);
    }
    result
}

/// Count compact JSON and its escaped MCP text copy without allocating either.
/// The fixed allowance covers RPC IDs, server metadata and result wrappers.
pub(crate) fn compact_mcp_bound(value: &impl Serialize) -> Result<usize, serde_json::Error> {
    struct Counter {
        bytes: usize,
        escapes: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .ok_or_else(|| std::io::Error::other("response byte count overflow"))?;
            self.escapes = self
                .escapes
                .checked_add(bytes.iter().filter(|b| matches!(b, b'"' | b'\\')).count())
                .ok_or_else(|| std::io::Error::other("response escape count overflow"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter {
        bytes: 0,
        escapes: 0,
    };
    serde_json::to_writer(&mut counter, value)?;
    counter
        .bytes
        .checked_mul(2)
        .and_then(|n| n.checked_add(counter.escapes))
        .and_then(|n| n.checked_add(64 * 1024))
        .ok_or_else(|| {
            serde_json::Error::io(std::io::Error::other("response frame count overflow"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn native_single_incomplete_reply_keeps_receipt_readiness_and_warnings() {
        // Transport fixture from the retained BEFORE106 manifest, not a new
        // native-store acceptance/publication assertion.
        let p: crate::storage::batch::BatchProgress = serde_json::from_slice(include_bytes!(
            "../../tests/fixtures/native-batch-before106/v3-manifest.json"
        ))
        .unwrap();
        let receipt = p
            .items
            .iter()
            .find_map(|i| match &i.state {
                crate::storage::batch::BatchItemState::Ok { receipt } => Some(receipt.clone()),
                _ => None,
            })
            .unwrap();
        let readiness = Readiness {
            store_id: receipt.store_id,
            committed_generation: receipt.committed_generation,
            ready: false,
            pending_work: 0,
            derived: vec![],
            pending_work_scope: None,
            observed_work: None,
        };
        let readiness_bytes = serde_json::to_vec(&readiness).unwrap();
        for missing_data in [false, true] {
            let reply = Reply {
                namespace_id: missing_data.then(|| NamespaceId::new(1)),
                namespace: missing_data.then(|| "fixture".into()),
                data: if missing_data {
                    None
                } else {
                    Some(serde_json::from_value(json!({"id":uuid::Uuid::from_u128(1),"namespace":"fixture","phase":"full","strength":1.,"stability":1.,"createdAt":"2026-01-01T00:00:00Z"})).unwrap())
                },
                receipt: Some(receipt.clone()),
                readiness: Some(readiness.clone()),
                read_generation: None,
                error: None,
                warnings: vec!["ORIGINAL_WARNING".into()],
            };
            // The private daemon decoder accepts optional missing identity;
            // response conversion must not erase canonical evidence.
            let decoded: Reply =
                serde_json::from_slice(&serde_json::to_vec(&reply).unwrap()).unwrap();
            let (status, envelope) = decoded.response();
            assert_eq!(status, 500);
            assert!(matches!(envelope.status, Status::Error));
            assert!(envelope.data.is_none());
            assert_eq!(envelope.receipt.as_ref(), Some(&receipt));
            assert_eq!(
                serde_json::to_vec(envelope.readiness.as_ref().unwrap()).unwrap(),
                readiness_bytes
            );
            assert_eq!(envelope.warnings, ["ORIGINAL_WARNING"]);
            let error = envelope.error.unwrap();
            assert_eq!(error.commit_state, CommitState::Committed);
            assert_eq!(error.operation_id, Some(receipt.operation_id));
        }
        let uncertain = Reply::uncertain();
        assert!(uncertain.receipt.is_none());
        assert!(uncertain.readiness.is_none());
        assert_eq!(uncertain.error.unwrap().commit_state, CommitState::Unknown);
    }
    #[test]
    fn native_single_strict_fields_and_original_vector_bits() {
        let good = json!({"contractVersion":2,"summary":"José","namespaceId":1,"embedding":[2.,-0.],"embeddingProfileId":format!("ep1:{}", "a".repeat(64))});
        let parsed = Request::parse(&good, false, None, "wrong-default")
            .unwrap()
            .prepare()
            .unwrap();
        assert_eq!(
            parsed
                .input
                .insert
                .memory
                .embedding
                .unwrap()
                .iter()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>(),
            vec![2f32.to_bits(), (-0f32).to_bits()]
        );
        for (field, value) in [
            ("embedding", json!(null)),
            ("embedding", json!([0., 0.])),
            ("embeddingProfileId", json!(null)),
            ("namespaceId", json!(0)),
            ("expectedGeneration", json!(1)),
            ("createdAt", json!(1000)),
            ("observedAt", json!(1000)),
            ("unknown", json!(true)),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert!(
                Request::parse(&bad, false, None, "native").is_err(),
                "{field}"
            );
        }
        for (value, status) in [(json!(null), 400), (json!([0., 0.]), 422), (json!([]), 422)] {
            let mut bad = good.clone();
            bad["embedding"] = value;
            let error = Request::parse(&bad, false, None, "native").unwrap_err();
            assert_eq!(Reply::invalid(error).response().0, status);
        }
        assert_eq!(input_schema(false)["additionalProperties"], false);
        assert!(input_schema(false)["properties"].get("createdAt").is_none());
        assert_eq!(
            input_schema(true)["properties"]["createdAt"]["type"],
            "integer"
        );
    }
}
