//! Existing public API1 record outputs; no versioned activation or attribution.
use super::{array, labels, object, object_optional, receipt};
use receipt::{nullable, reference as r, uint};
use serde_json::{Value, json};

pub(super) fn output(tool: &str) -> Value {
    let mut defs = receipt::definitions();
    let string = json!({"type":"string"});
    let boolean = json!({"type":"boolean"});
    let uuid = json!({"type":"string","format":"uuid"});
    let count = uint(u64::MAX);
    let u32n = uint(u32::MAX.into());
    let number = defs["state"]["properties"]["stability"].clone();
    let commit = json!({"enum":["notCommitted","committed","unknown"]});
    defs.insert("labels".into(), labels());
    defs.insert("scope".into(), json!({"oneOf":[
        object(json!({"namespaceId":u32n,"namespace":string})),
        object(json!({"kind":{"const":"catalog"}})),object(json!({"namespaceIds":array(u32n.clone())}))
    ]}));
    defs.insert("supersedes".into(), object_optional(json!({"target":uuid,
        "status":{"enum":["applied","alreadyApplied","appliedNotDurable","failed"]},"detail":string}), &["detail"]));
    defs.insert(
        "nearDuplicates".into(),
        object(json!({"threshold":{"type":"number"},
        "matches":array(object(json!({"id":uuid,"score":number,"summary":string})))})),
    );
    defs.insert("stored".into(), object_optional(json!({"id":string,"namespace":string,"phase":string,
        "strength":number,"stability":number,"createdAt":string,"supersedes":r("supersedes"),"nearDuplicates":r("nearDuplicates")
    }), &["supersedes","nearDuplicates"]));
    defs.insert("memory".into(), object_optional(json!({"id":string,"namespace":string,"summary":string,
        "fullText":nullable(string.clone()),"tags":array(string.clone()),"phase":string,"strength":number,"stability":number,
        "createdAt":string,"lastAccessedAt":string,"isPermastore":boolean,"edgeCount":uint(u16::MAX.into()),"labels":r("labels")
    }), &["labels"]));
    let body = match tool {
        "get_memory" => {
            // This decoder validates known exposure receipts. Its raw fallback
            // remains strings; do not treat those strings as WriteReceipts.
            let query = crate::serialization::query_schema::output_schema(false);
            let mut exposure = query["$defs"]["receipt"].clone();
            exposure["properties"]["outcome"]["oneOf"][1]["properties"]["transition"] =
                r("feedbackTransition");
            defs.insert("exposureReceipt".into(), exposure);
            let failure = object_optional(
                json!({
                    "error":string,"category":defs["error"]["properties"]["category"],"message":string,"field":string,
                    "retryable":boolean,"commitState":commit,"ready":{"const":false},"scope":r("scope"),
                    "receipt":r("exposureReceipt"),"observationReceipts":array(r("exposureReceipt")),
                    "readGeneration":r("generationRef"),"readiness":r("readiness"),"operationId":uuid,
                    "receivedEvidence":object(json!({"receiptsJson":{"type":"array","minItems":1,
                        "maxItems":crate::serialization::get_api1::MAX_FRAGMENTS,"items":string,
                        "description":"Original unverified receipt JSON strings, not validated current-operation evidence."}})),
                    "warnings":array(string.clone())
                }),
                &[
                    "field",
                    "scope",
                    "receipt",
                    "readGeneration",
                    "readiness",
                    "operationId",
                    "receivedEvidence",
                ],
            );
            json!({"anyOf":[r("memory"),failure]})
        }
        "store_memory" => {
            let failure = object_optional(
                json!({"error":string,"message":string,"field":string,
                    "commitState":commit,"ready":{"const":false},"receipt":r("retainedReceipt"),
                    "receivedEvidence":object(json!({"receipt":r("unverifiedReceipt")})),
                    "readiness":r("readiness"),"operationId":uuid,"warnings":array(string.clone())
                }),
                &[
                    "field",
                    "receipt",
                    "receivedEvidence",
                    "readiness",
                    "operationId",
                    "warnings",
                ],
            );
            json!({"anyOf":[r("stored"),failure]})
        }
        "store_memories" | "forget_memories" => {
            let insert = tool == "store_memories";
            let status = json!({"enum":["ok","error","notAttempted","unknown"]});
            defs.insert("batchItem".into(), object_optional(json!({"index":u32n,"status":status,"operationId":uuid,
                "namespaceId":u32n,"receipt":r("retainedReceipt"),"error":r("batchError"),"ready":boolean,"retryable":boolean,
                // BatchItemOutcome declares this as Value. It is display only,
                // never a retained proof, and legacy compatibility merges it.
                "result":{"description":"Response-only compatibility Value; not receipt evidence."}
            }), &["operationId","namespaceId","receipt","error","result"]));
            defs.insert("batch".into(), object_optional(json!({"resultVersion":u32n,"batchOperationId":uuid,"storeId":uuid,
                "reservationState":{"enum":["committed","unknown"]},"reservationReceipt":r("retainedReceipt"),
                "bookkeepingReceipts":array(r("retainedReceipt")),"unknownBookkeepingOperations":array(uuid.clone()),
                "items":array(r("batchItem")),"total":count,"succeeded":count,"failed":count,"unresolved":count
            }), &["reservationReceipt"]));
            let mut compat_properties = defs["stored"]["properties"].clone();
            let o = compat_properties.as_object_mut().unwrap();
            o.insert("index".into(), count.clone());
            o.insert("deleted".into(), boolean.clone());
            o.insert("status".into(), status.clone());
            o.insert("ready".into(), boolean.clone());
            o.insert("error".into(), string.clone());
            o.insert("field".into(), string.clone());
            o.insert("errorDetail".into(), r("batchError"));
            o.insert("receipt".into(), r("retainedReceipt"));
            o.insert("advisory".into(), object(json!({"requested":boolean,"readGeneration":nullable(r("generationRef")),"warnings":array(string.clone())})));
            // Existing durable compatibility deliberately merges its result
            // Value. Describe those extensions rather than silently forbidding
            // them or changing the producer to make a narrower schema possible.
            let compatibility_item = json!({"type":"object","properties":compat_properties,
                "required":["index"],"additionalProperties":true});
            let mut aggregate = json!({"results":array(compatibility_item),"total":count,"errors":count,
                "batch":r("batch"),"readiness":r("readiness"),"warnings":array(string.clone()),
                "error":object(json!({"code":{"const":"BATCH_NOT_READY_OR_PARTIAL"},"commitState":{"enum":["unknown","committed"]},
                    "message":string,"retryable":boolean}))});
            aggregate[if insert { "stored" } else { "deleted" }] = count.clone();
            let aggregate =
                object_optional(aggregate, &["batch", "readiness", "warnings", "error"]);
            if !insert {
                aggregate
            } else {
                let failure = object_optional(
                    json!({"error":r("error"),"receipt":r("retainedReceipt"),
                        "readiness":r("readiness"),"readGeneration":r("generationRef"),"warnings":array(string.clone()),
                        "batchOperationId":uuid,"reservationReceipt":r("retainedReceipt"),"commitEvidenceScope":{"const":"reservation"}
                    }),
                    &[
                        "receipt",
                        "readiness",
                        "readGeneration",
                        "batchOperationId",
                        "reservationReceipt",
                        "commitEvidenceScope",
                    ],
                );
                let evidence = object_optional(
                    json!({"batchOperationId":uuid,"storeId":uuid,
                        "reservationReceipt":r("unverifiedReceipt"),"bookkeepingReceipts":array(r("unverifiedReceipt")),
                        "unknownBookkeepingOperations":array(uuid.clone()),"items":array(object_optional(json!({
                            "index":u32n,"operationId":uuid,"namespaceId":u32n,"receipt":r("unverifiedReceipt")
                        }), &["operationId","namespaceId","receipt"]))
                    }),
                    &["reservationReceipt"],
                );
                let uncertain = object_optional(
                    json!({"error":object(json!({"code":{"const":"COMMIT_OUTCOME_UNKNOWN"},
                        "commitState":{"const":"unknown"},"retryable":boolean,"message":string})),
                        "receivedEvidence":evidence,"warnings":array(string.clone())
                    }),
                    &["receivedEvidence", "warnings"],
                );
                json!({"anyOf":[aggregate,failure,uncertain]})
            }
        }
        _ => unreachable!("only registered record schemas"),
    };
    let mut schema = body;
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["type"] = json!("object");
    schema["description"] = json!(
        "Existing API1 outputs. Retained receipt codec shape does not establish canonical validity, request binding or current-write commitment. Compatibility Value fields remain extensible."
    );
    schema["$defs"] = Value::Object(defs);
    schema
}
