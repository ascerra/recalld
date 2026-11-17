//! Actual owner receipts, then explicitly malformed typed peer specimens.
use super::*;
use crate::serialization::lifecycle_api1 as api1;
use crate::storage::control::{MutationOutcome, WriteReceipt};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_lifecycle_schema_retained_codec_is_not_canonical_proof() {
    for operation in [Operation::Feedback, Operation::Forget] {
        let (f, d, clock, bridge, state) = setup().await;
        let request = api1::request(d.parent, 3, None);
        let reply = bridge
            .storage
            .api1_lifecycle(request.clone(), operation)
            .await;
        assert!(reply.value.is_some(), "actual owner success: {reply:?}");
        let original = reply.lifecycle.as_ref().unwrap().receipt.clone().unwrap();
        let mut malformed = original.clone();
        match &mut malformed.outcome {
            MutationOutcome::LifecycleFeedbackApplied { transition } => {
                // FiniteF32 decodes these; canonical LifecycleState does not.
                transition.records[0].before.stability = (-7_f32).try_into().unwrap();
                transition.records[0].after.difficulty = (-9_f32).try_into().unwrap();
                transition.feedback.as_mut().unwrap().decay_rate_multiplier =
                    (-3_f32).try_into().unwrap();
            }
            MutationOutcome::NativeMemoryTombstoned { transition } => {
                transition.records.push(transition.records[0].clone());
                transition
                    .edges
                    .push(crate::storage::native_records::NativeEdgeProof {
                        source: d.parent,
                        target: d.parent,
                        edge_type: crate::model::EdgeType::Associative,
                        weight_bits: u32::MAX,
                        auto_created: true,
                        created_at: u64::MAX,
                    });
                transition.vector_execution =
                    Some(crate::storage::vector_certification::VectorExecution {
                        certification: crate::storage::vector_certification::VectorCertification {
                            version: u32::MAX,
                            origin: crate::storage::control::GenerationRef {
                                store_id: original.store_id,
                                generation: original.committed_generation,
                            },
                            operation_id: original.operation_id,
                            inventory_root: [255; 32],
                            vector_count: u64::MAX,
                        },
                        source: crate::storage::vector_certification::VectorSource::Supplied {
                            profile_id: "authored-codec-only-not-a-certified-profile".into(),
                        },
                    });
            }
            other => panic!("unexpected actual owner outcome: {other:?}"),
        }
        let canonical = serde_json::to_string(&malformed).unwrap();
        let typed: WriteReceipt = serde_json::from_str(&canonical).unwrap();
        assert_eq!(serde_json::to_string(&typed).unwrap(), canonical);
        let mut peer = serde_json::to_value(reply).unwrap();
        peer["lifecycle"]["receipt"] = serde_json::to_value(&malformed).unwrap();
        // Force the real malformed-wrapper path without changing request identity.
        peer["unexpectedPeerField"] = json!(true);
        let before = bytes(&f, d.parent);
        let calls = clock.0.load(Ordering::SeqCst);
        let result =
            api1::Reply::decode(peer.clone(), &request, operation).mcp(&d.parent.to_string());
        assert_eq!(result.is_error, Some(true));
        let value = Value::Object(result.structured_content.unwrap());
        assert_eq!(value["error"]["code"], "INVALID_RESPONSE");
        assert_eq!(value["error"]["commitState"], "committed");
        assert_eq!(
            value["error"]["operationId"],
            original.operation_id.to_string()
        );
        assert!(value["data"].is_null() && value["readiness"].is_null());
        let retained: WriteReceipt = serde_json::from_value(value["receipt"].clone()).unwrap();
        assert_eq!(serde_json::to_string(&retained).unwrap(), canonical);
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);

        if operation == Operation::Forget {
            // The optional proof's codec, unlike its semantic validator, rejects
            // present null and unknown enum values. Historical absence remains.
            for phase_rule in [Value::Null, json!("future_rule")] {
                let mut rejected = peer.clone();
                rejected["lifecycle"]["receipt"]["outcome"]["transition"]["phaseTimeRule"] =
                    phase_rule;
                let result =
                    api1::Reply::decode(rejected, &request, operation).mcp(&d.parent.to_string());
                let value = result.structured_content.unwrap();
                assert!(value["receipt"].is_null());
                assert_eq!(value["error"]["commitState"], "unknown");
            }
            let mut absent = peer;
            absent["lifecycle"]["receipt"]["outcome"]["transition"]
                .as_object_mut()
                .unwrap()
                .remove("phaseTimeRule");
            let result =
                api1::Reply::decode(absent, &request, operation).mcp(&d.parent.to_string());
            assert!(result.structured_content.unwrap()["receipt"].is_object());
        }
        assert_eq!(bytes(&f, d.parent), before);
        assert_eq!(clock.0.load(Ordering::SeqCst), calls);
        drop((bridge, state));
    }
}
