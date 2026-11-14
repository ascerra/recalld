//! Strict complete numeric-policy replacement. No lifecycle/profile/name edits,
//! server-sampled defaults, provider or clock values enter caller retry identity.
use super::contract::{ContractVersion, InputError, WriteOptions, reject_unknown_fields};
use crate::{
    model::{NamespaceConfig, NamespaceId, namespace::PhaseThresholds},
    storage::{catalog::NamespacePolicy, native_catalog::NamespacePolicyState},
};
use serde::Serialize;
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyValues {
    pub initial_stability: f32,
    pub default_difficulty: f32,
    pub phase_thresholds: PhaseThresholds,
    pub permastore_threshold: f32,
    pub desired_retention: f32,
    pub decay_rate_multiplier: Option<f32>,
}
impl PolicyValues {
    pub(crate) fn policy(&self) -> NamespacePolicy {
        NamespacePolicy {
            initial_stability: self.initial_stability,
            default_difficulty: self.default_difficulty,
            phase_thresholds: self.phase_thresholds,
            permastore_threshold: self.permastore_threshold,
            desired_retention: self.desired_retention,
            decay_rate_multiplier: self.decay_rate_multiplier,
        }
    }
    pub(crate) fn retained(state: &NamespacePolicyState) -> Self {
        let mut ns = NamespaceConfig::default_namespace(0);
        state.apply(&mut ns);
        Self {
            initial_stability: ns.initial_stability,
            default_difficulty: ns.default_difficulty,
            phase_thresholds: ns.phase_thresholds,
            permastore_threshold: ns.permastore_threshold,
            desired_retention: ns.desired_retention,
            decay_rate_multiplier: ns.decay_rate_multiplier,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PolicyRequest {
    pub namespace_id: NamespaceId,
    pub replacement: PolicyValues,
    pub write: WriteOptions,
}
fn invalid(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: field.into(),
        message,
    }
}
fn object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, InputError> {
    value
        .as_object()
        .ok_or_else(|| invalid("INVALID_TYPE", field, "expected an object"))
}
fn fields(obj: &Map<String, Value>, allowed: &[&str], prefix: &str) -> Result<(), InputError> {
    reject_unknown_fields(obj, allowed).map_err(|mut e| {
        e.field = format!("{prefix}{}", e.field);
        e
    })
}
fn required<'a>(
    obj: &'a Map<String, Value>,
    name: &str,
    prefix: &str,
) -> Result<&'a Value, InputError> {
    obj.get(name).ok_or_else(|| {
        invalid(
            "MISSING_FIELD",
            &format!("{prefix}/{name}"),
            "complete policy replacement requires this field",
        )
    })
}
fn number(
    obj: &Map<String, Value>,
    name: &str,
    prefix: &str,
    min: f64,
    max: f64,
    exclusive_min: bool,
) -> Result<f32, InputError> {
    let field = format!("{prefix}/{name}");
    let v = required(obj, name, prefix)?;
    let n = v
        .as_f64()
        .ok_or_else(|| invalid("INVALID_TYPE", &field, "expected a finite number"))?;
    let f = n as f32;
    if !n.is_finite()
        || !f.is_finite()
        || n < min
        || n > max
        || (exclusive_min && n == min)
        || (n != 0.0 && f == 0.0)
        || (exclusive_min && f as f64 <= min)
    {
        return Err(invalid(
            "OUT_OF_RANGE",
            &field,
            "number exceeds the supported finite policy range",
        ));
    }
    Ok(f)
}
impl PolicyRequest {
    pub(crate) fn from_mcp(value: &Value) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(invalid(
                "INVALID_COMBINATION",
                "/contractVersion",
                "explicit contractVersion 2 required",
            ));
        }
        Self::parse(value, None, None, true)
    }
    pub(crate) fn from_http(
        value: &Value,
        id: &str,
        key: Option<&str>,
    ) -> Result<Self, InputError> {
        let id = id
            .parse::<u32>()
            .ok()
            .filter(|n| *n != 0 && n.to_string() == id)
            .ok_or_else(|| {
                invalid(
                    "OUT_OF_RANGE",
                    "/namespaceId",
                    "namespaceId must be a nonzero canonical u32",
                )
            })?;
        Self::parse(value, Some(NamespaceId::new(id)), key, false)
    }
    fn parse(
        value: &Value,
        path_id: Option<NamespaceId>,
        key: Option<&str>,
        mcp: bool,
    ) -> Result<Self, InputError> {
        let obj = object(value, "")?;
        let mut allowed = vec!["policy", "storeId", "idempotencyKey", "expectedGeneration"];
        if mcp {
            allowed.extend(["contractVersion", "namespaceId"]);
        }
        fields(obj, &allowed, "")?;
        let namespace_id = match path_id {
            Some(id) => id,
            None => {
                let n = required(obj, "namespaceId", "")?.as_u64().ok_or_else(|| {
                    invalid(
                        "INVALID_TYPE",
                        "/namespaceId",
                        "namespaceId must be an integer",
                    )
                })?;
                NamespaceId::new(u32::try_from(n).ok().filter(|n| *n != 0).ok_or_else(|| {
                    invalid(
                        "OUT_OF_RANGE",
                        "/namespaceId",
                        "namespaceId must be a nonzero u32",
                    )
                })?)
            }
        };
        let p = object(required(obj, "policy", "")?, "/policy")?;
        fields(
            p,
            &[
                "initialStability",
                "defaultDifficulty",
                "phaseThresholds",
                "permastoreThreshold",
                "desiredRetention",
                "decayRateMultiplier",
            ],
            "/policy",
        )?;
        let t = object(
            required(p, "phaseThresholds", "/policy")?,
            "/policy/phaseThresholds",
        )?;
        fields(
            t,
            &["fullToSummary", "summaryToGhost", "ghostToDelete"],
            "/policy/phaseThresholds",
        )?;
        let thresholds = PhaseThresholds {
            full_to_summary: number(t, "fullToSummary", "/policy/phaseThresholds", 0., 1., true)?,
            summary_to_ghost: number(t, "summaryToGhost", "/policy/phaseThresholds", 0., 1., true)?,
            ghost_to_delete: number(t, "ghostToDelete", "/policy/phaseThresholds", 0., 1., true)?,
        };
        thresholds.validate().map_err(|_| {
            invalid(
                "OUT_OF_RANGE",
                "/policy/phaseThresholds",
                "thresholds must satisfy 0 < ghostToDelete < summaryToGhost < fullToSummary < 1",
            )
        })?;
        let replacement = PolicyValues {
            initial_stability: number(p, "initialStability", "/policy", 0., f32::MAX as f64, true)?,
            default_difficulty: number(p, "defaultDifficulty", "/policy", 1., 10., false)?,
            phase_thresholds: thresholds,
            permastore_threshold: number(
                p,
                "permastoreThreshold",
                "/policy",
                0.,
                f32::MAX as f64,
                true,
            )?,
            desired_retention: number(p, "desiredRetention", "/policy", 0., 1., false)?,
            decay_rate_multiplier: if required(p, "decayRateMultiplier", "/policy")?.is_null() {
                None
            } else {
                Some(number(
                    p,
                    "decayRateMultiplier",
                    "/policy",
                    0.,
                    f32::MAX as f64,
                    false,
                )?)
            },
        };
        replacement
            .policy()
            .validate()
            .map_err(|_| invalid("OUT_OF_RANGE", "/policy", "invalid numeric policy"))?;
        Ok(Self {
            namespace_id,
            replacement,
            write: WriteOptions::parse(obj, key)?,
        })
    }
    /// Private schema; activation still requires whole application conformance.
    pub(crate) fn schema() -> Value {
        let positive = json!({"type":"number","exclusiveMinimum":0,"maximum":f32::MAX as f64});
        let unit = json!({"type":"number","exclusiveMinimum":0,"exclusiveMaximum":1});
        json!({"type":"object","additionalProperties":false,"required":["contractVersion","namespaceId","policy"],"properties":{
            "contractVersion":{"type":"integer","const":2},"namespaceId":{"type":"integer","minimum":1,"maximum":u32::MAX},
            "storeId":{"type":"string","format":"uuid"},"idempotencyKey":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[ -~]+$"},"expectedGeneration":{"type":"string","pattern":"^(0|[1-9][0-9]*)$","description":"Canonical decimal in 0..9223372036854775807; storeId is required with a key or precondition."},
            "policy":{"type":"object","additionalProperties":false,"required":["initialStability","defaultDifficulty","phaseThresholds","permastoreThreshold","desiredRetention","decayRateMultiplier"],"properties":{
                "initialStability":positive,"defaultDifficulty":{"type":"number","minimum":1,"maximum":10},"permastoreThreshold":positive,"desiredRetention":{"type":"number","minimum":0,"maximum":1},
                "decayRateMultiplier":{"type":["number","null"],"minimum":0,"maximum":f32::MAX as f64,"description":"Explicit null selects the owner's inherited multiplier; no profile/lifecycle/default toggles."},
                "phaseThresholds":{"type":"object","additionalProperties":false,"required":["fullToSummary","summaryToGhost","ghostToDelete"],"properties":{"fullToSummary":unit,"summaryToGhost":unit,"ghostToDelete":unit},"description":"Strictly descending thresholds, checked after conversion to finite f32."}
            }}
        }})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body() -> Value {
        json!({"contractVersion":2,"namespaceId":1,"policy":{
            "initialStability":1.0,"defaultDifficulty":5,"phaseThresholds":{"fullToSummary":0.8,"summaryToGhost":0.3,"ghostToDelete":0.1},"permastoreThreshold":100,"desiredRetention":0.9,"decayRateMultiplier":null
        }})
    }
    #[test]
    fn native_policy_wire_strict_complete_policy_and_shared_preconditions() {
        let original = body();
        assert!(
            PolicyRequest::from_mcp(&original)
                .unwrap()
                .replacement
                .decay_rate_multiplier
                .is_none()
        );
        for (pointer, bad) in [
            ("/namespaceId", json!(0)),
            ("/namespaceId", json!(4294967296u64)),
            ("/namespaceId", json!("1")),
            ("/contractVersion", json!("2")),
            ("/policy", Value::Null),
            ("/policy/initialStability", json!(0)),
            ("/policy/initialStability", json!(1e100)),
            ("/policy/initialStability", json!(1e-100)),
            ("/policy/defaultDifficulty", json!(11)),
            ("/policy/desiredRetention", json!(1.1)),
            ("/policy/desiredRetention", Value::Null),
            ("/policy/decayRateMultiplier", json!(-1)),
            ("/policy/phaseThresholds/fullToSummary", json!(1)),
            ("/policy/phaseThresholds/ghostToDelete", json!(0.3)),
        ] {
            let mut value = original.clone();
            *value.pointer_mut(pointer).unwrap() = bad;
            assert!(PolicyRequest::from_mcp(&value).is_err(), "{value}");
        }
        for field in [
            "initialStability",
            "defaultDifficulty",
            "phaseThresholds",
            "permastoreThreshold",
            "desiredRetention",
            "decayRateMultiplier",
        ] {
            let mut value = original.clone();
            value["policy"].as_object_mut().unwrap().remove(field);
            assert_eq!(
                PolicyRequest::from_mcp(&value).unwrap_err().field,
                format!("/policy/{field}")
            );
        }
        for (pointer, field) in [
            ("", "namespace"),
            ("/policy", "lifecyclePolicy"),
            ("/policy", "embeddingProfileId"),
            ("/policy/phaseThresholds", "other"),
        ] {
            let mut value = original.clone();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), json!("unknown"));
            assert_eq!(
                PolicyRequest::from_mcp(&value).unwrap_err().field,
                format!("{pointer}/{field}")
            );
        }
        let mut value = original.clone();
        value["idempotencyKey"] = json!("retry");
        assert!(PolicyRequest::from_mcp(&value).is_err());
        value["storeId"] = json!(uuid::Uuid::nil());
        for bad in [
            json!(1),
            json!("01"),
            json!("9223372036854775808"),
            Value::Null,
        ] {
            value["expectedGeneration"] = bad;
            assert!(PolicyRequest::from_mcp(&value).is_err());
        }
        value["expectedGeneration"] = json!("9223372036854775807");
        assert!(PolicyRequest::from_mcp(&value).is_ok());
        for retention in [0, 1] {
            let mut value = original.clone();
            value["policy"]["desiredRetention"] = json!(retention);
            value["policy"]["decayRateMultiplier"] = json!(0);
            assert!(PolicyRequest::from_mcp(&value).is_ok());
        }
        let mut http = original.clone();
        http.as_object_mut().unwrap().remove("contractVersion");
        http.as_object_mut().unwrap().remove("namespaceId");
        for bad in ["0", "01", "+1", "4294967296"] {
            assert!(PolicyRequest::from_http(&http, bad, None).is_err());
        }
        assert!(PolicyRequest::from_http(&http, "1", None).is_ok());
        http["namespaceId"] = json!(2);
        assert!(PolicyRequest::from_http(&http, "1", None).is_err());
        let schema = PolicyRequest::schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["policy"]["required"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
    }
}
