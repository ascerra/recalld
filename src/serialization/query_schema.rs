//! Schemas for the prepared query consumers, not live capability advertising.
//! Storage validators still own receipt semantics and cross-field invariants.
use serde_json::{Value, json};

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn all(properties: Value) -> Value {
    let keys: Vec<_> = properties
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    object(properties.clone(), &keys)
}
fn reference(name: &str) -> Value {
    json!({"$ref":format!("#/$defs/{name}")})
}
fn array(item: Value) -> Value {
    json!({"type":"array","items":item})
}
fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}

pub(crate) fn output_schema(similar: bool) -> Value {
    let number = json!({"type":"number"});
    let boolean = json!({"type":"boolean"});
    let string = json!({"type":"string"});
    let count = json!({"type":"integer","minimum":0});
    let time = json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX});
    let namespace = json!({"type":"integer","minimum":1,"maximum":u32::MAX});
    let uuid = json!({"type":"string","format":"uuid"});
    // Lexical decimal range, including the 19-digit signed-counter boundary.
    let max = "9223372036854775807";
    let mut alternatives = vec!["0".to_owned(), "[1-9][0-9]{0,17}".to_owned()];
    for (i, digit) in max.bytes().enumerate() {
        let lower = if i == 0 { b'1' } else { b'0' };
        if digit > lower {
            alternatives.push(format!(
                "{}[{}-{}][0-9]{{{}}}",
                &max[..i],
                lower as char,
                (digit - 1) as char,
                max.len() - i - 1
            ));
        }
    }
    alternatives.push(max.into());
    // `$` admits a trailing newline in common JSON Schema regex engines.
    let generation =
        json!({"type":"string","pattern":format!("^({})(?![\\s\\S])", alternatives.join("|"))});
    let phase = json!({"enum":["full","summary","ghost","tombstone"]});
    let version = json!({"enum":["legacy_access_v1","explicit_feedback_v2"]});
    let access = json!({"enum":["direct_retrieval","associative_retrieval"]});
    let reason = json!({"enum":["missingRecord","missingStructure","foreignNamespace","branch","cycle","hopLimit","workLimit","ineligible"]});
    let mut defs = serde_json::Map::new();
    defs.insert("generation".into(), generation.clone());
    defs.insert(
        "generationRef".into(),
        all(json!({"storeId":uuid,"generation":generation})),
    );
    defs.insert(
        "namespace".into(),
        all(json!({"namespaceId":namespace,"namespace":string})),
    );
    defs.insert("scope".into(), json!({"oneOf":[reference("namespace"),all(json!({"namespaceIds":array(namespace.clone())}))]}));
    defs.insert("version".into(), json!({"oneOf":[all(json!({"status":{"enum":["current","historical"]}})),all(json!({"status":{"const":"unresolved"},"reason":reason}))]}));
    defs.insert("unresolved".into(), object(json!({"missingRecord":count,"missingStructure":count,"foreignNamespace":count,"branch":count,"cycle":count,"hopLimit":count,"workLimit":count,"ineligible":count}), &[]));
    defs.insert("labels".into(), all(json!({"entities":array(string.clone()),"topics":array(string.clone()),"emotions":array(string.clone()),"provenance":{"enum":["native_v2","legacy_retained_only"]}})));
    defs.insert("direct".into(), all(json!({"vector":nullable(number.clone()),"fts":nullable(number.clone()),"fused":nullable(number.clone()),"minScore":number,"disposition":{"enum":["passed","rejected","auxiliary"]}})));
    defs.insert("own".into(), all(json!({"formula":{"enum":["direct","graph","entityRecall","metadata"]},"direct":reference("direct"),"entityOverlap":number,"entityRecall":number,"ftsSignal":number,"rawRetrievability":number,"effectiveRetrievability":number,"activation":nullable(number.clone()),"activationDepth":nullable(count.clone()),"baseScore":number,"temporalFactor":number,"rifMultiplier":{"const":1.0},"score":number})));
    defs.insert("explanation".into(), all(json!({"rankingProfile":{"const":"baseline-v1"},"retrievalSemanticsVersion":{"const":2},"version":nullable(reference("version")),"rawRetrievability":number,"effectiveRetrievability":number,"own":nullable(reference("own")),"rejectedDirect":nullable(reference("direct")),"inheritedFloor":nullable(number.clone()),"inheritedSources":{"type":"array","maxItems":crate::search::MAX_EXPLANATION_SOURCES,"items":all(json!({"source":uuid,"own":reference("own")}))},"inheritedSourcesOmitted":count,"rifMultiplier":{"const":1.0},"finalScore":number})));
    let mut hit = json!({"memoryId":uuid,"createdAt":time,"lastAccessedAt":time,"score":number,"ftsScore":number,"compositeScore":number,"retrievability":number,"effectiveR":number,"phase":phase,"summary":string,"fullText":string,"tags":array(string.clone()),"edgeCount":count,"activationScore":number,"isPermastore":boolean,"stability":number,"namespace":string,"namespaceId":namespace,"labels":reference("labels"),"embedding":{"type":"array","maxItems":8192,"items":number}});
    let required = [
        "memoryId",
        "createdAt",
        "lastAccessedAt",
        "retrievability",
        "effectiveR",
        "phase",
        "tags",
        "edgeCount",
        "isPermastore",
        "stability",
        "namespace",
        "namespaceId",
    ];
    defs.insert("similarExplanation".into(), all(json!({"algorithm":{"const":"cosineMax"},"ownCosine":nullable(number.clone()),"ownAdmission":{"enum":["missing","rejectedByMinScore","notAdmitted","admitted"]},"minScore":nullable(json!({"type":"number","minimum":0,"maximum":1})),"admittedOwn":nullable(number.clone()),"inheritedFloor":nullable(number.clone()),"origins":{"type":"array","maxItems":64,"items":all(json!({"originId":uuid,"cosine":number}))},"omittedOrigins":count,"finalSimilarity":number,"version":nullable(reference("version"))})));
    let mut similar_hit = hit.clone();
    similar_hit["explanation"] = reference("similarExplanation");
    defs.insert("similarHit".into(), object(similar_hit, &required));
    hit["version"] = nullable(reference("version"));
    hit["graphDepth"] = nullable(count.clone());
    hit["related"] = array(all(json!({"id":uuid,"edgeType":string,"weight":number})));
    hit["explanation"] = reference("explanation");
    let mut ranked_required = required.to_vec();
    ranked_required.extend(["version", "graphDepth", "related"]);
    defs.insert("hit".into(), object(hit, &ranked_required));
    defs.insert("neighbor".into(), all(json!({"memoryId":uuid,"namespaceId":namespace,"namespace":string,"summary":string,"fullText":nullable(string.clone()),"labels":nullable(reference("labels")),"tags":array(string.clone()),"connectedTo":uuid,"edgeType":string,"weight":number})));
    defs.insert("channel".into(), all(json!({"participation":{"enum":["notRequested","completed"]},"offered":nullable(count.clone()),"admitted":nullable(count.clone()),"examinedUnit":nullable(json!({"enum":["vectorEntries","entityPostings","ftsRows","scoredVectors","canonicalRecords"]})),"examined":nullable(count.clone()),"eligible":nullable(count.clone()),"exhausted":nullable(boolean.clone()),"candidateLimited":nullable(boolean.clone()),"budgetExhausted":nullable(boolean.clone()),"returnedOwn":count,"returnedInherited":count})));
    defs.insert("canonicalEligibility".into(), all(json!({"examinedUnit":{"const":"canonicalRecords"},"examined":count,"passed":count,"firstRejections":object(json!({"namespace":count,"tombstone":count,"ghost":count,"phase":count,"strength":count,"permastore":count,"labels":count,"requiredTag":count,"excludedTag":count}), &[])})));
    defs.insert("coverage".into(), all(json!({"semanticsVersion":{"const":2},"versionMode":{"enum":["current","history"]},"budgetProfile":{"const":"retrieval-v2-default"},"canonicalEligibility":nullable(reference("canonicalEligibility")),"channels":all(json!({"vector":reference("channel"),"fts":reference("channel"),"entity":reference("channel"),"metadata":reference("channel"),"graph":reference("channel")})),"ftsAvailability":{"enum":["notRequested","available","unavailable"]},"graph":all(json!({"expandedStates":count,"examinedEdges":count,"stateLimited":boolean,"edgeLimited":boolean})),"supersession":all(json!({"examinedEdges":count,"unresolved":reference("unresolved"),"workLimited":boolean})),"metadataCandidateLimited":boolean,"resultLimited":boolean,"supplemental":all(json!({"requested":boolean,"neighborhoodsExamined":count,"edgesExamined":count,"supersessionEdgesExamined":count,"supersessionWorkLimited":boolean,"neighborhoodsSkippedEdgeBudget":count,"neighborhoodsUnexaminedStateBudget":count,"depthLimitedSources":count,"ineligibleNeighbors":count,"historicalNeighborsOmitted":count,"unresolvedNeighbors":reference("unresolved"),"eligibleUniqueNeighbors":count,"neighborLimitApplied":boolean}))})));
    defs.insert("similarCoverage".into(), all(json!({"semanticsVersion":{"const":2},"versionMode":{"enum":["current","history"]},"searchedNamespaceIds":array(namespace.clone()),"excludedNamespaces":array(all(json!({"namespaceId":namespace,"reason":{"enum":["unknownProfile","differentProfile","differentDimension"]}}))),"examinedRecords":count,"eligibleRecords":count,"scoredRecords":count,"scoreAdmitted":count,"resolvedCandidates":count,"resultLimited":boolean,"supersession":all(json!({"examinedEdges":count,"unresolved":reference("unresolved")}))})));
    defs.insert("data".into(), all(if similar {json!({"results":{"type":"array","maxItems":100,"items":reference("similarHit")},"totalMatches":count,"sourceNamespace":reference("namespace"),"coverage":reference("similarCoverage")})} else {json!({"results":{"type":"array","maxItems":100,"items":reference("hit")},"neighbors":array(reference("neighbor")),"totalMatches":count,"coverage":reference("coverage")})}));
    defs.insert("lifecycle".into(), all(json!({"observedAt":time,"retrievedAt":nullable(time.clone()),"reinforcedAt":nullable(time.clone()),"decayAnchorAt":time,"lifecycleVersion":version,"lifecycleUpdatedAt":time,"timestampOrigin":{"enum":["native","legacy_inferred"]},"phaseChangedAt":time,"policyRevision":generation,"recordGeneration":generation})));
    defs.insert("state".into(), all(json!({"lifecycle":reference("lifecycle"),"createdAt":time,"lastAccessedAt":time,"stability":{"type":"number","exclusiveMinimum":0},"difficulty":{"type":"number","minimum":1,"maximum":10},"strength":{"type":"number","minimum":0,"maximum":1},"decayStrength":{"type":"number","minimum":0,"maximum":1},"phase":phase,"isPermastore":boolean,"hasSummary":boolean,"hasText":boolean})));
    let hash = json!({"type":"array","minItems":32,"maxItems":32,"items":{"type":"integer","minimum":0,"maximum":255}});
    // An evicted event can predate exposure and retain another access kind.
    let event = all(
        json!({"timestamp":time,"kind":{"enum":["direct_retrieval","associative_retrieval","decay_sweep","manual_reinforcement"]}}),
    );
    defs.insert("change".into(), all(json!({"memoryId":uuid,"kind":access,"before":reference("state"),"after":reference("state"),"historyBefore":hash,"historyAfter":hash,"evictedEvent":nullable(event)})));
    defs.insert("transition".into(), all(json!({"namespaceId":namespace,"lifecycleVersion":version,"policyRevision":generation,"previousGeneration":generation,"captureGeneration":reference("generationRef"),"sampledAt":time,"records":array(reference("change")),"feedback":{"type":"null"}})));
    defs.insert("receipt".into(), all(json!({"storeId":uuid,"operationId":uuid,"committedGeneration":generation,"receiptVersion":{"const":1},"replayed":boolean,"outcome":{"oneOf":[all(json!({"kind":{"const":"exposuresRecorded"},"accesses":array(all(json!({"memoryId":uuid,"kind":access,"accessedAt":time})))})),all(json!({"kind":{"const":"lifecycleExposureRecorded"},"transition":reference("transition")}))]}})));
    // Unverified remote evidence is not this query's receipt. This schema
    // validates only the exact header and an opaque outcome object; the Rust
    // decoder additionally requires the complete WriteReceipt codec and exact
    // canonical roundtrip. Neither check proves canonical storage/authenticity.
    defs.insert("receivedUnverifiedReceipt".into(), all(json!({
        "storeId":uuid,"operationId":uuid,"committedGeneration":generation,
        "receiptVersion":{"const":1},"replayed":boolean,
        "outcome":{"type":"object","properties":{"kind":{"type":"string"}},
            "required":["kind"],"additionalProperties":true,
            "description":"Opaque unverified variant payload, preserved completely. This schema does not validate the outcome variant or prove current-write commitment. Runtime requires full Rust WriteReceipt decoding and exact canonical roundtrip."}
    })));
    defs.insert("readiness".into(), all(json!({"storeId":uuid,"committedGeneration":generation,"ready":boolean,"pendingWork":count,"derived":array(all(json!({"name":string,"appliedGeneration":generation,"ready":boolean})))})));
    defs.insert("error".into(), object(json!({"category":{"enum":["invalid_input","not_found","conflict","unavailable","partial","internal"]},"code":string,"message":string,"field":string,"retryable":boolean,"commitState":{"enum":["notCommitted","committed","unknown"]},"operationId":uuid}), &["category","code","message","retryable","commitState"]));
    let mut schema = object(
        json!({"contractVersion":{"const":2},"status":{"enum":["ok","error"]},"scope":reference("scope"),"data":reference("data"),"receipt":reference("receipt"),"observationReceipts":array(reference("receipt")),"readGeneration":reference("generationRef"),"error":reference("error"),"readiness":reference("readiness"),"warnings":array(string),
            "receivedEvidence":all(json!({"receipts":{"type":"array","minItems":1,"items":reference("receivedUnverifiedReceipt")}}))}),
        &["contractVersion", "status", "warnings"],
    );
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["$defs"] = Value::Object(defs);
    schema["allOf"] = json!([
        {"if":{"properties":{"status":{"const":"ok"}}},"then":{"required":["scope","data","readGeneration"],"not":{"required":["error"]}},"else":{"required":["error"],"not":{"required":["data"]}}},
        {"if":{"required":["receipt"]},"then":{"required":["observationReceipts"]}},
        {"if":{"required":["receivedEvidence"]},"then":{
            "properties":{"status":{"const":"error"},"warnings":{"contains":{"const":"UNVERIFIED_RECEIVED_EVIDENCE"}},
                "error":{"properties":{"commitState":{"enum":["notCommitted","unknown"]}},"not":{"required":["operationId"]}}},
            "not":{"anyOf":[{"required":["data"]},{"required":["receipt"]},{"required":["observationReceipts"]},{"required":["readiness"]},{"required":["readGeneration"]},{"required":["scope"]}]}
        }}
    ]);
    schema
}

