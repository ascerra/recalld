//! Strict metadata-only list input and generation-bound UUID continuation.
//! Private contract2 consumer: live selectors still reject contract2.
use super::{
    contract::{ContractVersion, InputError, reject_unknown_fields},
    scan::MAX_CURSOR_BYTES,
};
use crate::{
    model::{DecayPhase, MemoryId, NamespaceId, Tag, constants::*, tag::StructuredLabelFilter},
    storage::control::GenerationRef,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn invalid(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: field.into(),
        message,
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilters {
    pub tags: Vec<Tag>,
    pub labels: StructuredLabelFilter,
    pub phase: Option<DecayPhase>,
    pub time_range_start: Option<i64>,
    pub time_range_end: Option<i64>,
}
#[derive(Debug, Clone)]
pub struct ListPageRequest {
    pub namespace: Option<String>,
    pub limit: usize,
    pub cursor: Option<String>,
    pub filters: ListFilters,
}
impl ListPageRequest {
    pub(crate) fn from_mcp(value: &Value) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(invalid(
                "INVALID_COMBINATION",
                "/contractVersion",
                "explicit contractVersion 2 required",
            ));
        }
        Self::parse(value, true)
    }
    pub(crate) fn from_http(query: &[(String, String)]) -> Result<Self, InputError> {
        let mut object = serde_json::Map::new();
        for (field, text) in query {
            if object.contains_key(field) {
                return Err(invalid(
                    "INVALID_COMBINATION",
                    &format!("/{field}"),
                    "repeated query parameter",
                ));
            }
            // URL parameters are strings. Arrays use JSON array syntax so commas
            // inside exact canonical labels are never split or trimmed.
            let value = match field.as_str() {
                "limit" | "timeRangeStart" | "timeRangeEnd" | "tags" | "entities" | "topics"
                | "emotions" => serde_json::from_str(text).map_err(|_| {
                    invalid(
                        "INVALID_TYPE",
                        &format!("/{field}"),
                        "query value must be a JSON integer or string array",
                    )
                })?,
                _ => Value::String(text.clone()),
            };
            object.insert(field.clone(), value);
        }
        Self::parse(&Value::Object(object), false)
    }
    fn parse(value: &Value, mcp: bool) -> Result<Self, InputError> {
        let obj = value
            .as_object()
            .ok_or_else(|| invalid("INVALID_TYPE", "", "arguments must be an object"))?;
        let mut fields = vec![
            "namespace",
            "limit",
            "cursor",
            "tags",
            "entities",
            "topics",
            "emotions",
            "phase",
            "timeRangeStart",
            "timeRangeEnd",
        ];
        if mcp {
            fields.push("contractVersion");
        }
        reject_unknown_fields(obj, &fields)?;
        for (field, value) in obj {
            if value.is_null() && field != "namespace" {
                return Err(invalid(
                    "INVALID_TYPE",
                    &format!("/{field}"),
                    "null is not allowed",
                ));
            }
        }
        let namespace = match obj.get("namespace") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) => {
                crate::model::validation::validate_namespace_name(name)
                    .map_err(|_| invalid("OUT_OF_RANGE", "/namespace", "invalid namespace"))?;
                Some(name.clone())
            }
            _ => {
                return Err(invalid(
                    "INVALID_TYPE",
                    "/namespace",
                    "namespace must be a string or null",
                ));
            }
        };
        let limit = match obj.get("limit") {
            None => DEFAULT_LIST_LIMIT,
            Some(value) => {
                let n = value
                    .as_u64()
                    .ok_or_else(|| invalid("INVALID_TYPE", "/limit", "limit must be an integer"))?;
                if !(1..=MAX_LIST_LIMIT as u64).contains(&n) {
                    return Err(invalid("OUT_OF_RANGE", "/limit", "limit must be 1–200"));
                }
                n as usize
            }
        };
        let array = |field: &str| -> Result<Vec<String>, InputError> {
            let Some(value) = obj.get(field) else {
                return Ok(vec![]);
            };
            let values = value.as_array().ok_or_else(|| {
                invalid(
                    "INVALID_TYPE",
                    &format!("/{field}"),
                    "expected a string array",
                )
            })?;
            if values.len() > MAX_TAGS {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    &format!("/{field}"),
                    "too many labels or tags",
                ));
            }
            values
                .iter()
                .enumerate()
                .map(|(i, value)| {
                    let text = value.as_str().ok_or_else(|| {
                        invalid("INVALID_TYPE", &format!("/{field}/{i}"), "expected string")
                    })?;
                    if text.is_empty() || text.len() > LABEL_MAX_BYTES {
                        return Err(invalid(
                            "OUT_OF_RANGE",
                            &format!("/{field}/{i}"),
                            "label must contain 1–128 UTF-8 bytes",
                        ));
                    }
                    Ok(text.to_owned())
                })
                .collect()
        };
        let tags = array("tags")?
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                Tag::new(&s)
                    .map_err(|_| invalid("OUT_OF_RANGE", &format!("/tags/{i}"), "invalid tag"))
            })
            .collect::<Result<_, _>>()?;
        let labels = StructuredLabelFilter {
            entities: array("entities")?,
            topics: array("topics")?,
            emotions: array("emotions")?,
        };
        let time = |field: &str| -> Result<Option<i64>, InputError> {
            obj.get(field)
                .map(|v| {
                    let n = v.as_i64().ok_or_else(|| {
                        invalid(
                            "INVALID_TYPE",
                            &format!("/{field}"),
                            "timestamp must be integer milliseconds",
                        )
                    })?;
                    if chrono::DateTime::from_timestamp_millis(n).is_none() {
                        return Err(invalid(
                            "OUT_OF_RANGE",
                            &format!("/{field}"),
                            "timestamp is unrepresentable",
                        ));
                    }
                    Ok(n)
                })
                .transpose()
        };
        let time_range_start = time("timeRangeStart")?;
        let time_range_end = time("timeRangeEnd")?;
        if matches!((time_range_start,time_range_end),(Some(a),Some(b)) if a>b) {
            return Err(invalid(
                "INVALID_COMBINATION",
                "/timeRangeEnd",
                "time range must be ordered",
            ));
        }
        let phase = match obj.get("phase") {
            None => None,
            Some(Value::String(s)) => Some(match s.as_str() {
                "full" => DecayPhase::Full,
                "summary" => DecayPhase::Summary,
                "ghost" => DecayPhase::Ghost,
                _ => {
                    return Err(invalid(
                        "OUT_OF_RANGE",
                        "/phase",
                        "phase must be full, summary or ghost",
                    ));
                }
            }),
            _ => return Err(invalid("INVALID_TYPE", "/phase", "phase must be a string")),
        };
        let cursor = obj
            .get("cursor")
            .map(|v| {
                let s = v
                    .as_str()
                    .ok_or_else(|| invalid("INVALID_TYPE", "/cursor", "cursor must be a string"))?;
                if s.is_empty() || s.len() > MAX_CURSOR_BYTES {
                    return Err(invalid(
                        "OUT_OF_RANGE",
                        "/cursor",
                        "cursor must contain 1–4096 bytes",
                    ));
                }
                Ok(s.to_owned())
            })
            .transpose()?;
        Ok(Self {
            namespace,
            limit,
            cursor,
            filters: ListFilters {
                tags,
                labels,
                phase,
                time_range_start,
                time_range_end,
            },
        })
    }
    pub(crate) fn filter_hash(&self) -> [u8; 32] {
        Sha256::digest(serde_json::to_vec(&self.filters).expect("validated finite list filters"))
            .into()
    }
    /// Not advertised until the complete application contract is supported.
    pub(crate) fn schema() -> Value {
        let label = json!({"type":"array","maxItems":MAX_TAGS,"items":{"type":"string","minLength":1,"maxLength":LABEL_MAX_BYTES},"description":"Each item is 1–128 UTF-8 bytes. AND matching; whole-label Unicode lowercase, no trimming or Unicode normalization."});
        json!({"type":"object","additionalProperties":false,"required":["contractVersion"],"properties":{
            "contractVersion":{"type":"integer","const":2},
            "namespace":{"type":["string","null"],"minLength":1,"maxLength":NAMESPACE_NAME_MAX_BYTES,"pattern":"^[A-Za-z0-9_-]+$"},
            "limit":{"type":"integer","minimum":1,"maximum":MAX_LIST_LIMIT,"default":DEFAULT_LIST_LIMIT},
            "cursor":{"type":"string","minLength":1,"maxLength":MAX_CURSOR_BYTES},
            "tags":{"type":"array","maxItems":MAX_TAGS,"items":{"type":"string","minLength":1,"maxLength":TAG_MAX_BYTES,"pattern":"^[A-Za-z0-9][A-Za-z0-9/_.:-]*$"},"description":"AND matching of validated lowercase legacy tags; max 128 UTF-8 bytes per tag."},"entities":label,"topics":label,"emotions":label,
            "phase":{"type":"string","enum":["full","summary","ghost"]},
            "timeRangeStart":{"type":"integer","minimum":chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis(),"maximum":chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis()},
            "timeRangeEnd":{"type":"integer","minimum":chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis(),"maximum":chrono::DateTime::<chrono::Utc>::MAX_UTC.timestamp_millis()}
        }})
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListCursor {
    pub version: u32,
    pub contract_version: u32,
    pub eligibility_version: u32,
    pub at: GenerationRef,
    pub namespace_id: NamespaceId,
    pub namespace_sha256: [u8; 32],
    pub filter_sha256: [u8; 32],
    pub after: MemoryId,
}
impl ListCursor {
    pub(crate) fn encode(&self) -> Result<String, InputError> {
        Ok(URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).map_err(|_| Self::invalid())?))
    }
    pub(crate) fn invalid() -> InputError {
        invalid(
            "INVALID_CURSOR",
            "/cursor",
            "cursor is malformed, unsupported or differently bound",
        )
    }
    pub(crate) fn decode(token: &str, request: &ListPageRequest) -> Result<Self, InputError> {
        if token.is_empty() || token.len() > MAX_CURSOR_BYTES {
            return Err(Self::invalid());
        }
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| Self::invalid())?;
        if URL_SAFE_NO_PAD.encode(&bytes) != token {
            return Err(Self::invalid());
        }
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| Self::invalid())?;
        if cursor.version != 1
            || cursor.contract_version != 2
            || cursor.eligibility_version != 1
            || cursor.at.store_id.is_nil()
            || cursor.namespace_id.is_unset()
            || cursor.filter_sha256 != request.filter_hash()
        {
            return Err(Self::invalid());
        }
        Ok(cursor)
    }
}
