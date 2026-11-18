//! Existing delete output only. Create's arbitrary-object error lane stays open.
use super::{object, object_optional, receipt};
use serde_json::{Value, json};

pub(super) fn delete_output() -> Value {
    let mut defs = receipt::definitions();
    let string = json!({"type":"string"});
    let boolean = json!({"type":"boolean"});
    let uuid = json!({"type":"string","format":"uuid","not":{"const":"00000000-0000-0000-0000-000000000000"}});
    let mut retained = defs["retainedReceipt"].clone();
    retained["properties"]["storeId"] = uuid.clone();
    retained["properties"]["operationId"] = uuid.clone();
    retained["properties"]["receiptVersion"] = json!({"const":1});
    retained["properties"]["committedGeneration"] =
        json!({"allOf":[receipt::reference("generation"),{"not":{"const":"0"}}]});
    retained["properties"]["outcome"] = json!({"oneOf":[
        object(json!({"kind":{"const":"nativeNamespacePurgeStarted"},"transition":receipt::reference("purgeStarted")})),
        object(json!({"kind":{"const":"nativeNamespacePurgeAdvanced"},"transition":receipt::reference("purgeAdvanced")}))
    ]});
    defs.insert("purgeReceipt".into(), retained);
    let success = object(
        json!({"name":string,"id":receipt::uint(u32::MAX.into()),"deleted":boolean,
        "memoriesDeleted":receipt::uint(u64::MAX),"tombstonesPurged":receipt::uint(u64::MAX),
        "edgesRemoved":receipt::uint(u64::MAX),"ftsRowsRemoved":receipt::uint(u64::MAX),"vectorFileRemoved":boolean}),
    );
    let failure = |code: &str, unknown: bool| {
        let mut properties = json!({"error":{"const":code},"commitState":{"const":if unknown {"unknown"} else {"committed"}},
            // Runtime bounds UTF8 bytes. This safe character upper bound does
            // not claim to implement that byte/proof validator in JSON Schema.
            "message":{"type":"string","maxLength":512},"receipt":receipt::reference("purgeReceipt"),
            "commitEvidenceScope":{"enum":["purgeStart","purgeStep","purgeComplete"]}});
        if code == "PURGE_METRICS_UNAVAILABLE" {
            properties["commitEvidenceScope"] = json!({"const":"purgeComplete"});
        }
        let mut shape = if unknown {
            // structured_failure returns the original parser-accepted UUID
            // spelling (including simple/URN/braced), unlike receipt fields.
            properties["operationId"] = receipt::nullable(json!({"type":"string",
                "description":"Original Rust UUID-parser spelling, not necessarily canonical dashed format; this schema does not duplicate that parser."}));
            object_optional(properties, &["receipt", "commitEvidenceScope"])
        } else {
            object(properties)
        };
        shape["dependentRequired"] =
            json!({"receipt":["commitEvidenceScope"],"commitEvidenceScope":["receipt"]});
        shape["allOf"] = json!([
            {"if":{"required":["commitEvidenceScope"],"properties":{"commitEvidenceScope":{"const":"purgeStart"}}},
             "then":{"properties":{"receipt":{"properties":{"outcome":{"properties":{"kind":{"const":"nativeNamespacePurgeStarted"}}}}}}}},
            {"if":{"required":["commitEvidenceScope"],"properties":{"commitEvidenceScope":{"enum":["purgeStep","purgeComplete"]}}},
             "then":{"properties":{"receipt":{"properties":{"outcome":{"properties":{"kind":{"const":"nativeNamespacePurgeAdvanced"}}}}}}}},
            {"if":{"required":["commitEvidenceScope"],"properties":{"commitEvidenceScope":{"const":"purgeComplete"}}},
             "then":{"properties":{"receipt":{"properties":{"outcome":{"properties":{"transition":{"properties":{"stage":{"const":"complete"}}}}}}}}}},
            {"if":{"required":["commitEvidenceScope"],"properties":{"commitEvidenceScope":{"const":"purgeStep"}}},
             "then":{"properties":{"receipt":{"properties":{"outcome":{"properties":{"transition":{"properties":{"stage":{"enum":["deleting","cleanup"]}}}}}}}}}}
        ]);
        shape
    };
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","$defs":defs,
        "description":"API1 delete success or validated structured purge error. Known prior receipt evidence does not commit an unknown child; no inferred readiness or physical metrics. Schema shape is not receipt authenticity or canonical proof validation.",
        "oneOf":[success,failure("PURGE_METRICS_UNAVAILABLE",false),failure("SERVICE_UNAVAILABLE",false),failure("COMMIT_OUTCOME_UNKNOWN",true)]})
}