#[cfg(test)]
pub(crate) fn record_fixture(name: &str, similar: bool, value: &Value) {
    if let Some(dir) = std::env::var_os("RECALLD_QUERY_SCHEMA_FIXTURES") {
        let path = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join(format!("{name}.json")),
            serde_json::to_vec(&json!({"schema":output_schema(similar),"value":value})).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_query_prepared_schema_documents() {
        let mut cases = Vec::new();
        for similar in [false, true] {
            let schema = output_schema(similar);
            assert_eq!(schema["additionalProperties"], false);
            assert_eq!(schema["$defs"]["similarHit"]["additionalProperties"], false);
            assert!(
                schema["$defs"]["similarHit"]["properties"]
                    .get("explanation")
                    .is_some()
            );
            let input = if similar {
                super::super::query::similar_input_schema(true)
            } else {
                super::super::query::input_schema(true)
            };
            let base = if similar {
                json!({"contractVersion":2,"id":"00000000-0064-7000-8000-000000000001"})
            } else {
                json!({"contractVersion":2,"queryMode":"metadata_only","tags":["allow"]})
            };
            let mut values = vec![base.clone()];
            for (key, value) in [
                ("explain", json!(true)),
                ("explain", Value::Null),
                ("recordAccess", json!(false)),
                ("recordAccess", Value::Null),
                ("limit", json!(0)),
                ("limit", json!(101)),
                ("limit", json!(1)),
                ("unexpected", json!(true)),
                ("namespace", Value::Null),
                ("namespace", json!(true)),
                ("versionMode", json!("history")),
            ] {
                let mut v = base.clone();
                v[key] = value;
                values.push(v);
            }
            if !similar {
                for suffix in ["", "\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
                    values.push(json!({"contractVersion":2,"embedding":[1],"embeddingProfileId":format!("ep1:{}{suffix}","a".repeat(64))}));
                }
                values.extend([json!({"contractVersion":2}),json!({"contractVersion":2,"query":"hello"}),json!({"contractVersion":2,"query":"   "}),json!({"contractVersion":2,"queryMode":"metadata_only","tags":[]}),json!({"contractVersion":2,"queryMode":"metadata_only","query":"hello","tags":["allow"]}),json!({"contractVersion":2,"queryMode":"metadata_only","minStrength":0}),json!({"contractVersion":2,"queryMode":"metadata_only","permastoreOnly":false}),json!({"contractVersion":2,"queryMode":"metadata_only","permastoreOnly":true})]);
            }
            for value in values {
                let valid = if similar {
                    super::super::query::SimilarRequest::from_mcp(&value).is_ok()
                } else {
                    super::super::query::QueryRequest::from_mcp(&value, "default").is_ok()
                };
                cases.push(json!({"schema":input,"value":value,"valid":valid}));
            }
            for suffix in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
                let value = format!("1{suffix}");
                assert!(crate::storage::control::Generation::try_from(value.clone()).is_err());
                cases.push(
                    json!({"schema":schema["$defs"]["generation"],"value":value,"valid":false}),
                );
            }
        }
        if let Some(dir) = std::env::var_os("RECALLD_QUERY_SCHEMA_FIXTURES") {
            let path = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                path.join("input-cases.json"),
                serde_json::to_vec(&cases).unwrap(),
            )
            .unwrap();
        }
    }
}
