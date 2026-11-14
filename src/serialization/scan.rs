//! Strict v2 duplicate-scan wire input. Not an activation or readiness switch.
use super::contract::{ContractVersion, InputError, reject_unknown_fields};
use crate::model::{
    constants::{DUPLICATE_SIMILARITY_THRESHOLD, NAMESPACE_NAME_MAX_BYTES},
    validation::validate_namespace_name,
};
use serde_json::{Map, Value, json};

pub(crate) const DEFAULT_PAIR_BUDGET: u32 = 10_000;
pub(crate) const MAX_PAIR_BUDGET: u32 = 100_000;
pub(crate) const MAX_CURSOR_BYTES: usize = 4096;
// Whole-snapshot limits, not sample caps: exceeding either must fail the page.
pub(crate) const MAX_SNAPSHOT_RECORDS: usize = 100_000;
pub(crate) const MAX_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;
// Bound the shared page for the existing daemon frame and both MCP payload
// copies (escaped pretty text plus structured JSON). No result truncation:
// exceeding this conservative wire bound fails the whole page before push.
pub(crate) const MAX_SCAN_RESPONSE_BYTES: usize = crate::daemon::protocol::MAX_MESSAGE_SIZE;
pub(crate) const SCAN_RESPONSE_FIXED_BYTES: usize = 64 * 1024;
// Two UUIDs, finite f64, keys, nested indentation, JSON escaping and both copies.
pub(crate) const PAIR_RESPONSE_MAX_BYTES: usize = 512;
pub(crate) const MAX_RESPONSE_PAIRS: usize =
    (MAX_SCAN_RESPONSE_BYTES - SCAN_RESPONSE_FIXED_BYTES) / PAIR_RESPONSE_MAX_BYTES;

