//! Live modern v1 structured outputs. Prepared v2 schemas are not capabilities.
use serde_json::{Value, json};

fn object(properties: Value) -> Value {
    let required: Vec<_> = properties.as_object().unwrap().keys().cloned().collect();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

pub(super) fn output(tool: &str) -> Option<Value> {
    let count = json!({"type":"integer","minimum":0,"maximum":u64::MAX});
    let name = json!({"type":"string"});
    Some(match tool {
        "namespace_stats" => object(json!({
            "name":name,"memoryCount":count,"tombstoneCount":count,
            "phaseCounts":object(json!({"full":count,"summary":count,"ghost":count})),
            "permastoreCount":count,"avgStrength":{"type":"number"},
            "edgeCount":count,"vectorBytes":count
        })),
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
