//! Private point exchange. Public get routing/admission is independently unchanged.
use crate::model::{AccessKind, DecayPhase};
use crate::serialization::contract::{CommitState, ErrorCategory, ExecutionError, Scope, Status};
use crate::serialization::get::{GetReply, GetRequest};
use crate::storage::control::MutationOutcome;
use serde_json::{Value, json};

pub(crate) const GET: &str = "get_contract_v1";

pub(crate) fn response_limit() -> usize {
    static LIMIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| {
        let mut overhead = 0;
        for access in [false, true] {
            for sent in [false, true] {
                for evidence in [false, true] {
                    let mut reply = GetReply::remote_failure(access, sent, vec![]);
                    if evidence {
                        reply.received_evidence = Some(
                            crate::serialization::query_response::ReceivedQueryEvidence {
                                receipts: vec![],
                            },
                        );
                        reply
                            .envelope
                            .warnings
                            .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
                    }
                    let value = serde_json::to_value(reply.mcp()).expect("fixed get error");
                    overhead = overhead.max(
                        serde_json::to_vec(&super::protocol::DaemonResponse::success(
                            u64::MAX,
                            value,
                        ))
                        .expect("fixed get frame")
                        .len(),
                    );
                }
            }
        }
        // The same R1 raw scalar/structure preflight bounds reserialization;
        // compact JSON escaped inside text costs at most twice its raw bytes.
        // Known GetReply uses f32 domain fields then MCP to_value; reserve the
        // existing one-point scalar/receipt/completion bound for that numeric
        // expansion: 50176 (two receipt aliases/completion/framing) +
        // 3 MemoryResponse f32 * 32 bytes * 3 MCP copies = 50464 < 65536.
        // Strict typed decode rejects extra numeric leaves; fixed integers are
        // exact and variable strings remain counted in raw. Received evidence
        // stays Value and never reconstructs f32.
        (super::protocol::MAX_MESSAGE_SIZE
            - overhead
            - crate::serialization::get::POINT_EVIDENCE_RESERVE)
            / 3
    })
}

pub(crate) fn admitted_frame(bytes: &[u8]) -> bool {
    super::prepared_query::shape::check(bytes, response_limit()).is_ok()
}

