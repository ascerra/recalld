//! Prepared application2 query contract. Live version admission remains closed.
use super::contract::{ContractVersion, InputError, reject_unknown_fields};
use crate::mcp::args::MAX_SEARCH_LIMIT;
use crate::model::{Tag, constants::*, tag::StructuredLabelFilter};
use crate::search::{PipelineSearchFilter, QueryMode, SearchQuery, VersionMode};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone)]
pub struct QueryRequest {
    #[cfg(test)]
    pub(crate) before_barrier: Option<fn(&crate::derived_completion::DerivedCompletion)>,
    pub query: SearchQuery,
    pub version_mode: VersionMode,
    pub include_neighbors: bool,
    pub include_embeddings: bool,
    pub embedding_profile_id: Option<String>,
    pub explain: bool,
}

pub struct SimilarRequest {
    pub(crate) id: crate::model::MemoryId,
    pub(crate) namespace: Option<String>,
    pub(crate) options: crate::search::SimilaritySelection,
    pub(crate) explain: bool,
}
impl SimilarRequest {
    pub(crate) fn from_mcp(value: &Value) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(invalid(
                "INVALID_COMBINATION",
                "contractVersion",
                "prepared similarity requires explicit contractVersion 2",
            ));
        }
        Self::parse(value, None)
    }
    pub(crate) fn from_http(value: &Value, id: crate::model::MemoryId) -> Result<Self, InputError> {
        Self::parse(value, Some(id))
    }
    fn parse(value: &Value, path_id: Option<crate::model::MemoryId>) -> Result<Self, InputError> {
        let m = value
            .as_object()
            .ok_or_else(|| invalid("INVALID_TYPE", "", "similarity arguments must be an object"))?;
        let mut fields = vec![
            "namespace",
            "namespaceScope",
            "limit",
            "minScore",
            "versionMode",
            "recordAccess",
            "includeEmbeddings",
            "tags",
            "entities",
            "topics",
            "emotions",
            "phases",
            "minStrength",
            "permastoreOnly",
            "explain",
        ];
        if path_id.is_none() {
            fields.extend(["contractVersion", "id", "mode"]);
        }
        reject_unknown_fields(m, &fields)?;
        if path_id.is_none()
            && string(m, "mode", 16)?
                .as_deref()
                .is_some_and(|v| v != "single")
        {
            return Err(invalid(
                "INVALID_COMBINATION",
                "mode",
                "single similarity requires mode single",
            ));
        }
        let id = match path_id {
            Some(id) => id,
            None => {
                let id = string(m, "id", 36)?.ok_or_else(|| {
                    invalid("MISSING_FIELD", "id", "single similarity requires an id")
                })?;
                let uuid = uuid::Uuid::parse_str(&id)
                    .map_err(|_| invalid("INVALID_TYPE", "id", "id must be a UUID"))?;
                if uuid.is_nil() {
                    return Err(invalid("OUT_OF_RANGE", "id", "id must be a non-nil UUID"));
                }
                crate::model::MemoryId::from_uuid(uuid)
            }
        };
        let namespace = if m.get("namespace") == Some(&Value::Null) {
            None
        } else {
            string(m, "namespace", NAMESPACE_NAME_MAX_BYTES)?
        };
        if let Some(ns) = &namespace {
            crate::model::validation::validate_namespace_name(ns)
                .map_err(|_| invalid("OUT_OF_RANGE", "namespace", "invalid namespace assertion"))?;
        }
        let scope = match string(m, "namespaceScope", 32)?.as_deref() {
            None | Some("sameNamespace") => crate::search::SimilarityScope::SameNamespace,
            Some("allCompatible") => crate::search::SimilarityScope::AllCompatible,
            _ => {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    "namespaceScope",
                    "namespaceScope must be sameNamespace or allCompatible",
                ));
            }
        };
        let mode = match string(m, "versionMode", 16)?.as_deref() {
            None | Some("current") => VersionMode::Current,
            Some("history") => VersionMode::History,
            _ => {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    "versionMode",
                    "versionMode must be current or history",
                ));
            }
        };
        Ok(Self {
            id,
            namespace,
            explain: boolean(m, "explain", false)?,
            options: crate::search::SimilaritySelection {
                semantics: crate::search::RetrievalSemantics::V2(mode),
                scope,
                filter: filters(m)?,
                limit: integer(m, "limit", 10, 1, MAX_SEARCH_LIMIT as u64)? as usize,
                min_score: fraction(m, "minScore")?,
                record_access: boolean(m, "recordAccess", true)?,
                include_embeddings: boolean(m, "includeEmbeddings", false)?,
                original_contract_version: 2,
            },
        })
    }
}

