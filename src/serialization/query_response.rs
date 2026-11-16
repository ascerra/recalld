//! Shared prepared query projection. Unknown source coverage remains JSON null.
use super::contract::*;
use crate::search::{
    ChannelParticipation, CoherentSearchResponse, FtsAvailability, RetrievalChannel,
    RetrievalSemantics, SearchError, UnresolvedReason, VersionMode,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryReply {
    #[serde(flatten)]
    pub envelope: Envelope<Value>,
    /// Every retained exposure receipt, including a failed multi-step observer.
    /// A single-namespace successful recall also exposes its one `receipt`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observation_receipts: Vec<crate::storage::control::WriteReceipt>,
    #[serde(skip)]
    pub http_status: u16,
}
impl QueryReply {
    pub(crate) fn unavailable() -> Self {
        Self::failure(SearchError::Unavailable(
            "prepared coherent query unavailable".into(),
        ))
    }
    pub(crate) fn input(error: InputError) -> Self {
        let http_status = if (error.field == "/embedding" && error.code == "OUT_OF_RANGE")
            || (error.field == "/embeddingProfileId" && error.code == "INVALID_COMBINATION")
        {
            422
        } else {
            400
        };
        Self {
            envelope: Envelope::failure(
                None,
                ExecutionError {
                    category: ErrorCategory::InvalidInput,
                    code: error.code.into(),
                    message: error.message.into(),
                    field: Some(error.field),
                    retryable: false,
                    commit_state: CommitState::NotCommitted,
                    operation_id: None,
                },
            ),
            observation_receipts: vec![],
            http_status,
        }
    }
    pub(crate) fn failure(error: SearchError) -> Self {
        let (category, code, status) = match &error {
            SearchError::CanonicalRead(error)
                if matches!(
                    **error,
                    crate::storage::StorageError::NamespaceMismatch { .. }
                ) =>
            {
                (ErrorCategory::Conflict, "NAMESPACE_MISMATCH", 409)
            }
            SearchError::EmbeddingProfileMismatch => {
                (ErrorCategory::Conflict, "EMBEDDING_PROFILE_MISMATCH", 409)
            }
            SearchError::NamespaceNotFound(_) => {
                (ErrorCategory::NotFound, "NAMESPACE_NOT_FOUND", 404)
            }
            SearchError::MemoryNotFound(_) => (ErrorCategory::NotFound, "MEMORY_NOT_FOUND", 404),
            SearchError::InvalidEmbedding(_) => (ErrorCategory::InvalidInput, "OUT_OF_RANGE", 422),
            SearchError::EmptyQuery | SearchError::InvalidFilter(_) => {
                (ErrorCategory::InvalidInput, "INVALID_COMBINATION", 400)
            }
            SearchError::EmbeddingFailed(_) => {
                (ErrorCategory::Unavailable, "EMBEDDING_UNAVAILABLE", 503)
            }
            _ => (ErrorCategory::Unavailable, "INDEX_NOT_READY", 503),
        };
        let mut execution = ExecutionError {
            category,
            code: code.into(),
            message: "query could not complete".into(),
            field: None,
            retryable: false,
            commit_state: CommitState::NotCommitted,
            operation_id: None,
        };
        let mut receipts = vec![];
        if let SearchError::Observation(observation) = error {
            receipts = observation.receipts;
            execution.commit_state = match observation.commit_state {
                "committed" => CommitState::Committed,
                "unknown" => CommitState::Unknown,
                _ => CommitState::NotCommitted,
            };
            execution.operation_id = observation.unknown_operation_id;
        }
        let mut envelope = Envelope::failure(None, execution);
        envelope.receipt = (receipts.len() == 1).then(|| receipts[0].clone());
        if !receipts.is_empty() {
            envelope.warnings.push("READINESS_UNKNOWN: observation committed; no current readiness observation is available".into());
        }
        Self {
            envelope,
            observation_receipts: receipts,
            http_status: status,
        }
    }
    pub(crate) fn mcp(&self) -> crate::mcp::protocol::ToolCallResult {
        // Both transports serialize this exact typed envelope. Compact text
        // avoids doubling pretty-print overhead on full retained receipts.
        let text =
            serde_json::to_string(self).expect("query wire consists of serializable domain types");
        let value: Value = serde_json::from_str(&text).expect("serialized query JSON");
        let mut result = crate::mcp::protocol::ToolCallResult::text(text);
        result.structured_content = value.as_object().cloned();
        result.is_error = Some(self.http_status >= 400);
        result
    }
    pub(crate) fn fits_transport(&self) -> bool {
        self.fits_before_observation(0)
    }
    pub(crate) fn fits_before_observation(&self, exposures: usize) -> bool {
        // Count the actual typed JSON twice: structured bytes and the exact
        // escaping of those same bytes in compact MCP text. No pretty estimate.
        struct Count {
            raw: usize,
            escaped: usize,
        }
        impl std::io::Write for Count {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.raw = self
                    .raw
                    .checked_add(b.len())
                    .ok_or_else(|| std::io::Error::other("response overflow"))?;
                for byte in b {
                    let n = match byte {
                        b'"' | b'\\' => 2,
                        0..=31 => 6,
                        _ => 1,
                    };
                    self.escaped = self
                        .escaped
                        .checked_add(n)
                        .ok_or_else(|| std::io::Error::other("response overflow"))?;
                }
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut c = Count { raw: 0, escaped: 0 };
        // NativeLifecycleWrite is content-free: per returned UUID, two fixed
        // LifecycleStates, two32-byte hash arrays and at most one evicted event.
        // 4096 bounds a complete one-record receipt, including its per-namespace
        // fixed fields and escaped JSON with numeric headroom; legacy exposure is
        // smaller. Four copies cover receipt + observationReceipts in both MCP
        // payloads. Framing/readiness/receipt fixed fields have64KiB allowance.
        serde_json::to_writer(&mut c, self).is_ok()
            && exposures
                .checked_mul(4096 * 4)
                .and_then(|n| n.checked_add(64 * 1024))
                .and_then(|n| n.checked_add(c.raw))
                .and_then(|n| n.checked_add(c.escaped))
                .is_some_and(|n| n <= crate::daemon::protocol::MAX_MESSAGE_SIZE)
    }
    pub(crate) fn oversize(mut self) -> Self {
        self.envelope.data = None;
        self.envelope.status = Status::Error;
        self.http_status = 400;
        self.envelope.error = Some(ExecutionError {
            category: ErrorCategory::InvalidInput,
            code: "RESPONSE_TOO_LARGE".into(),
            message: "whole query response exceeds transport capacity; no result was truncated"
                .into(),
            field: None,
            retryable: false,
            commit_state: if self.observation_receipts.is_empty() {
                CommitState::NotCommitted
            } else {
                CommitState::Committed
            },
            operation_id: None,
        });
        self
    }
}

pub(crate) fn project_similarity(
    selected: &crate::search::SelectedSimilarity,
    all_compatible: bool,
) -> Result<QueryReply, SearchError> {
    let d = &selected.diagnostics;
    let RetrievalSemantics::V2(mode) = d.semantics else {
        return Err(SearchError::Unavailable(
            "similarity semantics2 unavailable".into(),
        ));
    };
    let mut results = Vec::with_capacity(selected.rows.len());
    for row in &selected.rows {
        if row.payload.record.id != *row.result.memory_id.as_bytes()
            || row.payload.record.namespace_id != row.namespace.id.get()
        {
            return Err(SearchError::Unavailable(
                "similarity captured identity mismatch".into(),
            ));
        }
        let mut result = row.result.clone();
        result.full_text = row.payload.full_text.clone();
        result.labels = row.payload.record.schema.as_ref().map(|s| s.labels.clone());
        let mut value = serde_json::to_value(result)
            .map_err(|_| SearchError::Unavailable("similarity serialization failed".into()))?;
        value["namespaceId"] = json!(row.namespace.id);
        if let Some(embedding) = &row.payload.embedding {
            value["embedding"] = json!(embedding);
        }
        results.push(value);
    }
    let c = &selected.coverage;
    let excluded: Vec<_> = c
        .excluded
        .iter()
        .map(|(id, reason)| {
            json!({"namespaceId":id,"reason":match reason {
                crate::search::SimilarityExclusion::UnknownProfile=>"unknownProfile",
                crate::search::SimilarityExclusion::DifferentProfile=>"differentProfile",
                crate::search::SimilarityExclusion::DifferentDimension=>"differentDimension",
            }})
        })
        .collect();
    let scope = if all_compatible {
        Scope::Namespaces {
            namespace_ids: c.searched.clone(),
        }
    } else {
        Scope::Namespace {
            namespace_id: selected.source_namespace.id,
            namespace: selected.source_namespace.name.clone(),
        }
    };
    let mut envelope = Envelope::success(
        scope,
        json!({"results":results,"totalMatches":c.resolved_candidates,
        "sourceNamespace":{"namespaceId":selected.source_namespace.id,"namespace":selected.source_namespace.name},
        "coverage":{"semanticsVersion":2,"versionMode":match mode {VersionMode::Current=>"current",VersionMode::History=>"history"},
            "searchedNamespaceIds":c.searched,"excludedNamespaces":excluded,"examinedRecords":c.examined_records,"eligibleRecords":c.eligible_records,"scoredRecords":c.scored_records,
            "scoreAdmitted":c.score_admitted,"resolvedCandidates":c.resolved_candidates,"resultLimited":c.result_limit_applied,
            "supersession":{"examinedEdges":d.supersession_edges_examined,"unresolved":counts(&d.unresolved)},
        }}),
    );
    envelope.read_generation = Some(selected.read_generation);
    Ok(QueryReply {
        envelope,
        observation_receipts: vec![],
        http_status: 200,
    })
}

fn reason(reason: UnresolvedReason) -> &'static str {
    match reason {
        UnresolvedReason::MissingRecord => "missingRecord",
        UnresolvedReason::MissingStructure => "missingStructure",
        UnresolvedReason::ForeignNamespace => "foreignNamespace",
        UnresolvedReason::Branch => "branch",
        UnresolvedReason::Cycle => "cycle",
        UnresolvedReason::HopLimit => "hopLimit",
        UnresolvedReason::WorkLimit => "workLimit",
        UnresolvedReason::Ineligible => "ineligible",
    }
}
pub(crate) fn version(status: crate::search::VersionStatus) -> Value {
    match status {
        crate::search::VersionStatus::Current => json!({"status":"current"}),
        crate::search::VersionStatus::Historical => json!({"status":"historical"}),
        crate::search::VersionStatus::Unresolved(r) => {
            json!({"status":"unresolved","reason":reason(r)})
        }
    }
}
fn counts(values: &std::collections::BTreeMap<UnresolvedReason, usize>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(k, v)| (reason(*k).into(), json!(v)))
            .collect(),
    )
}

