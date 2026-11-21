//! Live modern v1 structured outputs. Prepared v2 schemas are not capabilities.
use serde_json::{Value, json};

#[path = "tool_catalog_schema.rs"]
mod catalog;
#[path = "tool_lifecycle_schema.rs"]
mod lifecycle;
#[path = "tool_receipt_schema.rs"]
mod receipt;
#[path = "tool_record_schema.rs"]
mod record;

fn object(properties: Value) -> Value {
    object_optional(properties, &[])
}

fn object_optional(properties: Value, optional: &[&str]) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .unwrap()
        .keys()
        .filter(|key| !optional.contains(&key.as_str()))
        .cloned()
        .collect();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}

fn nonempty_array(items: Value) -> Value {
    json!({"type":"array","minItems":1,"items":items})
}

fn labels() -> Value {
    let strings = array(json!({"type":"string"}));
    object(json!({
        "entities":strings,"topics":strings,"emotions":strings,
        "provenance":{"enum":["native_v2","legacy_retained_only"]}
    }))
}

fn search_hit() -> Value {
    let string = json!({"type":"string"});
    let strings = array(string.clone());
    object_optional(
        json!({
            "id":string,"summary":string,"fullText":string,"score":{"type":"number"},
            "namespace":string,"tags":strings,"entities":nonempty_array(string.clone()),"topics":nonempty_array(string.clone()),
            "emotions":nonempty_array(string.clone()),"phase":string,"strength":{"type":"number"},
            "createdAt":string,"lastAccessedAt":string,"labels":labels(),
            "related":nonempty_array(object(json!({"id":string,"edgeType":string,"weight":{"type":"number"}})))
        }),
        &[
            "fullText", "entities", "topics", "emotions", "labels", "related",
        ],
    )
}

/// `NamespacePolicyView`: the stored policy plus the derived effective terms.
fn policy() -> Value {
    let number = json!({"type":"number"});
    let days = json!({"type":["number","null"]});
    object(json!({
        "initialStability":number,"defaultDifficulty":number,"desiredRetention":number,
        "phaseThresholds":object(json!({"fullToSummary":number,"summaryToGhost":number,"ghostToDelete":number})),
        "permastoreThreshold":number,"decayRateMultiplier":days,
        "effectiveDecayRateMultiplier":number,"daysUntilSummary":days,"daysUntilGhost":days
    }))
}

pub(super) fn output(tool: &str) -> Option<Value> {
    let count = json!({"type":"integer","minimum":0,"maximum":u64::MAX});
    let name = json!({"type":"string"});
    Some(match tool {
        "get_session" => json!({
            "type":"object",
            "required":["supportedContractVersions","defaultNamespace","namespacesAreAccessControls","accessDefaults"],
            "properties":{
                "supportedContractVersions":{"type":"array","items":{"const":1}},
                "defaultNamespace":{
                    "type":"object","required":["requestedName","source","exists","byIdResolution","resolved"],
                    "properties":{
                        "requestedName":{"type":"string"},"source":{"type":"string"},
                        "exists":{"type":"boolean"},"byIdResolution":{"const":"record"},
                        "resolved":{"type":["object","null"]}
                    }
                },
                "namespacesAreAccessControls":{"const":false},
                "accessDefaults":{"type":"object"}
            }
        }),
        "delete_namespace" => catalog::delete_output(),
        "move_memories" => crate::serialization::move_memories::output_schema(),
        "get_memory" | "store_memory" | "store_memories" | "forget_memories" => {
            record::output(tool)
        }
        "reinforce_memory" => lifecycle::output(true),
        "forget_memory" => lifecycle::output(false),
        "list_memories" => {
            let mut entries = array(object_optional(
                json!({
                    "id":name,"summary":name,"createdAt":name,
                    "entities":nonempty_array(name.clone()),"topics":nonempty_array(name.clone()),"labels":labels()
                }),
                &["entities", "topics", "labels"],
            ));
            entries["maxItems"] = json!(crate::model::constants::MAX_LIST_LIMIT);
            object(json!({
                "memories":entries,"total":count,"offset":count,
                "limit":{"type":"integer","minimum":1,"maximum":crate::model::constants::MAX_LIST_LIMIT}
            }))
        }
        "recall_memories" => {
            let compact = object(json!({
                "memories":array(object_optional(json!({
                    "id":name,"summary":name,"fullText":name,
                    "entities":nonempty_array(name.clone()),"topics":nonempty_array(name.clone()),"labels":labels()
                }), &["fullText","entities","topics","labels"])),
                "count":count
            }));
            let neighbor = object_optional(
                json!({
                    "id":name,"summary":name,"fullText":name,"edgeType":name,
                    "connectedTo":name,"weight":{"type":"number"},
                    "topics":nonempty_array(name.clone()),"emotions":nonempty_array(name.clone()),"labels":labels()
                }),
                &["fullText", "topics", "emotions", "labels"],
            );
            let full = object_optional(
                json!({
                    "memories":array(search_hit()),"count":count,
                    "graphContext":object(json!({"neighbors":nonempty_array(neighbor),"neighborCount":count}))
                }),
                &["graphContext"],
            );
            // Empty compact and full results overlap legitimately.
            json!({"type":"object","anyOf":[compact,full]})
        }
        "find_similar_memories" => {
            let coverage = |known| {
                object(json!({
                    "algorithm":{"const":"sampledNearestNeighbors"},
                    "sampleCap":{"const":crate::mcp::bridge::DUPLICATE_SAMPLE_CAP},
                    "neighborCap":{"const":crate::mcp::bridge::DUPLICATE_NEIGHBOR_CAP},
                    "complete":{"const":false},"countsAvailable":{"const":known},
                    "eligibleCount":if known {count.clone()} else {json!({"type":"null"})},
                    "sampledCount":if known {count.clone()} else {json!({"type":"null"})},
                    "skippedVectorCount":if known {count.clone()} else {json!({"type":"null"})},
                    "truncated":{"type":if known {"boolean"} else {"null"}}
                }))
            };
            let single =
                object(json!({"sourceId":name,"memories":array(search_hit()),"count":count}));
            let scan = object(json!({
                "namespace":name,"threshold":{"type":"number","minimum":0,"maximum":1},
                "clusters":array(object(json!({
                    "memories":array(object(json!({"id":name,"summary":name}))),
                    "maxSimilarity":{"type":"number"}
                }))),
                "clusterCount":count,"coverage":{"anyOf":[coverage(true),coverage(false)]}
            }));
            json!({"type":"object","anyOf":[single,scan]})
        }
        "namespace_stats" => object_optional(
            json!({
                "name":name,"memoryCount":count,"tombstoneCount":count,
                "phaseCounts":object(json!({"full":count,"summary":count,"ghost":count})),
                "permastoreCount":count,"avgStrength":{"type":"number"},
                "edgeCount":count,"vectorBytes":count,"policy":policy()
            }),
            &["policy"],
        ),
        "list_tags" => {
            let bucket = object(json!({
                "total":count,"truncated":{"type":"boolean"},
                "items":{"type":"array","maxItems":crate::model::constants::MAX_TAG_LIMIT,
                    "items":object(json!({"name":name,"count":count}))}
            }));
            object(json!({
                "namespace":{"type":["string","null"]},
                "memoriesCounted":{"anyOf":[count,{"type":"null"}]},
                "tags":bucket,"entities":bucket,"topics":bucket,"emotions":bucket
            }))
        }
        _ => return None,
    })
}

#[cfg(test)]
#[path = "tool_schema_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tool_read_schema_tests.rs"]
mod read_tests;