fn invalid(code: &'static str, field: &str, message: &'static str) -> InputError {
    InputError {
        code,
        field: format!("/{field}"),
        message,
    }
}
fn string(m: &Map<String, Value>, key: &str, max: usize) -> Result<Option<String>, InputError> {
    m.get(key)
        .map(|v| {
            let s = v
                .as_str()
                .ok_or_else(|| invalid("INVALID_TYPE", key, "expected a string"))?;
            if s.is_empty() || s.len() > max {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    key,
                    "string is empty or exceeds its UTF-8 byte limit",
                ));
            }
            Ok(s.to_owned())
        })
        .transpose()
}
fn boolean(m: &Map<String, Value>, key: &str, default: bool) -> Result<bool, InputError> {
    m.get(key)
        .map(|v| {
            v.as_bool()
                .ok_or_else(|| invalid("INVALID_TYPE", key, "expected a boolean"))
        })
        .unwrap_or(Ok(default))
}
fn integer(
    m: &Map<String, Value>,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, InputError> {
    let n = m
        .get(key)
        .map(|v| {
            v.as_u64()
                .ok_or_else(|| invalid("INVALID_TYPE", key, "expected a nonnegative integer"))
        })
        .unwrap_or(Ok(default))?;
    if !(min..=max).contains(&n) {
        return Err(invalid(
            "OUT_OF_RANGE",
            key,
            "integer exceeds the supported range",
        ));
    }
    Ok(n)
}
fn fraction(m: &Map<String, Value>, key: &str) -> Result<Option<f32>, InputError> {
    m.get(key)
        .map(|v| {
            let n = v
                .as_f64()
                .ok_or_else(|| invalid("INVALID_TYPE", key, "expected a number"))?;
            if !n.is_finite() || !(0.0..=1.0).contains(&n) {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    key,
                    "expected a finite number in 0..1",
                ));
            }
            Ok(n as f32)
        })
        .transpose()
}
fn strings(m: &Map<String, Value>, key: &str) -> Result<Vec<String>, InputError> {
    let Some(v) = m.get(key) else {
        return Ok(vec![]);
    };
    let a = v
        .as_array()
        .ok_or_else(|| invalid("INVALID_TYPE", key, "expected a string array"))?;
    if a.len() > MAX_TAGS {
        return Err(invalid("OUT_OF_RANGE", key, "too many labels"));
    }
    a.iter()
        .map(|v| {
            let s = v
                .as_str()
                .ok_or_else(|| invalid("INVALID_TYPE", key, "every label must be a string"))?;
            if s.is_empty() || s.len() > TAG_MAX_BYTES {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    key,
                    "label exceeds its UTF-8 byte limit",
                ));
            }
            Ok(s.to_owned())
        })
        .collect()
}