pub(crate) fn project(
    prepared: &CoherentSearchResponse,
    explain: bool,
) -> Result<QueryReply, SearchError> {
    let unavailable = || SearchError::Unavailable("coherent query projection is incomplete".into());
    let d = &prepared.response.diagnostics;
    let RetrievalSemantics::V2(mode) = d.semantics else {
        return Err(unavailable());
    };
    let payload = prepared.payload.as_ref().ok_or_else(unavailable)?;
    let supplemental = payload.supplemental.as_ref().ok_or_else(unavailable)?;
    let channels = d.channels.as_ref().ok_or_else(unavailable)?;
    let mut channel_values = serde_json::Map::new();
    for (channel, c) in channels {
        let name = match channel {
            RetrievalChannel::Vector => "vector",
            RetrievalChannel::Fts => "fts",
            RetrievalChannel::Entity => "entity",
            RetrievalChannel::Metadata => "metadata",
            RetrievalChannel::Graph => "graph",
        };
        channel_values.insert(name.into(),json!({"participation":match c.participation {ChannelParticipation::NotRequested=>"notRequested",ChannelParticipation::Completed=>"completed"},"offered":c.offered,"admitted":c.admitted,"examined":c.examined,"eligible":c.eligible,"exhausted":c.exhausted,"candidateLimited":c.candidate_limited,"budgetExhausted":c.budget_exhausted,"returnedOwn":c.returned_own,"returnedInherited":c.returned_inherited}));
    }
    if payload.rows.len() != prepared.response.response.results.len() {
        return Err(unavailable());
    }
    let mut hits = Vec::with_capacity(payload.rows.len());
    for (result, row) in prepared.response.response.results.iter().zip(&payload.rows) {
        if row.record.id != *result.memory_id.as_bytes()
            || row.record.namespace_id != payload.namespace.id.get()
        {
            return Err(unavailable());
        }
        let mut result = result.clone();
        result.full_text = row.full_text.clone();
        result.labels = row.record.schema.as_ref().map(|s| s.labels.clone());
        let mut value = serde_json::to_value(result).map_err(|_| unavailable())?;
        value["namespaceId"] = json!(payload.namespace.id);
        value["version"] = json!(
            d.versions
                .get(&crate::model::MemoryId::from_bytes(row.record.id))
                .copied()
                .map(version)
        );
        value["graphDepth"] = json!(
            d.graph_depths
                .get(&crate::model::MemoryId::from_bytes(row.record.id))
        );
        value["related"] = json!(
            payload
                .related
                .get(&crate::model::MemoryId::from_bytes(row.record.id))
                .map(|edges| edges
                    .iter()
                    .map(|e| json!({"id":e.id,"edgeType":e.edge_type,"weight":e.weight}))
                    .collect::<Vec<_>>())
                .unwrap_or_default()
        );
        if explain {
            let evidence = d
                .provenance
                .as_ref()
                .and_then(|p| p.get(&crate::model::MemoryId::from_bytes(row.record.id)))
                .and_then(|p| p.explanation.as_ref())
                .ok_or_else(unavailable)?;
            value["explanation"] = super::query_explanation::project(evidence);
        }
        if let Some(embedding) = &row.embedding {
            value["embedding"] = json!(embedding);
        }
        hits.push(value);
    }
    let neighbors:Vec<_>=payload.neighbors.iter().map(|n| json!({"memoryId":uuid::Uuid::from_bytes(n.payload.record.id),"namespaceId":payload.namespace.id,"namespace":payload.namespace.name,"summary":n.payload.record.summary,"fullText":n.payload.full_text,"labels":n.payload.record.schema.as_ref().map(|s|&s.labels),"tags":n.payload.record.tags,"connectedTo":n.connected_to,"edgeType":n.edge_type,"weight":n.weight})).collect();
    let coverage = json!({
        "semanticsVersion":2,"versionMode":match mode{VersionMode::Current=>"current",VersionMode::History=>"history"},
        "budgetProfile":"retrieval-v2-default","channels":channel_values,
        "ftsAvailability":match d.fts.availability{FtsAvailability::NotRequested=>"notRequested",FtsAvailability::Available=>"available",FtsAvailability::Unavailable=>"unavailable"},
        "graph":{"expandedStates":d.graph.expanded_states,"examinedEdges":d.graph.examined_edges,"stateLimited":d.graph.state_limit,"edgeLimited":d.graph.edge_limit},
        "supersession":{"examinedEdges":d.supersession_edges_examined,"unresolved":counts(&d.unresolved),"workLimited":d.unresolved.contains_key(&UnresolvedReason::WorkLimit)||supplemental.supersession_work_limited},
        "metadataCandidateLimited":d.metadata_candidate_limited,"resultLimited":d.result_limit_applied,
        "supplemental":{"requested":supplemental.neighbors_requested,"neighborhoodsExamined":supplemental.neighborhoods_examined,"edgesExamined":supplemental.edges_examined,"supersessionEdgesExamined":supplemental.supersession_edges_examined,"supersessionWorkLimited":supplemental.supersession_work_limited,"neighborhoodsSkippedEdgeBudget":supplemental.neighborhoods_skipped_edge_budget,"neighborhoodsUnexaminedStateBudget":supplemental.neighborhoods_unexamined_state_budget,"depthLimitedSources":supplemental.depth_limited_sources,"ineligibleNeighbors":supplemental.ineligible_neighbors,"historicalNeighborsOmitted":supplemental.historical_neighbors_omitted,"unresolvedNeighbors":counts(&supplemental.unresolved_neighbors),"eligibleUniqueNeighbors":supplemental.eligible_unique_neighbors,"neighborLimitApplied":supplemental.neighbor_limit_applied}
    });
    let mut envelope = Envelope::success(
        Scope::Namespace {
            namespace_id: payload.namespace.id,
            namespace: payload.namespace.name.clone(),
        },
        json!({"results":hits,"neighbors":neighbors,"totalMatches":prepared.response.response.total_matches,"coverage":coverage}),
    );
    envelope.read_generation = Some(prepared.read_generation);
    Ok(QueryReply {
        envelope,
        observation_receipts: vec![],
        http_status: 200,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_query_valid_lifecycle_receipt_groups_fit_full_error_projection_reservation() {
        use crate::model::schema::{LifecycleFields, LifecycleVersion, TimestampOrigin};
        use crate::model::{AccessEvent, AccessKind, DecayPhase, MemoryId, NamespaceId};
        use crate::storage::control::{Generation, GenerationRef, MutationOutcome, WriteReceipt};
        use crate::storage::native_lifecycle::{
            LifecycleChange, LifecycleState, NativeLifecycleWrite, exposed, history_hash,
        };
        let time = chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp_millis();
        let previous = Generation::try_from((Generation::MAX.get() - 1).to_string()).unwrap();
        let state = LifecycleState {
            lifecycle: LifecycleFields {
                observed_at: time,
                retrieved_at: Some(time),
                reinforced_at: Some(time),
                decay_anchor_at: time,
                lifecycle_version: LifecycleVersion::ExplicitFeedbackV2,
                lifecycle_updated_at: time,
                timestamp_origin: TimestampOrigin::LegacyInferred,
                phase_changed_at: time,
                policy_revision: Generation::MAX,
                record_generation: previous,
            },
            created_at: time,
            last_accessed_at: time,
            stability: crate::model::constants::STABILITY_CEILING
                .try_into()
                .unwrap(),
            difficulty: 10f32.try_into().unwrap(),
            strength: f32::MIN_POSITIVE.try_into().unwrap(),
            decay_strength: f32::MIN_POSITIVE.try_into().unwrap(),
            phase: DecayPhase::Summary,
            is_permastore: false,
            has_summary: true,
            has_text: false,
        };
        state.validate().unwrap();
        let history = vec![
            AccessEvent {
                timestamp: time,
                kind: AccessKind::AssociativeRetrieval
            };
            crate::model::constants::ACCESS_HISTORY_MAX
        ];
        let step = LifecycleChange {
            memory_id: MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX)),
            kind: AccessKind::AssociativeRetrieval,
            before: state.clone(),
            after: exposed(&state, time).unwrap(),
            history_before: history_hash(&history).unwrap(),
            history_after: history_hash(&history).unwrap(),
            evicted_event: Some(history[0]),
        };
        let store_id = uuid::Uuid::from_u128(u128::MAX);
        let mut receipts = vec![];
        for i in 0..crate::mcp::args::MAX_SEARCH_LIMIT {
            let mut step = step.clone();
            step.memory_id = MemoryId::from_uuid(uuid::Uuid::from_u128(u128::MAX - i as u128));
            let previous = Generation::try_from(
                (Generation::MAX.get() - crate::mcp::args::MAX_SEARCH_LIMIT as u64 + i as u64)
                    .to_string(),
            )
            .unwrap();
            let transition = NativeLifecycleWrite {
                namespace_id: NamespaceId::new(u32::MAX - i as u32),
                lifecycle_version: LifecycleVersion::ExplicitFeedbackV2,
                policy_revision: Generation::MAX,
                previous_generation: previous,
                capture_generation: GenerationRef {
                    store_id,
                    generation: previous,
                },
                sampled_at: time,
                records: vec![step.clone()],
                feedback: None,
            };
            let receipt = WriteReceipt {
                store_id,
                operation_id: uuid::Uuid::from_u128(u128::MAX - i as u128),
                committed_generation: previous.next().unwrap(),
                receipt_version: 1,
                outcome: MutationOutcome::LifecycleExposureRecorded {
                    transition: transition.clone(),
                },
                replayed: false,
            };
            transition.validate(&receipt, false).unwrap();
            let text = serde_json::to_string(&receipt).unwrap();
            let escaped = serde_json::to_string(&text).unwrap();
            // Includes each namespace's complete receipt overhead. Numeric
            // slack covers other valid finite spellings/hash byte widths;
            // no invalid state or unreachable negative stability is used.
            assert!(
                escaped.len() + 16 * 40 + 2 * 32 * 4 < 4096,
                "{}",
                escaped.len()
            );
            receipts.push(receipt);
        }
        let failure = QueryReply::failure(SearchError::Observation(
            crate::decay::observation::ObservationError::not_ready(
                receipts.clone(),
                "raw diagnostic intentionally excluded: ".repeat(100_000),
            ),
        ));
        super::super::query_schema::record_fixture(
            "maximal-valid-receipts",
            false,
            &serde_json::to_value(&failure).unwrap(),
        );
        assert_eq!(failure.observation_receipts, receipts);
        assert!(failure.envelope.receipt.is_none());
        assert!(failure.fits_transport());
        assert!(
            serde_json::to_vec(&failure.mcp()).unwrap().len()
                < receipts.len() * 4096 * 4 + 64 * 1024
        );
        assert!(failure.envelope.error.as_ref().unwrap().message.len() < 128);
        let one = QueryReply::failure(SearchError::Observation(
            crate::decay::observation::ObservationError::not_ready(vec![receipts[0].clone()], "x"),
        ));
        assert_eq!(one.envelope.receipt.as_ref(), Some(&receipts[0]));
        assert_eq!(one.observation_receipts, vec![receipts[0].clone()]);
        assert!(serde_json::to_vec(&one.mcp()).unwrap().len() < 4096 * 4 + 64 * 1024);
    }
}
