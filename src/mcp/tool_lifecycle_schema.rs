//! Live API1 lifecycle shapes, including retained typed invalid-response receipts.
//! These are codec schemas, not canonical proof or current-operation validation.
use super::{array, object, object_optional};
use serde_json::{Value, json};

fn r(name: &str) -> Value {
    json!({"$ref":format!("#/$defs/{name}")})
}
fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}
fn uint(max: u64) -> Value {
    json!({"type":"integer","minimum":0,"maximum":max})
}

pub(super) fn output(feedback: bool) -> Value {
    let string = json!({"type":"string"});
    let boolean = json!({"type":"boolean"});
    // Typed f32 output spells MAX as 3.4028235e38, slightly beyond the exact
    // f64 widening. Describe the actual emitter; never change receipt values.
    let max: f64 =
        serde_json::from_str(&serde_json::to_string(&f32::MAX).expect("finite f32 endpoint"))
            .expect("numeric f32 endpoint");
    let number = json!({"type":"number","minimum":-max,"maximum":max});
    let time = json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX});
    let uuid = json!({"type":"string","format":"uuid"});
    let namespace = uint(u32::MAX.into());
    // Reuse the already tested strict decimal codec, including terminal-newline
    // rejection, without importing prepared query output semantics.
    let generation =
        crate::serialization::query_schema::output_schema(false)["$defs"]["generation"].clone();
    let hash = json!({"type":"array","minItems":32,"maxItems":32,"items":uint(255)});
    let phase = json!({"enum":["full","summary","ghost","tombstone"]});
    let version = json!({"enum":["legacy_access_v1","explicit_feedback_v2"]});
    let access = json!({"enum":["direct_retrieval","associative_retrieval","decay_sweep","manual_reinforcement"]});
    let provenance = json!({"enum":["native_v2","legacy_retained_only"]});
    let mut defs = serde_json::Map::new();
    defs.insert("generation".into(), generation);
    defs.insert(
        "generationRef".into(),
        object(json!({"storeId":uuid,"generation":r("generation")})),
    );
    defs.insert("lifecycle".into(), object(json!({
        "observedAt":time,"retrievedAt":nullable(time.clone()),"reinforcedAt":nullable(time.clone()),
        "decayAnchorAt":time,"lifecycleVersion":version,"lifecycleUpdatedAt":time,
        "timestampOrigin":{"enum":["native","legacy_inferred"]},"phaseChangedAt":time,
        "policyRevision":r("generation"),"recordGeneration":r("generation")
    })));
    // FiniteF32's codec accepts finite values outside canonical lifecycle ranges.
    // Such typed receipts can be retained by the existing malformed-peer path.
    defs.insert(
        "state".into(),
        object(json!({
            "lifecycle":r("lifecycle"),"createdAt":time,"lastAccessedAt":time,
            "stability":number,"difficulty":number,"strength":number,"decayStrength":number,
            "phase":phase,"isPermastore":boolean,"hasSummary":boolean,"hasText":boolean
        })),
    );
    defs.insert(
        "change".into(),
        object(json!({
            "memoryId":uuid,"kind":access,"before":r("state"),"after":r("state"),
            "historyBefore":hash,"historyAfter":hash,
            "evictedEvent":nullable(object(json!({"timestamp":time,"kind":access})))
        })),
    );
    defs.insert("feedbackTransition".into(), object(json!({
        "namespaceId":namespace,"lifecycleVersion":version,"policyRevision":r("generation"),
        "previousGeneration":r("generation"),"captureGeneration":r("generationRef"),
        "sampledAt":time,"records":array(r("change")),
        "feedback":nullable(object(json!({"quality":uint(255),"decayRateMultiplier":number,"permastoreThreshold":number})))
    })));
    defs.insert(
        "vector".into(),
        object(
            json!({"slot":uint(u32::MAX.into()),"allocationEpoch":r("generation"),"sha256":hash}),
        ),
    );
    defs.insert(
        "content".into(),
        object(json!({
            "summarySha256":hash,"tagsSha256":hash,"labelsSha256":hash,"labelProvenance":provenance,
            "extensionsSha256":hash,"textSha256":nullable(hash.clone()),
            "vector":nullable(r("vector")),"profileId":nullable(string.clone())
        })),
    );
    defs.insert("recordState".into(), object(json!({
        "lifecycle":r("state"),"edgeCount":uint(u16::MAX.into()),"historySha256":hash,"content":r("content")
    })));
    defs.insert("document".into(), object(json!({
        "recipe":{"const":"document_v1"},"source":{"enum":["retained_canonical_v1","caller_document_v1"]},
        "sourceRecordGeneration":r("generation"),"phase":phase,"summarySha256":hash,"tagsSha256":hash,
        "labelsSha256":hash,"labelProvenance":provenance,"documentSha256":hash,
        "fullText":{"oneOf":[object(json!({"kind":{"const":"absent"}})),object(json!({"kind":{"const":"present"},"sha256":hash}))]}
    })));
    defs.insert("vectorExecution".into(), object(json!({
        "certification":object(json!({"version":uint(u32::MAX.into()),"origin":r("generationRef"),
            "operationId":uuid,"inventoryRoot":hash,"vectorCount":uint(u64::MAX)})),
        // Enum struct-variant fields retain snake_case unless explicitly renamed.
        "source":{"oneOf":[object(json!({"kind":{"const":"supplied"},"profile_id":string})),
            object(json!({"kind":{"const":"generated"},"document":r("document")}))]}
    })));
    defs.insert("recordTransition".into(), object_optional(json!({
        "namespaceId":namespace,"memoryId":uuid,"lifecycleVersion":version,"policyRevision":r("generation"),
        "previousGeneration":r("generation"),"captureGeneration":r("generationRef"),"sampledAt":time,
        "embeddingDim":uint(u32::MAX.into()),
        "records":array(object(json!({"memoryId":uuid,"before":nullable(r("recordState")),"after":r("recordState")}))),
        "edges":array(object(json!({"source":uuid,"target":uuid,
            "edgeType":{"enum":["parent_child","associative","causal","contradicts","entity","temporal","supersedes"]},
            "weightBits":uint(u32::MAX.into()),"autoCreated":boolean,"createdAt":uint(u64::MAX)}))),
        "phaseTimeRule":{"const":"monotonic_marker_v1"},"vectorExecution":r("vectorExecution")
    }), &["phaseTimeRule","vectorExecution"]));
    let outcome = if feedback {
        json!({"oneOf":[
            object(json!({"kind":{"const":"legacyFeedbackRecorded"},"memoryId":uuid,"quality":uint(255),"appliedAt":time})),
            object(json!({"kind":{"const":"lifecycleFeedbackApplied"},"transition":r("feedbackTransition")}))
        ]})
    } else {
        json!({"oneOf":[
            object(json!({"kind":{"const":"memoryTombstoned"},"memoryId":uuid,"status":{"enum":["tombstoned","already_tombstoned"]}})),
            object(json!({"kind":{"const":"nativeMemoryTombstoned"},"transition":r("recordTransition")}))
        ]})
    };
    defs.insert(
        "receipt".into(),
        object(json!({
            "storeId":uuid,"operationId":uuid,"committedGeneration":r("generation"),
            "receiptVersion":{"const":1},"replayed":boolean,"outcome":outcome
        })),
    );
    defs.insert("error".into(), object_optional(json!({
        "category":{"enum":["invalid_input","not_found","conflict","unavailable","partial","internal"]},
        "code":string,"message":string,"field":string,"retryable":boolean,
        "commitState":{"enum":["notCommitted","committed","unknown"]},"operationId":uuid
    }), &["field","operationId"]));
    defs.insert("data".into(), object(json!({
        "id":uuid,"lifecycleVersion":version,"policyRevision":r("generation"),"reinforcedAt":nullable(time.clone()),
        "phase":phase,"strength":number,"stability":number,"isPermastore":boolean,"historical":boolean
    })));
    defs.insert("legacyData".into(), json!({"oneOf":[
        object(json!({"kind":{"const":"feedback"},"id":uuid,"quality":uint(255),"appliedAt":time,"historical":boolean})),
        object(json!({"kind":{"const":"forget"},"id":uuid,"status":{"enum":["tombstoned","already_tombstoned"]},"historical":boolean}))
    ]}));
    defs.insert("readiness".into(), object_optional(json!({
        "storeId":uuid,"committedGeneration":r("generation"),"ready":boolean,"pendingWork":uint(u64::MAX),
        "derived":array(object(json!({"name":string,"appliedGeneration":nullable(r("generation")),"ready":boolean}))),
        "pendingWorkScope":{"const":"acceptedTickets"},
        "observedWork":object(json!({"unresolvedWork":uint(u64::MAX),"admissionOpen":boolean,
            "pendingEffects":uint(u64::MAX),"pendingPurges":uint(u64::MAX),"pendingBatchItems":uint(u64::MAX)}))
    }), &["pendingWorkScope","observedWork"]));
    let failure = object_optional(
        json!({
            "scope":nullable(json!({"type":"array","prefixItems":[namespace,string],"minItems":2,"maxItems":2})),
            "data":nullable(r("data")),"legacyData":r("legacyData"),"receipt":nullable(r("receipt")),
            "error":r("error"),"readiness":nullable(r("readiness"))
        }),
        &["legacyData"],
    );
    let success = if feedback {
        object(
            json!({"id":string,"strength":number,"stability":number,"phase":string,"isPermastore":boolean}),
        )
    } else {
        object(json!({"id":string,"deleted":boolean}))
    };
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object",
        "description":"API1 success or complete typed lifecycle failure. Receipt shape does not establish canonical proof validity or current-operation attribution.",
        "$defs":defs,"oneOf":[success,failure]})
}