#[derive(Debug, Clone, PartialEq)]
pub struct ExactScanRequest {
    pub namespace: Option<String>,
    pub threshold: f64,
    pub pair_budget: u32,
    pub cursor: Option<String>,
}
fn error(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: field.into(),
        message,
    }
}
fn object(value: &Value) -> Result<&Map<String, Value>, InputError> {
    value
        .as_object()
        .ok_or_else(|| error("INVALID_TYPE", "", "arguments must be an object"))
}
impl ExactScanRequest {
    pub(crate) fn from_mcp(value: &Value) -> Result<Self, InputError> {
        let object = object(value)?;
        reject_unknown_fields(
            object,
            &[
                "contractVersion",
                "mode",
                "namespace",
                "threshold",
                "cursor",
                "pairBudget",
            ],
        )?;
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(error(
                "INVALID_COMBINATION",
                "/contractVersion",
                "exact scan requires explicit contractVersion 2",
            ));
        }
        if object.get("mode").and_then(Value::as_str) != Some("scan") {
            return Err(error(
                "INVALID_COMBINATION",
                "/mode",
                "exact scan requires mode scan",
            ));
        }
        let namespace = match object.get("namespace") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) => {
                validate_namespace_name(name).map_err(|_| {
                    error(
                        "OUT_OF_RANGE",
                        "/namespace",
                        "namespace must be 1–64 ASCII letters, digits, hyphens or underscores",
                    )
                })?;
                Some(name.clone())
            }
            Some(_) => {
                return Err(error(
                    "INVALID_TYPE",
                    "/namespace",
                    "namespace must be a string or null",
                ));
            }
        };
        Self::parse_options(object, namespace)
    }
    /// REST selects v2 via its header and resolves namespace from the path.
    /// Body scope/version/mode selectors cannot shadow that resolved route.
    pub(crate) fn from_http(value: &Value, namespace: &str) -> Result<Self, InputError> {
        let object = object(value)?;
        reject_unknown_fields(object, &["threshold", "cursor", "pairBudget"])?;
        validate_namespace_name(namespace)
            .map_err(|_| error("OUT_OF_RANGE", "/namespace", "invalid namespace path"))?;
        Self::parse_options(object, Some(namespace.into()))
    }
    fn parse_options(
        object: &Map<String, Value>,
        namespace: Option<String>,
    ) -> Result<Self, InputError> {
        let threshold = match object.get("threshold") {
            None => DUPLICATE_SIMILARITY_THRESHOLD,
            Some(value) => value
                .as_f64()
                .ok_or_else(|| error("INVALID_TYPE", "/threshold", "threshold must be a number"))?,
        };
        if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
            return Err(error(
                "OUT_OF_RANGE",
                "/threshold",
                "threshold must be finite and between 0 and 1",
            ));
        }
        let pair_budget = match object.get("pairBudget") {
            None => DEFAULT_PAIR_BUDGET,
            Some(value) => {
                let n = value.as_u64().ok_or_else(|| {
                    error(
                        "INVALID_TYPE",
                        "/pairBudget",
                        "pairBudget must be an integer",
                    )
                })?;
                if !(1..=u64::from(MAX_PAIR_BUDGET)).contains(&n) {
                    return Err(error(
                        "OUT_OF_RANGE",
                        "/pairBudget",
                        "pairBudget must be between 1 and 100000",
                    ));
                }
                u32::try_from(n).map_err(|_| {
                    error(
                        "OUT_OF_RANGE",
                        "/pairBudget",
                        "pairBudget exceeds supported bounds",
                    )
                })?
            }
        };
        let cursor = object
            .get("cursor")
            .map(|value| {
                let text = value.as_str().ok_or_else(|| {
                    error(
                        "INVALID_TYPE",
                        "/cursor",
                        "cursor must be a string; omit it for the first page",
                    )
                })?;
                if text.is_empty() || text.len() > MAX_CURSOR_BYTES {
                    return Err(error(
                        "OUT_OF_RANGE",
                        "/cursor",
                        "cursor must contain 1–4096 bytes",
                    ));
                }
                Ok(text.to_owned())
            })
            .transpose()?;
        // Signed zero has one semantic binding. No f64->f32 threshold narrowing.
        Ok(Self {
            namespace,
            threshold: if threshold == 0.0 { 0.0 } else { threshold },
            pair_budget,
            cursor,
        })
    }
    /// Internal v2 schema, not included in live tools/list while v2 is gated.
    pub(crate) fn mcp_schema() -> Value {
        let mut schema = Self::http_schema();
        schema["properties"]["contractVersion"] = json!({"type":"integer","const":2});
        schema["properties"]["mode"] = json!({"type":"string","const":"scan"});
        schema["properties"]["namespace"] = json!({"type":["string","null"],"minLength":1,"maxLength":NAMESPACE_NAME_MAX_BYTES,"pattern":"^[A-Za-z0-9_-]+$","description":"Omitted/null uses resolved session scope."});
        schema["required"] = json!(["contractVersion", "mode"]);
        schema
    }
    pub(crate) fn http_schema() -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{
            "threshold":{"type":"number","minimum":0,"maximum":1,"default":DUPLICATE_SIMILARITY_THRESHOLD},
            "pairBudget":{"type":"integer","minimum":1,"maximum":MAX_PAIR_BUDGET,"default":DEFAULT_PAIR_BUDGET,"description":"Maximum unordered pairs compared, including nonmatching pairs; no coercion or clamping."},
            "cursor":{"type":"string","minLength":1,"maxLength":MAX_CURSOR_BYTES,"description":"Opaque continuation; omit for first page. Server limit is 4096 UTF-8 bytes; malformed, stale or differently bound cursors fail."}
        }})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_scan_wire_rejects_ignored_v1_fields_and_invalid_nulls() {
        let original = json!({"contractVersion":2,"mode":"scan","namespace":null});
        let parsed = ExactScanRequest::from_mcp(&original).unwrap();
        assert_eq!(parsed.namespace, None);
        assert_eq!(parsed.pair_budget, DEFAULT_PAIR_BUDGET);
        assert_eq!(parsed.threshold, DUPLICATE_SIMILARITY_THRESHOLD);
        assert_eq!(
            original,
            json!({"contractVersion":2,"mode":"scan","namespace":null})
        );
        for field in [
            "id",
            "sameNamespace",
            "limit",
            "minScore",
            "recordAccess",
            "maxMemories",
            "unknown",
        ] {
            let mut value = original.clone();
            value[field] = json!(null);
            let e = ExactScanRequest::from_mcp(&value).unwrap_err();
            assert_eq!(e.code, "UNKNOWN_FIELD");
            assert_eq!(e.field, format!("/{field}"));
        }
        for field in ["threshold", "pairBudget", "cursor"] {
            let mut value = original.clone();
            value[field] = json!(null);
            assert_eq!(
                ExactScanRequest::from_mcp(&value).unwrap_err().field,
                format!("/{field}")
            );
        }
        for value in [
            json!(1.5),
            json!("1"),
            json!(true),
            json!(0),
            json!(-1),
            json!(100001),
            json!(u64::MAX),
        ] {
            let mut input = original.clone();
            input["pairBudget"] = value;
            assert!(ExactScanRequest::from_mcp(&input).is_err());
        }
        for value in [json!(1), json!(100000)] {
            let mut input = original.clone();
            input["pairBudget"] = value;
            assert!(ExactScanRequest::from_mcp(&input).is_ok());
        }
        for value in [json!(-0.01), json!(1.01), json!("0.85"), json!(true)] {
            let mut input = original.clone();
            input["threshold"] = value;
            assert!(ExactScanRequest::from_mcp(&input).is_err());
        }
        for value in [json!(""), json!("contains space"), json!([]), json!(true)] {
            let mut input = original.clone();
            input["namespace"] = value;
            assert!(ExactScanRequest::from_mcp(&input).is_err());
        }
        assert!(ExactScanRequest::from_http(&json!({"contractVersion":2}), "default").is_err());
        assert!(ExactScanRequest::from_http(&json!({"namespace":"foreign"}), "default").is_err());
        assert!(
            ExactScanRequest::from_http(
                &json!({"cursor":"x".repeat(MAX_CURSOR_BYTES+1)}),
                "default"
            )
            .is_err()
        );
        assert_eq!(
            ExactScanRequest::from_http(&json!({"threshold":-0.0}), "default")
                .unwrap()
                .threshold
                .to_bits(),
            0
        );
    }
    #[test]
    fn exact_scan_schema_and_parser_share_bounds_without_advertising_v2() {
        let schema = ExactScanRequest::mcp_schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["pairBudget"]["maximum"],
            MAX_PAIR_BUDGET
        );
        assert_eq!(
            schema["properties"]["cursor"]["maxLength"],
            MAX_CURSOR_BYTES
        );
        for field in ["id", "sameNamespace", "limit", "minScore"] {
            assert!(schema["properties"].get(field).is_none());
        }
        for tool in crate::mcp::tools::tool_definitions() {
            assert_eq!(
                tool.input_schema["properties"]["contractVersion"]["enum"],
                json!([1])
            );
        }
    }
}
