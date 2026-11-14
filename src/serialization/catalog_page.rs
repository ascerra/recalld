//! Strict, private contract2 catalog input. V1 parsers are unchanged.
use super::{
    contract::{ContractVersion, InputError, reject_unknown_fields},
    list::invalid,
    scan::MAX_CURSOR_BYTES,
};
use crate::{
    model::{NamespaceId, constants::*},
    storage::control::GenerationRef,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogKind {
    Namespaces,
    Tags,
}
#[derive(Debug, Clone)]
pub struct CatalogPageRequest {
    pub kind: CatalogKind,
    pub namespace: Option<String>,
    pub all_namespaces: bool,
    pub limit: usize,
    pub cursor: Option<String>,
}
impl CatalogPageRequest {
    pub(crate) fn from_mcp(value: &Value, kind: CatalogKind) -> Result<Self, InputError> {
        if ContractVersion::from_mcp(value)? != ContractVersion::V2 {
            return Err(invalid(
                "INVALID_COMBINATION",
                "/contractVersion",
                "explicit contractVersion 2 required",
            ));
        }
        Self::parse(value, kind, true)
    }
    pub(crate) fn from_http(
        query: &[(String, String)],
        kind: CatalogKind,
    ) -> Result<Self, InputError> {
        let mut object = serde_json::Map::new();
        for (key, text) in query {
            if object.contains_key(key) {
                return Err(invalid(
                    "INVALID_COMBINATION",
                    &format!("/{key}"),
                    "repeated query parameter",
                ));
            }
            let value = match key.as_str() {
                "limit" | "allNamespaces" => serde_json::from_str(text).map_err(|_| {
                    invalid(
                        "INVALID_TYPE",
                        &format!("/{key}"),
                        "expected a JSON integer or boolean",
                    )
                })?,
                _ => Value::String(text.clone()),
            };
            object.insert(key.clone(), value);
        }
        Self::parse(&Value::Object(object), kind, false)
    }
    fn parse(value: &Value, kind: CatalogKind, mcp: bool) -> Result<Self, InputError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("INVALID_TYPE", "", "expected an object"))?;
        let mut fields = vec!["limit", "cursor"];
        if mcp {
            fields.push("contractVersion");
        }
        if kind == CatalogKind::Tags {
            fields.extend(["namespace", "allNamespaces"]);
        }
        reject_unknown_fields(object, &fields)?;
        let namespace = match object.get("namespace") {
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
        let all_namespaces = if kind == CatalogKind::Namespaces {
            true
        } else {
            match object.get("allNamespaces") {
                None => false,
                Some(Value::Bool(value)) => *value,
                _ => {
                    return Err(invalid(
                        "INVALID_TYPE",
                        "/allNamespaces",
                        "allNamespaces must be boolean",
                    ));
                }
            }
        };
        if all_namespaces && namespace.is_some() {
            return Err(invalid(
                "INVALID_COMBINATION",
                "/namespace",
                "namespace cannot be combined with allNamespaces",
            ));
        }
        let (default, max) = Self::limits(kind);
        let limit = match object.get("limit") {
            None => default,
            Some(value) => {
                let n = value
                    .as_u64()
                    .ok_or_else(|| invalid("INVALID_TYPE", "/limit", "limit must be an integer"))?;
                if n == 0 || n > max as u64 {
                    return Err(invalid(
                        "OUT_OF_RANGE",
                        "/limit",
                        "limit exceeds catalog bounds",
                    ));
                }
                n as usize
            }
        };
        let cursor = object
            .get("cursor")
            .map(|v| {
                let s = v
                    .as_str()
                    .ok_or_else(|| invalid("INVALID_TYPE", "/cursor", "cursor must be a string"))?;
                if s.is_empty() || s.len() > MAX_CURSOR_BYTES {
                    return Err(CatalogCursor::invalid());
                }
                Ok(s.to_owned())
            })
            .transpose()?;
        Ok(Self {
            kind,
            namespace,
            all_namespaces,
            limit,
            cursor,
        })
    }
    pub(crate) fn limits(kind: CatalogKind) -> (usize, usize) {
        match kind {
            CatalogKind::Namespaces => (DEFAULT_LIST_LIMIT, MAX_LIST_LIMIT),
            CatalogKind::Tags => (DEFAULT_TAG_LIMIT, MAX_TAG_LIMIT),
        }
    }
    pub(crate) fn schema(kind: CatalogKind) -> Value {
        let (default, max) = Self::limits(kind);
        let mut schema = json!({"type":"object","additionalProperties":false,"required":["contractVersion"],"properties":{
            "contractVersion":{"type":"integer","const":2},"limit":{"type":"integer","minimum":1,"maximum":max,"default":default,"description":"Per bucket for tags; total entries for namespaces. A separate whole-response byte budget also applies."},"cursor":{"type":"string","minLength":1,"maxLength":MAX_CURSOR_BYTES}
        }});
        if kind == CatalogKind::Tags {
            schema["properties"]["namespace"] = json!({"type":["string","null"],"minLength":1,"maxLength":NAMESPACE_NAME_MAX_BYTES,"pattern":"^[A-Za-z0-9_-]+$","description":"Omitted/null uses the actual session default unless allNamespaces is true."});
            schema["properties"]["allNamespaces"] = json!({"type":"boolean","default":false,"description":"Cannot be combined with an explicit namespace."});
        }
        schema
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CatalogCursor {
    pub version: u32,
    pub contract_version: u32,
    pub ordering_version: u32,
    pub kind: CatalogKind,
    pub at: GenerationRef,
    pub namespace_id: Option<NamespaceId>,
    pub catalog_sha256: String,
    pub namespace_after: Option<u32>,
    pub tag_after: [Option<String>; 4],
}
impl CatalogCursor {
    pub(crate) fn invalid() -> InputError {
        invalid(
            "INVALID_CURSOR",
            "/cursor",
            "catalog cursor is malformed, unsupported or differently bound",
        )
    }
    pub(crate) fn encode(&self) -> Result<String, InputError> {
        let token = URL_SAFE_NO_PAD.encode(serde_json::to_vec(self).map_err(|_| Self::invalid())?);
        if token.len() > MAX_CURSOR_BYTES {
            return Err(Self::invalid());
        }
        Ok(token)
    }
    pub(crate) fn decode(token: &str, request: &CatalogPageRequest) -> Result<Self, InputError> {
        if token.is_empty() || token.len() > MAX_CURSOR_BYTES {
            return Err(Self::invalid());
        }
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| Self::invalid())?;
        if URL_SAFE_NO_PAD.encode(&bytes) != token {
            return Err(Self::invalid());
        }
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| Self::invalid())?;
        let digest = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if cursor.version != 1
            || cursor.contract_version != 2
            || cursor.ordering_version != 1
            || cursor.kind != request.kind
            || cursor.at.store_id.is_nil()
            || cursor.namespace_id.is_some_and(|id| id.is_unset())
            || cursor.namespace_id.is_none() != request.all_namespaces
            || !digest(&cursor.catalog_sha256)
            || cursor.tag_after.iter().flatten().any(|s| !digest(s))
            || match cursor.kind {
                CatalogKind::Namespaces => {
                    cursor.namespace_after.is_none_or(|id| id == 0)
                        || cursor.tag_after.iter().any(Option::is_some)
                }
                CatalogKind::Tags => {
                    cursor.namespace_after.is_some() || cursor.tag_after.iter().all(Option::is_none)
                }
            }
        {
            return Err(Self::invalid());
        }
        Ok(cursor)
    }
}
