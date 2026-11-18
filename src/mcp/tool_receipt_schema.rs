//! Typed retained receipt codecs. Shape is not canonical proof or attribution.
use super::{array, lifecycle, object, object_optional};
use serde_json::{Map, Value, json};

pub(super) fn reference(name: &str) -> Value {
    json!({"$ref":format!("#/$defs/{name}")})
}
pub(super) fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}
pub(super) fn uint(max: u64) -> Value {
    json!({"type":"integer","minimum":0,"maximum":max})
}

pub(super) fn definitions() -> Map<String, Value> {
    let mut defs = lifecycle::output(false)["$defs"]
        .as_object()
        .unwrap()
        .clone();
    let r = reference;
    let string = json!({"type":"string"});
    let boolean = json!({"type":"boolean"});
    let uuid = json!({"type":"string","format":"uuid"});
    let u32n = uint(u32::MAX.into());
    let u64n = uint(u64::MAX);
    let time = json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX});
    let number = defs["state"]["properties"]["stability"].clone();
    let hash = defs["content"]["properties"]["summarySha256"].clone();
    let version = defs["lifecycle"]["properties"]["lifecycleVersion"].clone();
    let phase = defs["state"]["properties"]["phase"].clone();
    let access = json!({"enum":["direct_retrieval","associative_retrieval","decay_sweep","manual_reinforcement"]});
    let stage = json!({"enum":["deleting","cleanup","complete"]});
    let triple = json!({"type":"array","items":number,"minItems":3,"maxItems":3});
    defs.insert(
        "recordChange".into(),
        defs["recordTransition"]["properties"]["records"]["items"].clone(),
    );
    defs.insert(
        "edgeProof".into(),
        defs["recordTransition"]["properties"]["edges"]["items"].clone(),
    );
    defs.insert(
        "certification".into(),
        defs["vectorExecution"]["properties"]["certification"].clone(),
    );
    defs.insert(
        "event".into(),
        object(json!({"timestamp":time,"kind":access})),
    );
    defs.insert(
        "batchError".into(),
        object(json!({"code":string,"field":nullable(string.clone())})),
    );
    defs.insert("policy".into(), object(json!({
        "initialStability":number,"defaultDifficulty":number,"thresholds":triple,
        "permastoreThreshold":number,"desiredRetention":number,"decayRateMultiplier":nullable(number.clone())
    })));
    defs.insert(
        "decayEvidence".into(),
        object(json!({"multiplier":number,"maxBonus":number,"thresholds":triple,"bonus":number})),
    );
    defs.insert("inheritedBinding".into(), object(json!({"version":u32n,"multiplierBits":u32n,"adoption":r("generationRef"),"operationId":uuid})));
    defs.insert("profile".into(), object(json!({
        "identityVersion":u32n,"providerKind":{"enum":["openai","ollama","bedrock","passthrough"]},
        "modelId":string,"modelRevision":nullable(string.clone()),"dimension":u32n,
        "documentPreprocessing":object(json!({"version":string,"prefix":string})),
        "queryPreprocessing":object(json!({"version":string,"prefix":string})),"normalization":{"const":"l2_unit_v1"}
    })));
    defs.insert("namespaceSchema".into(), object_optional(json!({
        "namespaceSchemaVersion":u32n,"lifecycleVersion":version,"policyRevision":r("generation"),
        "embeddingBinding":{"oneOf":[object(json!({"state":{"const":"unknownLegacy"}})),object(json!({
            "state":{"const":"bound"},"profileId":string,"descriptor":r("profile"),
            "provenance":{"enum":["native_v2","operator_attestation","reembedding"]}
        }))]},
        "inheritedDecay":r("inheritedBinding"),"vectorCertification":r("certification"),
        "requiredFeatures":array(string.clone()),
        // This declared BTreeMap<String, Value> is intentionally extensible.
        "extensions":{"type":"object","additionalProperties":true}
    }), &["inheritedDecay","vectorCertification"]));
    defs.insert("namespaceProof".into(), object(json!({
        "namespaceId":u32n,"name":string,"embeddingDim":u32n,"createdAt":time,"policy":r("policy"),"schema":r("namespaceSchema")
    })));
    defs.insert(
        "deletion".into(),
        object(json!({
            "memoryId":uuid,"namespaceId":u32n,"before":r("recordState"),"history":array(r("event"))
        })),
    );
    defs.insert(
        "peerChange".into(),
        object(json!({"namespaceId":u32n,"change":r("recordChange")})),
    );
    defs.insert("purgeChunk".into(), object(json!({"deletions":array(r("deletion")),"peers":array(r("peerChange")),"removedEdges":array(r("edgeProof"))})));
    defs.insert(
        "requestContext".into(),
        object(json!({"proofVersion":u32n,"contractVersion":u32n})),
    );
    defs.insert("purgeStarted".into(), object_optional(json!({
        "request":object(json!({"context":r("requestContext"),"force":boolean,"expectedGeneration":nullable(r("generation"))})),
        "previousGeneration":r("generation"),"beforeNamespace":r("namespaceProof")
    }), &["request"]));
    defs.insert("purgeAdvanced".into(), object(json!({
        "parentOperationId":uuid,"namespaceId":u32n,"previousGeneration":r("generation"),"sequence":u64n,"stage":stage,
        "chunk":nullable(r("purgeChunk"))
    })));
    defs.insert("namespaceCreated".into(), object_optional(json!({
        "vectorCertification":r("certification"),"namespaceId":u32n,"previousMaxId":u32n,"previousGeneration":r("generation"),
        "name":string,"createdAt":time,"profile":r("profile"),"policy":r("policy"),"inheritedDecay":r("inheritedBinding")
    }), &["vectorCertification","inheritedDecay"]));
    defs.insert("policyUpdated".into(), object_optional(json!({
        "namespaceId":u32n,"lifecycleVersion":version,"previousRevision":r("generation"),"nextRevision":r("generation"),
        "previousGeneration":r("generation"),"captureGeneration":r("generationRef"),"before":r("policy"),"after":r("policy"),
        "clearedInheritedDecay":r("inheritedBinding"),"recordCount":u64n,"recordTokenDigest":string
    }), &["clearedInheritedDecay"]));
    defs.insert("policyUnchanged".into(), object(json!({
        "namespaceId":u32n,"lifecycleVersion":version,"policyRevision":r("generation"),"previousGeneration":r("generation"),
        "captureGeneration":r("generationRef"),"policy":r("policy")
    })));
    let boundary = json!({"namespaceId":u32n,"memoryId":uuid,"lifecycleVersion":version,"policyRevision":r("generation"),
        "previousGeneration":r("generation"),"captureGeneration":r("generationRef"),"sampledAt":time,"policy":r("decayEvidence")});
    let mut phase_write = boundary.clone();
    phase_write["change"] = r("recordChange");
    defs.insert("phaseWrite".into(), object(phase_write));
    let mut hard_delete = boundary;
    hard_delete["deletion"] = r("deletion");
    hard_delete["peers"] = array(r("peerChange"));
    hard_delete["removedEdges"] = array(r("edgeProof"));
    defs.insert("hardDelete".into(), object(hard_delete));
    let graph = json!({"namespaceId":u32n,"lifecycleVersion":version,"policyRevision":r("generation"),
        "captureGeneration":r("generationRef"),"previousGeneration":r("generation"),"edges":array(r("edgeProof")),"records":array(r("recordChange"))});
    defs.insert("graphUnchanged".into(), object(graph.clone()));
    let mut graph_write = graph;
    graph_write["sampledAt"] = time.clone();
    graph_write["edges"] = array(object(
        json!({"before":nullable(r("edgeProof")),"after":r("edgeProof")}),
    ));
    defs.insert("graphWrite".into(), object(graph_write));
    let mut store = defs["recordTransition"]["properties"].clone();
    store.as_object_mut().unwrap().remove("phaseTimeRule");
    store["parentId"] = nullable(uuid.clone());
    store["supersedes"] = nullable(uuid.clone());
    store["settings"] = object(
        json!({"autolinkEnabled":boolean,"autoLinkThresholdBits":u64n,"maxAutoLinks":u32n,
        "maxEntityLinks":u32n,"temporalWindowMs":u64n,"maxTemporalLinks":u32n}),
    );
    store["checkDuplicates"] = boolean.clone();
    store["advisory"] = nullable(array(object(json!({"memoryId":uuid,"scoreBits":u32n}))));
    defs.insert(
        "storeWrite".into(),
        object_optional(store, &["vectorExecution"]),
    );
    defs.insert("phaseCounts".into(), object(json!({"fullToSummary":u64n,"summaryToGhost":u64n,"ghostToDelete":u64n,"retained":u64n})));
    defs.insert("inheritedPreview".into(), object(json!({
        "version":u32n,"source":r("generationRef"),"namespaceId":u32n,"policyRevision":r("generation"),"lifecycleVersion":version,
        "prior":nullable(r("inheritedBinding")),"multiplierBits":u32n,"sampledAt":time,"examinedRows":u64n,"recordCount":u64n,
        "examinedEdges":u64n,"canonicalDigest":hash,"candidate":r("phaseCounts"),"token":hash
    })));
    defs.insert(
        "inheritedAdopted".into(),
        object(
            json!({"contractVersion":u32n,"preview":r("inheritedPreview"),
        "after":r("inheritedBinding"),"nextRevision":r("generation"),"recordTokenDigest":string}),
        ),
    );
    defs.insert("reembedded".into(), object(json!({"version":u32n,"contractVersion":u32n,"source":r("generationRef"),
        "sourceOrigins":array(r("generationRef")),"beforeNamespace":r("namespaceProof"),"certification":r("certification"),
        "beforeInventoryRoot":hash,"records":array(object(json!({"record":r("recordChange"),"document":r("document")})))})));
    defs.insert("policyTransition".into(), object(json!({"namespaceId":u32n,"from":version,"to":version,
        "previousRevision":r("generation"),"nextRevision":r("generation"),"previousGeneration":r("generation"),
        "recordCount":u64n,"recordTokenDigest":string})));
    defs.insert(
        "textGeneration".into(),
        object(json!({"id":uuid,"storeId":uuid,"filename":string,
        "formatVersion":uint(u16::MAX.into()),"sealedLength":u64n,"sealedSha256":hash})),
    );
    defs.insert("textRelocation".into(), object(json!({"proofVersion":u32n,"source":r("generationRef"),
        "before":r("textGeneration"),"after":r("textGeneration"),"recordCount":u64n,"retainedCount":u64n,
        "beforeRefsSha256":hash,"afterRefsSha256":hash,"recordsSha256":hash,"copiedTextSha256":hash})));

    let mut outcomes = vec![];
    for (kind, def) in [
        ("vectorsReembedded", "reembedded"),
        ("inheritedDecayAdopted", "inheritedAdopted"),
        ("nativeNamespacePurgeStarted", "purgeStarted"),
        ("nativeNamespacePurgeAdvanced", "purgeAdvanced"),
        ("nativeNamespacePolicyUnchanged", "policyUnchanged"),
        ("nativeNamespacePolicyUpdated", "policyUpdated"),
        ("nativeNamespaceCreated", "namespaceCreated"),
        ("nativePhaseMaterialized", "phaseWrite"),
        ("nativeMemoryHardDeleted", "hardDelete"),
        ("nativeGraphUnchanged", "graphUnchanged"),
        ("nativeGraphChanged", "graphWrite"),
        ("nativeMemoryInsertedWithLinks", "storeWrite"),
        ("nativeMemoryInserted", "recordTransition"),
        ("nativeMemoryTombstoned", "recordTransition"),
        ("lifecycleExposureRecorded", "feedbackTransition"),
        ("lifecycleFeedbackApplied", "feedbackTransition"),
        ("lifecyclePolicyMigrated", "policyTransition"),
        ("nativeTextGenerationSelected", "textRelocation"),
    ] {
        outcomes.push(object(json!({"kind":{"const":kind},"transition":r(def)})));
    }
    for (kind, mut fields) in [
        (
            "batchReserved",
            json!({"batch_kind":{"enum":["insert","forget"]},"item_count":u32n,"caller_hash":hash,"resolution_hash":hash}),
        ),
        (
            "batchItemRejected",
            json!({"batch_operation_id":uuid,"index":u32n,"error":r("batchError")}),
        ),
        (
            "lifecycleMaterialized",
            json!({"memoryId":uuid,"fromPhase":phase,"toPhase":phase,"strengthBits":u32n,"decayStrengthBits":u32n,"removedFieldsMask":uint(255)}),
        ),
        (
            "namespacePurgeStarted",
            json!({"namespace_id":u32n,"name_hash":hash}),
        ),
        (
            "namespacePurgeAdvanced",
            json!({"parent_operation_id":uuid,"namespace_id":u32n,"sequence":u64n,"stage":stage,
            "removed":u64n,"tombstones_removed":u64n,"edges_removed":u64n,"last_id":nullable(uuid.clone())}),
        ),
        (
            "memoryTombstoned",
            json!({"memoryId":uuid,"status":{"enum":["tombstoned","already_tombstoned"]}}),
        ),
        (
            "memoryInserted",
            json!({"memoryId":uuid,"createdAt":time,"initialStabilityBits":u32n,"embeddingDim":u32n,"edgesPut":u64n}),
        ),
        (
            "namespaceCreated",
            json!({"namespaceId":u32n,"createdAt":time}),
        ),
        ("namespacePolicyUpdated", json!({"namespaceId":u32n})),
        (
            "exposuresRecorded",
            json!({"accesses":array(object(json!({"memoryId":uuid,"kind":access,"accessedAt":time})))}),
        ),
        (
            "legacyFeedbackRecorded",
            json!({"memoryId":uuid,"quality":uint(255),"appliedAt":time}),
        ),
        (
            "graphChanged",
            json!({"edges_put":u64n,"edges_removed":u64n,"memories_removed":u64n}),
        ),
        (
            "schemaMigrated",
            json!({"migrationId":uuid,"recordCount":u64n,"attestedNamespaces":u64n}),
        ),
        (
            "textGenerationSelected",
            json!({"generationId":uuid,"formatVersion":uint(u16::MAX.into()),"entriesKept":u64n}),
        ),
        ("accessRecorded", json!({"memoryId":uuid,"accessedAt":time})),
    ] {
        fields["kind"] = json!({"const":kind});
        outcomes.push(object(fields));
    }
    let kinds: Vec<_> = outcomes
        .iter()
        .map(|o| o["properties"]["kind"]["const"].clone())
        .collect();
    let header = json!({"storeId":uuid,"operationId":uuid,"committedGeneration":r("generation"),
        "receiptVersion":u32n,"replayed":boolean,"outcome":{"oneOf":outcomes}});
    defs.insert("retainedReceipt".into(), object(header.clone()));
    let mut unverified = header;
    unverified["outcome"] = json!({"type":"object","properties":{"kind":{"enum":kinds}},"required":["kind"],"additionalProperties":true,
        "description":"Opaque supported variant payload in explicitly unverified received evidence; not proof validity or current-write commitment."});
    defs.insert("unverifiedReceipt".into(), object(unverified));
    defs
}
