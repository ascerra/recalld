//! Strict native creation inputs. Parsing never samples provider/configuration
//! defaults: A1 must replay original selectors before resolving dynamic inputs.
use super::contract::{ContractVersion, InputError, WriteOptions, reject_unknown_fields};
use crate::storage::native_catalog::NativeNamespaceInput;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CreateNamespaceRequest {
    pub input: NativeNamespaceInput,
    pub write: WriteOptions,
}

impl CreateNamespaceRequest {
    pub(crate) fn from_mcp(value: &Value) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(invalid(
                "/contractVersion",
                "native creation requires contractVersion 2",
            ));
        }
        Self::parse(value, None, true)
    }

    pub(crate) fn from_http(value: &Value, header_key: Option<&str>) -> Result<Self, InputError> {
        Self::parse(value, header_key, false)
    }

    fn parse(value: &Value, header_key: Option<&str>, mcp: bool) -> Result<Self, InputError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("", "arguments must be an object"))?;
        let mut allowed = vec![
            "name",
            "embeddingDim",
            "embeddingProfileId",
            "initialStability",
            "desiredRetention",
            "decayRateMultiplier",
            "storeId",
            "idempotencyKey",
            "expectedGeneration",
        ];
        if mcp {
            allowed.push("contractVersion");
        }
        reject_unknown_fields(object, &allowed)?;
        for (name, value) in object {
            if value.is_null() {
                return Err(invalid(&format!("/{name}"), "this field is not nullable"));
            }
        }
        // Keep the established finite/range/name validation; v2 only changes
        // null and unknown-field handling, not legacy numeric bounds.
        let fields = crate::mcp::args::parse_create_namespace(value).map_err(|e| {
            let numeric_range = match e.field.as_str() {
                "embeddingDim" => value[&e.field].as_u64().is_some(),
                "initialStability" | "desiredRetention" | "decayRateMultiplier" => {
                    value[&e.field].is_number()
                }
                _ => false,
            };
            InputError {
                code: if numeric_range {
                    "OUT_OF_RANGE"
                } else {
                    "INVALID_TYPE"
                },
                field: format!("/{}", e.field),
                message: "invalid namespace creation field",
            }
        })?;
        let embedding_profile_id = object
            .get("embeddingProfileId")
            .map(|value| {
                let id = value.as_str().ok_or_else(|| {
                    invalid("/embeddingProfileId", "profile selector must be a string")
                })?;
                if !id.strip_prefix("ep1:").is_some_and(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                }) {
                    return Err(invalid(
                        "/embeddingProfileId",
                        "profile selector must be an ep1 digest",
                    ));
                }
                Ok(id.to_owned())
            })
            .transpose()?;
        Ok(Self {
            input: NativeNamespaceInput {
                name: fields.name,
                embedding_dim: fields.embedding_dim.map(u32::from),
                embedding_profile_id,
                initial_stability: fields.initial_stability,
                desired_retention: fields.desired_retention,
                decay_rate_multiplier: fields.decay_rate_multiplier,
            },
            write: WriteOptions::parse(object, header_key)?,
        })
    }
}

fn invalid(field: &str, message: &'static str) -> InputError {
    InputError {
        code: "INVALID_TYPE",
        field: field.into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_catalog_wire_strict_fields_and_legacy_bounds() {
        for (field, values) in [
            (
                "embeddingDim",
                vec![json!(0), json!(8193), json!(4.0), json!("4"), Value::Null],
            ),
            (
                "embeddingProfileId",
                vec![json!("unknown"), json!("ep1:abc"), json!({}), Value::Null],
            ),
            (
                "initialStability",
                vec![
                    json!(0),
                    json!(-1),
                    json!(1e100),
                    json!(1e-100),
                    Value::Null,
                ],
            ),
            (
                "desiredRetention",
                vec![json!(-0.1), json!(1.1), json!(false), Value::Null],
            ),
            (
                "decayRateMultiplier",
                vec![json!(-1), json!(1e100), Value::Null],
            ),
        ] {
            for value in values {
                let mut request = json!({"name":"created","contractVersion":2});
                request[field] = value;
                assert_eq!(
                    CreateNamespaceRequest::from_mcp(&request)
                        .unwrap_err()
                        .field,
                    format!("/{field}")
                );
            }
        }
        for retention in [0., 1.] {
            let parsed = CreateNamespaceRequest::from_http(
                &json!({"name":"created","desiredRetention":retention,"decayRateMultiplier":0}),
                None,
            )
            .unwrap();
            assert_eq!(parsed.input.desired_retention, Some(retention as f32));
            assert_eq!(parsed.input.decay_rate_multiplier, Some(0.));
        }
        assert_eq!(
            CreateNamespaceRequest::from_http(&json!({"name":"created","extra":1}), None)
                .unwrap_err()
                .code,
            "UNKNOWN_FIELD"
        );
        assert!(
            CreateNamespaceRequest::from_http(&json!({"name":"created","contractVersion":2}), None)
                .is_err()
        );
    }

    #[test]
    fn native_catalog_wire_preserves_selectors_and_cross_transport_key() {
        let id = uuid::Uuid::new_v4().to_string();
        let profile = format!("ep1:{}", "a".repeat(64));
        let mut request = json!({"name":"created","storeId":id,"expectedGeneration":"9007199254740993","idempotencyKey":"key","embeddingProfileId":profile,"embeddingDim":4});
        let http = CreateNamespaceRequest::from_http(&request, Some("key")).unwrap();
        request["contractVersion"] = json!(2);
        let mcp = CreateNamespaceRequest::from_mcp(&request).unwrap();
        assert_eq!(http.write, mcp.write);
        assert_eq!(mcp.input.embedding_dim, Some(4));
        assert_eq!(
            mcp.input.embedding_profile_id.as_deref(),
            Some(profile.as_str())
        );
        assert!(mcp.input.initial_stability.is_none());
        assert!(mcp.input.decay_rate_multiplier.is_none());
        request.as_object_mut().unwrap().remove("contractVersion");
        assert_eq!(
            CreateNamespaceRequest::from_http(&request, Some("different"))
                .unwrap_err()
                .field,
            "/idempotencyKey"
        );
        request["expectedGeneration"] = json!(9007199254740993u64);
        assert_eq!(
            CreateNamespaceRequest::from_http(&request, None)
                .unwrap_err()
                .field,
            "/expectedGeneration"
        );
    }
}