fn filters(m: &Map<String, Value>) -> Result<PipelineSearchFilter, InputError> {
    let tags = strings(m, "tags")?
        .into_iter()
        .map(|t| Tag::new(&t).map_err(|_| invalid("OUT_OF_RANGE", "tags", "invalid raw tag")))
        .collect::<Result<Vec<_>, _>>()?;
    let entities = strings(m, "entities")?;
    let labels = StructuredLabelFilter {
        entities: entities.clone(),
        topics: strings(m, "topics")?,
        emotions: strings(m, "emotions")?,
    };
    labels.validate(true).map_err(|_| {
        invalid(
            "OUT_OF_RANGE",
            "entities",
            "invalid structured label filter",
        )
    })?;
    let phases = strings(m, "phases")?
        .into_iter()
        .map(|s| {
            serde_json::from_value(Value::String(s))
                .map_err(|_| invalid("OUT_OF_RANGE", "phases", "unsupported phase"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PipelineSearchFilter {
        labels,
        require_tags: tags,
        phases,
        min_strength: fraction(m, "minStrength")?,
        permastore_only: boolean(m, "permastoreOnly", false)?,
        ..Default::default()
    })
}

impl QueryRequest {
    pub(crate) fn from_mcp(value: &Value, default_namespace: &str) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(invalid(
                "INVALID_COMBINATION",
                "contractVersion",
                "prepared query requires explicit contractVersion 2",
            ));
        }
        Self::parse(value, default_namespace, true)
    }
    /// Called only after the HTTP header's application-version selection.
    pub(crate) fn from_http(value: &Value, default_namespace: &str) -> Result<Self, InputError> {
        Self::parse(value, default_namespace, false)
    }
    fn parse(value: &Value, default_namespace: &str, mcp: bool) -> Result<Self, InputError> {
        let m = value
            .as_object()
            .ok_or_else(|| invalid("INVALID_TYPE", "", "query must be an object"))?;
        let mut fields = vec![
            "namespace",
            "query",
            "ftsQuery",
            "embedding",
            "embeddingProfileId",
            "queryMode",
            "versionMode",
            "tags",
            "entities",
            "topics",
            "emotions",
            "phases",
            "minStrength",
            "minScore",
            "permastoreOnly",
            "includeGhosts",
            "limit",
            "depth",
            "timeRangeStart",
            "timeRangeEnd",
            "recordAccess",
            "explain",
            "includeEmbeddings",
        ];
        if mcp {
            fields.extend(["contractVersion", "compact"]);
        }
        reject_unknown_fields(m, &fields)?;
        let namespace = match m.get("namespace") {
            None | Some(Value::Null) => default_namespace.to_owned(),
            Some(v) => v
                .as_str()
                .ok_or_else(|| {
                    invalid(
                        "INVALID_TYPE",
                        "namespace",
                        "namespace must be a string or null",
                    )
                })?
                .to_owned(),
        };
        crate::model::validation::validate_namespace_name(&namespace)
            .map_err(|_| invalid("OUT_OF_RANGE", "namespace", "invalid namespace"))?;
        let version_mode = match string(m, "versionMode", 16)?.as_deref() {
            None | Some("current") => VersionMode::Current,
            Some("history") => VersionMode::History,
            _ => {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    "versionMode",
                    "versionMode must be current or history",
                ));
            }
        };
        let query_mode = match string(m, "queryMode", 32)?.as_deref() {
            None | Some("embedding_plus_metadata") => QueryMode::EmbeddingPlusMetadata,
            Some("embedding_only") => QueryMode::EmbeddingOnly,
            Some("metadata_only") => QueryMode::MetadataOnly,
            _ => {
                return Err(invalid(
                    "OUT_OF_RANGE",
                    "queryMode",
                    "unsupported queryMode",
                ));
            }
        };
        let explain = boolean(m, "explain", false)?;
        let text = string(m, "query", FULL_TEXT_MAX_BYTES)?;
        let fts_query = string(m, "ftsQuery", FULL_TEXT_MAX_BYTES)?;
        let profile = string(m, "embeddingProfileId", 68)?;
        if profile.as_ref().is_some_and(|p| {
            !p.strip_prefix("ep1:").is_some_and(|h| {
                h.len() == 64
                    && h.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        }) {
            return Err(invalid(
                "OUT_OF_RANGE",
                "embeddingProfileId",
                "expected ep1: followed by 64 lowercase hexadecimal digits",
            ));
        }
        let embedding = m
            .get("embedding")
            .map(|v| {
                let a = v.as_array().ok_or_else(|| {
                    invalid(
                        "INVALID_TYPE",
                        "embedding",
                        "embedding must be a number array",
                    )
                })?;
                if a.is_empty() || a.len() > 8192 {
                    return Err(invalid(
                        "OUT_OF_RANGE",
                        "embedding",
                        "embedding dimension must be in 1..8192",
                    ));
                }
                a.iter()
                    .map(|v| {
                        let n = v.as_f64().ok_or_else(|| {
                            invalid(
                                "INVALID_TYPE",
                                "embedding",
                                "every vector component must be a number",
                            )
                        })?;
                        let f = n as f32;
                        if !f.is_finite() {
                            return Err(invalid(
                                "OUT_OF_RANGE",
                                "embedding",
                                "vector components must be finite f32 values",
                            ));
                        }
                        Ok(f)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        if embedding.is_some() != profile.is_some() {
            return Err(invalid(
                "INVALID_COMBINATION",
                "embeddingProfileId",
                "embedding and embeddingProfileId must be supplied together",
            ));
        }
        if let Some(v) = &embedding {
            let mut copy = v.clone();
            crate::embedding::validation::normalize_vector(&mut copy, v.len()).map_err(|_| {
                invalid(
                    "OUT_OF_RANGE",
                    "embedding",
                    "embedding must have a finite nonzero norm",
                )
            })?;
        }
        if query_mode == QueryMode::MetadataOnly
            && (embedding.is_some() || text.is_some() || fts_query.is_some())
        {
            return Err(invalid(
                "INVALID_COMBINATION",
                "queryMode",
                "metadata_only rejects text, keyword and embedding selectors",
            ));
        }
        let filter = filters(m)?;
        let entities = filter.labels.entities.clone();
        let time = |key| {
            m.get(key)
                .map(|v| {
                    v.as_i64().ok_or_else(|| {
                        invalid("INVALID_TYPE", key, "expected integer epoch milliseconds")
                    })
                })
                .transpose()
        };
        let start = time("timeRangeStart")?;
        let end = time("timeRangeEnd")?;
        if start.zip(end).is_some_and(|(a, b)| a > b) {
            return Err(invalid(
                "INVALID_COMBINATION",
                "timeRangeEnd",
                "time range is reversed",
            ));
        }
        let mut query = SearchQuery {
            record_access: boolean(m, "recordAccess", true)?,
            query_embedding: embedding,
            text,
            fts_query,
            namespace,
            filter,
            limit: integer(m, "limit", 10, 1, MAX_SEARCH_LIMIT as u64)? as usize,
            min_score: fraction(m, "minScore")?.unwrap_or(0.),
            include_ghosts: boolean(m, "includeGhosts", false)?,
            query_mode,
            graph_depth: integer(m, "depth", 0, 0, 3)? as u8,
            time_range_start: start,
            time_range_end: end,
            entities,
        };
        query.validate().map_err(|_| {
            invalid(
                "INVALID_COMBINATION",
                "query",
                "query mode requires usable search criteria",
            )
        })?;
        Ok(Self {
            #[cfg(test)]
            before_barrier: None,
            query,
            version_mode,
            include_neighbors: mcp && !boolean(m, "compact", true)?,
            include_embeddings: boolean(m, "includeEmbeddings", false)?,
            embedding_profile_id: profile,
            explain,
        })
    }
}

/// Prepared schema, not advertised by the live v1 tool catalog yet.
pub(crate) fn input_schema(mcp: bool) -> Value {
    let mut properties = json!({
        "namespace":{"type":["string","null"],"minLength":1,"maxLength":64},
        "query":{"type":"string","minLength":1,"maxLength":FULL_TEXT_MAX_BYTES,"description":"UTF-8 byte limit enforced by server"},
        "ftsQuery":{"type":"string","minLength":1,"maxLength":FULL_TEXT_MAX_BYTES},
        "embedding":{"type":"array","minItems":1,"maxItems":8192,"items":{"type":"number","minimum":-(f32::MAX as f64),"maximum":f32::MAX as f64}},
        "embeddingProfileId":{"type":"string","pattern":"^ep1:[0-9a-f]{64}(?![\\s\\S])"},
        "queryMode":{"enum":["embedding_plus_metadata","embedding_only","metadata_only"],"default":"embedding_plus_metadata"},
        "versionMode":{"enum":["current","history"],"default":"current"},
        "limit":{"type":"integer","minimum":1,"maximum":MAX_SEARCH_LIMIT,"default":10},
        "depth":{"type":"integer","minimum":0,"maximum":3,"default":0},
        "minStrength":{"type":"number","minimum":0,"maximum":1},
        "minScore":{"type":"number","minimum":0,"maximum":1},
        "timeRangeStart":{"type":"integer","minimum":i64::MIN,"maximum":i64::MAX},"timeRangeEnd":{"type":"integer","minimum":i64::MIN,"maximum":i64::MAX},
        "recordAccess":{"type":"boolean","default":true},
        "includeEmbeddings":{"type":"boolean","default":false},
        "includeGhosts":{"type":"boolean","default":false},
        "permastoreOnly":{"type":"boolean","default":false},"explain":{"type":"boolean","default":false,"description":"Ranked-query baseline-v1 evidence only; does not change ranking or exposure."}
    });
    for key in ["tags", "entities", "topics", "emotions"] {
        properties[key] = json!({"type":"array","maxItems":MAX_TAGS,"items":{"type":"string","minLength":1,"maxLength":TAG_MAX_BYTES}});
    }
    properties["phases"] = json!({"type":"array","maxItems":MAX_TAGS,"items":{"enum":["full","summary","ghost","tombstone"]}});
    if mcp {
        properties["contractVersion"] = json!({"const":2});
        properties["compact"] = json!({"type":"boolean","default":true});
    }
    let mut metadata = Vec::new();
    for field in ["tags", "entities", "topics", "emotions", "phases"] {
        metadata.push(json!({"required":[field],"properties":{field:{"minItems":1}}}));
    }
    metadata.push(json!({"required":["minStrength"]}));
    metadata.push(
        json!({"required":["permastoreOnly"],"properties":{"permastoreOnly":{"const":true}}}),
    );
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":properties,
        "required":if mcp {vec!["contractVersion"]} else {vec![]},
        "dependentRequired":{"embedding":["embeddingProfileId"],"embeddingProfileId":["embedding"]},
        "if":{"required":["queryMode"],"properties":{"queryMode":{"const":"metadata_only"}}},
        "then":{"allOf":[{"not":{"anyOf":[{"required":["query"]},{"required":["ftsQuery"]},{"required":["embedding"]}]}},{"anyOf":metadata}]},
        "else":{"anyOf":[{"required":["embedding"]},{"required":["query"],"properties":{"query":{"pattern":"\\S"}}}]}})
}

pub(crate) fn similar_input_schema(mcp: bool) -> Value {
    let common = input_schema(mcp);
    let mut properties = serde_json::Map::new();
    for field in [
        "namespace",
        "limit",
        "minScore",
        "versionMode",
        "recordAccess",
        "includeEmbeddings",
        "tags",
        "entities",
        "topics",
        "emotions",
        "phases",
        "minStrength",
        "permastoreOnly",
        "explain",
    ] {
        properties.insert(field.into(), common["properties"][field].clone());
    }
    properties.insert("explain".into(), json!({"type":"boolean","default":false,"description":"Actual cosine-max selection evidence for returned rows, with bounded eligible origins and exact omitted count. Does not change selection or exposure."}));
    properties.insert(
        "namespaceScope".into(),
        json!({"enum":["sameNamespace","allCompatible"],"default":"sameNamespace"}),
    );
    if mcp {
        properties.insert("contractVersion".into(), json!({"const":2}));
        properties.insert("mode".into(), json!({"const":"single","default":"single"}));
        properties.insert("id".into(), json!({"type":"string","format":"uuid"}));
    }
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":properties,"required":if mcp{vec!["contractVersion","id"]}else{vec![]}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_query_single_schema_rejects_scan_and_conflicting_legacy_scope_selectors() {
        let v = json!({"contractVersion":2,"id":"00000000-0064-7000-8000-000000000001"});
        assert!(SimilarRequest::from_mcp(&v).is_ok());
        for field in [
            "threshold",
            "cursor",
            "pairBudget",
            "sameNamespace",
            "query",
            "embedding",
            "depth",
        ] {
            let mut bad = v.clone();
            bad[field] = json!(false);
            assert!(SimilarRequest::from_mcp(&bad).is_err());
            assert!(
                similar_input_schema(true)["properties"]
                    .get(field)
                    .is_none()
            );
        }
        for (field, value) in [
            ("namespaceScope", json!("all")),
            ("namespace", json!(false)),
            ("mode", json!("scan")),
            ("limit", json!(0)),
            ("versionMode", Value::Null),
        ] {
            let mut bad = v.clone();
            bad[field] = value;
            assert!(SimilarRequest::from_mcp(&bad).is_err());
        }
        assert_eq!(
            similar_input_schema(true)["properties"]["namespaceScope"]["default"],
            "sameNamespace"
        );
    }
    #[test]
    fn strict_query_selectors_reject_before_work_and_preserve_defaults() {
        let base = json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"]});
        let request = QueryRequest::from_mcp(&base, "frozen").unwrap();
        assert_eq!(request.query.namespace, "frozen");
        assert!(request.query.record_access);
        assert_eq!(request.version_mode, VersionMode::Current);
        assert!(!request.include_neighbors);
        for (key, values) in [
            (
                "namespace",
                vec![json!(false), json!(0), json!([]), json!({}), json!("")],
            ),
            ("recordAccess", vec![json!(null), json!("false"), json!(0)]),
            (
                "limit",
                vec![json!(0), json!(101), json!(1.5), json!(null), json!("2")],
            ),
            ("depth", vec![json!(4), json!(null), json!(-1)]),
            ("minScore", vec![json!(-0.1), json!(1.1), json!(null)]),
            ("versionMode", vec![json!("asOf"), json!(null), json!(2)]),
            ("explain", vec![json!(null), json!("true")]),
            ("tags", vec![json!(null), json!("allow"), json!([1])]),
        ] {
            for value in values {
                let mut v = base.clone();
                v[key] = value;
                assert!(QueryRequest::from_mcp(&v, "default").is_err(), "{v}");
            }
        }
        let mut explicit = base.clone();
        explicit["namespace"] = json!("other");
        assert_eq!(
            QueryRequest::from_mcp(&explicit, "frozen")
                .unwrap()
                .query
                .namespace,
            "other"
        );
        explicit["namespace"] = Value::Null;
        assert_eq!(
            QueryRequest::from_mcp(&explicit, "frozen")
                .unwrap()
                .query
                .namespace,
            "frozen"
        );
        for key in [
            "sameNamespace",
            "namespaceScope",
            "cursor",
            "pairBudget",
            "unknown",
        ] {
            let mut v = base.clone();
            v[key] = json!(true);
            assert_eq!(
                QueryRequest::from_mcp(&v, "default").unwrap_err().code,
                "UNKNOWN_FIELD"
            );
        }
        let mut v = base.clone();
        v["query"] = json!("ignored?");
        assert_eq!(
            QueryRequest::from_mcp(&v, "default").unwrap_err().code,
            "INVALID_COMBINATION"
        );
        let mut v = base.clone();
        v["timeRangeStart"] = json!(2);
        v["timeRangeEnd"] = json!(1);
        assert!(QueryRequest::from_mcp(&v, "default").is_err());
        assert_eq!(
            input_schema(true)["properties"]["recordAccess"]["default"],
            true
        );
        assert_eq!(input_schema(true)["additionalProperties"], false);
        assert!(
            input_schema(false)["properties"]
                .get("contractVersion")
                .is_none()
        );
    }

    #[test]
    fn strict_query_vectors_require_pair_and_keep_original_f32_bits() {
        let profile = format!("ep1:{}", "a".repeat(64));
        let value = json!({"contractVersion":2,"query":"keywords","embedding":[2.0,0.0],"embeddingProfileId":profile});
        let parsed = QueryRequest::from_mcp(&value, "default").unwrap();
        assert_eq!(
            parsed.query.query_embedding.unwrap()[0].to_bits(),
            2f32.to_bits()
        );
        for bad in [
            json!([0., 0.]),
            json!([1e100, 0.]),
            json!(["1", 0.]),
            json!(null),
        ] {
            let mut v = value.clone();
            v["embedding"] = bad;
            assert!(QueryRequest::from_mcp(&v, "default").is_err());
        }
        let mut v = value.clone();
        v.as_object_mut().unwrap().remove("embeddingProfileId");
        assert!(QueryRequest::from_mcp(&v, "default").is_err());
        let mut v = value;
        v["embeddingProfileId"] = json!("passthrough");
        assert!(QueryRequest::from_mcp(&v, "default").is_err());
    }
}
