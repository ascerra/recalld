//! Output shapes of the actual prepared page consumers, not live v2 advertising.
//! Canonical selection and cross-field/count/cursor authority stay in the backend
//! and the strict Remote decoder. Schema string lengths count characters, not UTF-8 bytes.
use super::catalog_page::CatalogKind;
use serde_json::{Value, json};

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn all(properties: Value) -> Value {
    let fields = properties
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    object(properties.clone(), &fields)
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}
fn continuation(mut data: Value) -> Value {
    data["allOf"] = json!([{"if":{"properties":{"complete":{"const":true}}},"then":{"properties":{"nextCursor":{"type":"null"}}},"else":{"properties":{"nextCursor":{"type":"string","minLength":1}}}}]);
    data
}
pub(crate) fn output_schema(kind: Option<CatalogKind>) -> Value {
    // Reuse the already tested strict u63 decimal expression (including strict
    // end-of-string); no second numeric spelling/range contract.
    let generation = super::query_schema::output_schema(false)["$defs"]["generation"].clone();
    let uuid = json!({"type":"string","format":"uuid","not":{"const":"00000000-0000-0000-0000-000000000000"}});
    let id = json!({"type":"integer","minimum":1,"maximum":u32::MAX});
    let name = json!({"type":"string","minLength":1,"maxLength":crate::model::constants::NAMESPACE_NAME_MAX_BYTES,"pattern":"^[A-Za-z0-9_-]+(?![\\s\\S])"});
    let scope = all(json!({"namespaceId":id,"namespace":name}));
    let catalog = all(json!({"kind":{"const":"catalog"}}));
    let cursor = nullable(
        json!({"type":"string","minLength":1,"maxLength":super::scan::MAX_CURSOR_BYTES,"pattern":"^[A-Za-z0-9_-]+(?![\\s\\S])"}),
    );
    let time = json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX});
    let label =
        json!({"type":"string","minLength":1,"maxLength":crate::model::constants::LABEL_MAX_BYTES});
    let labels = all(
        json!({"entities":array(label.clone()),"topics":array(label.clone()),"emotions":array(label.clone()),"provenance":{"enum":["native_v2","legacy_retained_only"]}}),
    );
    let profile = json!({"type":"string","pattern":"^ep1:[0-9a-f]{64}(?![\\s\\S])"});
    let dimension = json!({"type":"integer","minimum":1,"maximum":8192});
    let configured = nullable(
        json!({"oneOf":[all(json!({"identity":{"const":"unknown"},"dimension":dimension})),all(json!({"identity":{"const":"configuration"},"dimension":dimension,"profileId":profile}))]}),
    );
    let (data, scope) = match kind {
        None => {
            let row = object(
                json!({"id":uuid,"summary":{"type":"string","maxLength":crate::canonical_list::MAX_ROW_BYTES},"phase":{"enum":["full","summary","ghost"]},"createdAt":time,"tags":array(label.clone()),"entities":array(label.clone()),"topics":array(label.clone()),"emotions":array(label.clone()),"labels":labels}),
                &[
                    "id",
                    "summary",
                    "phase",
                    "createdAt",
                    "tags",
                    "entities",
                    "topics",
                    "emotions",
                ],
            );
            let mut rows = array(row);
            rows["maxItems"] = json!(crate::model::constants::MAX_LIST_LIMIT);
            (
                continuation(all(
                    json!({"memories":rows,"total":generation,"limit":{"type":"integer","minimum":1,"maximum":crate::model::constants::MAX_LIST_LIMIT},"complete":{"type":"boolean"},"nextCursor":cursor}),
                )),
                scope,
            )
        }
        Some(CatalogKind::Namespaces) => {
            let row = all(
                json!({"id":id,"name":name,"embeddingDim":{"type":"integer","minimum":0,"maximum":u32::MAX},"memoryCount":generation,"createdAt":time,"embeddingProfileId":nullable(profile),"bindingProvenance":nullable(json!({"enum":["native_v2","operator_attestation","reembedding"]})),"profileCompatibility":{"enum":["compatible","unknownNamespaceProfile","unknownActiveProfile","dimensionMismatch","profileMismatch"]}}),
            );
            let mut rows = array(row);
            rows["maxItems"] = json!(crate::model::constants::MAX_LIST_LIMIT);
            (
                continuation(all(
                    json!({"namespaces":rows,"total":generation,"limit":{"type":"integer","minimum":1,"maximum":crate::model::constants::MAX_LIST_LIMIT},"complete":{"type":"boolean"},"nextCursor":cursor,"configuredEmbedding":configured}),
                )),
                catalog,
            )
        }
        Some(CatalogKind::Tags) => {
            let entry = all(
                json!({"name":label,"count":generation,"sources":all(json!({"rawTags":generation,"canonicalLegacyRetainedOnly":generation,"canonicalNativeV2":generation}))}),
            );
            let mut items = array(entry);
            items["maxItems"] = json!(crate::model::constants::MAX_TAG_LIMIT);
            let bucket =
                all(json!({"total":generation,"truncated":{"type":"boolean"},"items":items}));
            (
                continuation(all(
                    json!({"memoriesCounted":generation,"limit":{"type":"integer","minimum":1,"maximum":crate::model::constants::MAX_TAG_LIMIT},"tags":bucket,"entities":bucket,"topics":bucket,"emotions":bucket,"complete":{"type":"boolean"},"nextCursor":cursor}),
                )),
                json!({"oneOf":[scope,catalog]}),
            )
        }
    };
    let error = object(
        json!({"category":{"enum":["invalidInput","notFound","conflict","unavailable"]},"code":{"type":"string"},"message":{"type":"string"},"field":{"type":"string"},"retryable":{"type":"boolean"},"commitState":{"const":"notCommitted"}}),
        &["category", "code", "message", "retryable", "commitState"],
    );
    let mut schema = object(
        json!({"contractVersion":{"const":2},"status":{"enum":["ok","error"]},"scope":scope,"data":data,"readGeneration":all(json!({"storeId":uuid,"generation":generation})),"error":error,"warnings":array(json!({"type":"string"}))}),
        &["contractVersion", "status", "warnings"],
    );
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["allOf"] = json!([
        {"if":{"properties":{"status":{"const":"ok"}}},"then":{"required":["data","scope","readGeneration"],"not":{"required":["error"]}}},
        {"if":{"properties":{"status":{"const":"error"}}},"then":{"required":["error"],"not":{"anyOf":[{"required":["data"]},{"required":["scope"]},{"required":["readGeneration"]}]}}}
    ]);
    schema
}

#[cfg(test)]
pub(crate) fn record_fixture(kind: Option<CatalogKind>, value: &Value) {
    use sha2::{Digest, Sha256};
    let Some(dir) = std::env::var_os("RECALLD_PAGE_SCHEMA_FIXTURES") else {
        return;
    };
    let bytes = serde_json::to_vec(value).unwrap();
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let prefix = match kind {
        None => "list",
        Some(CatalogKind::Namespaces) => "namespaces",
        Some(CatalogKind::Tags) => "tags",
    };
    let path = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join(format!("{prefix}-{hash}.json")),
        serde_json::to_vec(&json!({"schema":output_schema(kind),"value":value})).unwrap(),
    )
    .unwrap();
}