pub(crate) async fn remote(
    client: &super::DaemonClient,
    request: GetRequest,
    limit: usize,
) -> GetReply {
    use super::client::TransmissionPhase;
    if let Err(error) = request.validate() {
        return GetReply::input(error);
    }
    // The fixed private era certifies the full outer ceiling. A smaller caller
    // must fail before transmission, never observe then discover an outer bound.
    if limit < super::protocol::MAX_MESSAGE_SIZE {
        return GetReply::remote_failure(request.record_access, false, vec![]);
    }
    let arguments = json!({"contractVersion":2,"id":request.id.to_string(),"namespace":request.namespace,"recordAccess":request.record_access});
    let exchange = client
        .call_prepared(
            GET,
            json!({
                "arguments":arguments,"responseLimit":response_limit(),
                "originalContractVersion":request.original_contract_version,
            }),
        )
        .await;
    let mut evidence = vec![];
    if let Some(result) = exchange.result {
        if exchange.phase == TransmissionPhase::MatchedReply
            && result.as_object().is_some_and(|o| o.len() == 3)
            && result.get("wireVersion").and_then(Value::as_u64) == Some(1)
            && let (Some(status), Some(reply)) = (
                result
                    .get("httpStatus")
                    .and_then(Value::as_u64)
                    .and_then(|v| u16::try_from(v).ok()),
                result.get("reply"),
            )
            && let Ok(reply) = decode(reply, status, &request)
        {
            return reply;
        }
        // Named admitted result fragments only, never recursive evidence or
        // current-operation attribution from an uncorrelated/malformed peer.
        if let Some(reply) = result.get("reply") {
            let alias = reply.get("receipt").into_iter();
            let groups = reply
                .get("observationReceipts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten();
            for value in alias.chain(groups) {
                if super::prepared_query::decode::receipt(value).is_ok()
                    && !evidence.contains(value)
                {
                    evidence.push(value.clone());
                }
            }
        }
    }
    GetReply::remote_failure(
        request.record_access,
        exchange.phase != TransmissionPhase::BeforeSend,
        evidence,
    )
}

pub(crate) async fn dispatch(
    search: &dyn crate::mcp::bridge::SearchPipeline,
    params: Value,
    peer_limit: Option<usize>,
) -> Result<Value, crate::mcp::bridge::BridgeError> {
    use crate::mcp::bridge::BridgeError;
    if !search.supports_prepared_get()
        || peer_limit.is_none_or(|v| v < super::protocol::MAX_MESSAGE_SIZE)
    {
        return Err(BridgeError::Unavailable(
            "prepared get requires negotiated point capability".into(),
        ));
    }
    let object = params
        .as_object()
        .ok_or_else(|| BridgeError::InvalidInput("prepared get requires object".into()))?;
    if object.len() != 3
        || object.get("responseLimit").and_then(Value::as_u64) != Some(response_limit() as u64)
        || !matches!(
            object
                .get("originalContractVersion")
                .and_then(Value::as_u64),
            Some(1 | 2)
        )
    {
        return Err(BridgeError::InvalidInput(
            "prepared get allowance/origin mismatch".into(),
        ));
    }
    let args = object
        .get("arguments")
        .ok_or_else(|| BridgeError::InvalidInput("missing get arguments".into()))?;
    let reply = match GetRequest::from_mcp(args) {
        Ok(mut r) => {
            r.original_contract_version =
                object["originalContractVersion"].as_u64().unwrap() as u32;
            match r.validate() {
                Ok(()) => search.get_contract(r, response_limit()).await,
                Err(error) => GetReply::input(error),
            }
        }
        Err(e) => GetReply::input(e),
    };
    // No fallible BridgeError mapping after the observer. Typed known/unknown
    // outcomes remain in `result`, including defensive final-frame refusal.
    Ok(framed_reply(reply))
}

fn framed_reply(mut reply: GetReply) -> Value {
    fn encode(reply: &GetReply) -> Value {
        // DTO consists solely of finite domain scalars, strings and fixed keys;
        // no user Serialize implementation or invalid map key can enter here.
        let bytes = serde_json::to_vec(&super::prepared_query::Reply {
            wire_version: 1,
            http_status: reply.http_status,
            reply,
        })
        .expect("get domain DTO serializes");
        serde_json::from_slice(&bytes).expect("serialized get DTO")
    }
    let value = encode(&reply);
    let full = super::protocol::DaemonResponse::success(u64::MAX, value.clone());
    if serde_json::to_vec(&full).expect("typed get frame").len() <= response_limit() {
        return value;
    }
    reply.envelope.data = None;
    reply.envelope.status = Status::Error;
    reply.envelope.readiness = None;
    reply.http_status = 503;
    // Keep an existing unknown operation and every original known receipt.
    if reply.envelope.error.is_none() {
        reply.envelope.error = Some(ExecutionError {
            category: ErrorCategory::Unavailable,
            code: "GET_RESPONSE_UNAVAILABLE".into(),
            message: "Prepared get response unavailable; retained observation evidence follows."
                .into(),
            field: None,
            retryable: false,
            commit_state: if reply.observation_receipts.is_empty() {
                CommitState::NotCommitted
            } else {
                CommitState::Committed
            },
            operation_id: None,
        });
    }
    encode(&reply)
}

fn require(value: bool) -> Result<(), ()> {
    value.then_some(()).ok_or(())
}
fn decode(value: &Value, status: u16, request: &GetRequest) -> Result<GetReply, ()> {
    require(
        value.get("contractVersion").and_then(Value::as_u64) == Some(2)
            && value.get("receivedEvidence").is_none(),
    )?;
    let mut reply: GetReply = super::prepared_query::decode::typed(value)?;
    reply.http_status = status;
    require(matches!(status, 200 | 400 | 404 | 409 | 422 | 503))?;
    let ok = reply.envelope.status == Status::Ok;
    require(ok == (status == 200))?;
    require(if ok {
        reply.envelope.error.is_none()
            && reply.envelope.data.is_some()
            && reply.envelope.read_generation.is_some()
    } else {
        reply.envelope.status == Status::Error
            && reply.envelope.data.is_none()
            && reply.envelope.error.is_some()
    })?;
    let scope = match &reply.envelope.scope {
        Some(Scope::Namespace {
            namespace_id,
            namespace,
        }) => {
            require(!namespace_id.is_unset())?;
            crate::model::validation::validate_namespace_name(namespace).map_err(|_| ())?;
            require(
                request
                    .namespace
                    .as_ref()
                    .is_none_or(|n| n.eq_ignore_ascii_case(namespace)),
            )?;
            Some((*namespace_id, namespace.as_str()))
        }
        None => None,
        _ => return Err(()),
    };
    if let Some(g) = reply.envelope.read_generation {
        require(!g.store_id.is_nil())?;
    }
    let mut observe = false;
    if let Some(data) = &reply.envelope.data {
        let (_, ns) = scope.ok_or(())?;
        let m = &data.memory;
        require(
            m.id == request.id && m.namespace == ns && data.record_access == request.record_access,
        )?;
        for time in [data.evaluated_at, m.created_at, m.last_accessed_at] {
            require(chrono::DateTime::from_timestamp_millis(time).is_some())?;
        }
        require(
            m.strength.is_finite()
                && m.decay_strength.is_finite()
                && m.stability.is_finite()
                && m.stability > 0.,
        )?;
        if let Some(labels) = &m.labels {
            labels.validate().map_err(|_| ())?;
        }
        match m.phase {
            DecayPhase::Full => {}
            DecayPhase::Summary => require(m.full_text.is_none())?,
            DecayPhase::Ghost | DecayPhase::Tombstone => require(
                m.summary.is_none()
                    && m.full_text.is_none()
                    && m.tags.is_empty()
                    && m.labels.as_ref().is_none_or(|v| {
                        v.entities.is_empty() && v.topics.is_empty() && v.emotions.is_empty()
                    }),
            )?,
        }
        observe =
            request.record_access && matches!(m.phase, DecayPhase::Full | DecayPhase::Summary);
    }
    require(reply.observation_receipts.len() <= 1)?;
    require(reply.envelope.receipt.as_ref() == reply.observation_receipts.first())?;
    require(
        request.record_access
            || (reply.observation_receipts.is_empty() && reply.envelope.readiness.is_none()),
    )?;
    if ok {
        require(observe == !reply.observation_receipts.is_empty())?;
    }
    for receipt in &reply.observation_receipts {
        let raw = value.get("receipt").ok_or(())?;
        super::prepared_query::decode::receipt(raw)?;
        let read = reply.envelope.read_generation.ok_or(())?;
        let (ns, _) = scope.ok_or(())?;
        require(
            !receipt.replayed
                && receipt.store_id == read.store_id
                && receipt.committed_generation > read.generation,
        )?;
        match &receipt.outcome {
            MutationOutcome::LifecycleExposureRecorded { transition } => {
                transition.validate(receipt, false).map_err(|_| ())?;
                require(
                    transition.namespace_id == ns
                        && transition.capture_generation == read
                        && transition.records.len() == 1,
                )?;
                let change = &transition.records[0];
                require(
                    change.memory_id == request.id && change.kind == AccessKind::DirectRetrieval,
                )?;
                if let Some(data) = &reply.envelope.data {
                    require(transition.sampled_at == data.evaluated_at)?;
                }
            }
            MutationOutcome::ExposuresRecorded { accesses } => {
                require(
                    accesses.len() == 1
                        && accesses[0].memory_id == request.id
                        && accesses[0].kind == AccessKind::DirectRetrieval
                        && chrono::DateTime::from_timestamp_millis(accesses[0].accessed_at)
                            .is_some(),
                )?;
                // The legacy final writer clamps against live anchors, including
                // intervening feedback; it does not carry a captured policy token.
                if let Some(data) = &reply.envelope.data {
                    require(accesses[0].accessed_at >= data.evaluated_at)?;
                }
            }
            _ => return Err(()),
        }
    }
    if let Some(error) = &reply.envelope.error {
        require(matches!(
            (status, &error.category),
            (400 | 422, ErrorCategory::InvalidInput)
                | (404, ErrorCategory::NotFound)
                | (409, ErrorCategory::Conflict)
                | (503, ErrorCategory::Unavailable)
        ))?;
        require(!error.retryable && error.operation_id.is_none_or(|id| !id.is_nil()))?;
        require(request.record_access || error.commit_state == CommitState::NotCommitted)?;
        require(match error.commit_state {
            CommitState::NotCommitted => reply.observation_receipts.is_empty(),
            CommitState::Committed => !reply.observation_receipts.is_empty(),
            CommitState::Unknown => true,
        })?;
    }
    if let Some(ready) = &reply.envelope.readiness {
        let receipt = reply.observation_receipts.first().ok_or(())?;
        require(
            ok && ready.ready
                && ready.pending_work == 0
                && ready.store_id == receipt.store_id
                && ready.committed_generation >= receipt.committed_generation
                && ready.pending_work_scope.is_none()
                && ready.observed_work.is_none()
                && ready.derived.len() == 4,
        )?;
        let mut names = std::collections::BTreeSet::new();
        for view in &ready.derived {
            require(
                matches!(view.name.as_str(), "graph" | "vectors" | "fts" | "entities")
                    && names.insert(view.name.as_str())
                    && view.ready
                    && view.applied_generation == Some(ready.committed_generation),
            )?;
        }
    } else {
        require(!ok || reply.observation_receipts.is_empty())?;
    }
    Ok(reply)
}

#[cfg(test)]
#[path = "prepared_get_tests.rs"]
pub(crate) mod tests;
