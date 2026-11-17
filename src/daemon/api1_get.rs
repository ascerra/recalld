//! New ordinary API1 point exchange; the existing prepared O1 era is unchanged.
use crate::mcp::bridge::{BridgeError, StorageEngine};
use crate::model::{AccessKind, DecayPhase};
use crate::serialization::contract::{CommitState, Scope, Status};
use crate::serialization::get::{GetReply, GetRequest};
use crate::serialization::get_api1::{
    self as wire, COMPLETION_RESERVE, MAX_FRAGMENTS, Mode, Reply, Request,
};
use crate::storage::control::MutationOutcome;
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde_json::{Value, json, value::RawValue};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const GET: &str = "get_api1_v1";

pub(crate) fn response_limit() -> usize {
    static LIMIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| {
        let mut overhead = 0;
        // Null strings here count punctuation only, never admitted receipts.
        let raw = vec!["null".to_owned(); MAX_FRAGMENTS];
        for access in [false, true] {
            for sent in [false, true] {
                let mut reply = GetReply::remote_failure(access, sent, vec![]);
                reply
                    .envelope
                    .warnings
                    .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
                let value = serde_json::to_value(wire::project(&reply, chrono_tz::UTC, &raw))
                    .expect("fixed failure");
                let frame = super::protocol::DaemonResponse::success(u64::MAX, value);
                overhead = overhead.max(
                    serde_json::to_vec(&frame)
                        .expect("fixed frame")
                        .len()
                        .checked_add(4)
                        .expect("fixed overhead"),
                );
            }
        }
        // Compact checked raw: <=2R structured and <=4R nested text. Count
        // before dedup; retain full evidence or reject admission, never clip.
        super::protocol::MAX_MESSAGE_SIZE
            .checked_sub(overhead)
            .and_then(|n| n.checked_sub(COMPLETION_RESERVE))
            .map(|n| n / 6)
            .expect("reviewed fixed point allowance")
    })
}

pub(crate) fn admitted_frame(raw: &[u8]) -> bool {
    super::prepared_query::shape::check(raw, response_limit()).is_ok()
}

pub(crate) async fn dispatch(
    storage: &dyn StorageEngine,
    params: Value,
    peer_limit: Option<usize>,
) -> Result<Value, BridgeError> {
    if !storage.supports_api1_get()
        || peer_limit.is_none_or(|n| n < super::protocol::MAX_MESSAGE_SIZE)
    {
        return Err(BridgeError::Unavailable(
            "ordinary get requires negotiated api1GetV1".into(),
        ));
    }
    let object = params
        .as_object()
        .ok_or_else(|| BridgeError::InvalidInput("get requires object".into()))?;
    if object.keys().any(|k| {
        !matches!(
            k.as_str(),
            "wireVersion"
                | "originalContractVersion"
                | "id"
                | "namespace"
                | "recordAccess"
                | "responseLimit"
        )
    }) || object.get("wireVersion").and_then(Value::as_u64) != Some(1)
        || object
            .get("originalContractVersion")
            .and_then(Value::as_u64)
            != Some(1)
        || object.get("responseLimit").and_then(Value::as_u64) != Some(response_limit() as u64)
        || !object.get("recordAccess").is_some_and(Value::is_boolean)
    {
        return Err(BridgeError::InvalidInput(
            "API1 get era/origin/allowance mismatch".into(),
        ));
    }
    let id = crate::mcp::args::require_uuid(&params, "id")
        .map_err(|e| BridgeError::InvalidInput(e.to_string()))?;
    let namespace = crate::mcp::args::parse_namespace_assertion(&params)
        .map_err(|e| BridgeError::InvalidInput(e.to_string()))?
        .map(str::to_owned);
    let request = Request {
        id,
        namespace,
        record_access: params["recordAccess"].as_bool().unwrap(),
    };
    let reply = storage.get_api1(request, response_limit()).await;
    match reply {
        Reply::Point {
            reply,
            timezone,
            native,
        } => Ok(wire::tagged(
            &reply,
            timezone,
            if native { Mode::Native } else { Mode::Legacy },
        )),
        Reply::Unverified {
            reply,
            receipts_json,
        } if receipts_json.is_empty() => Ok(wire::tagged(&reply, chrono_tz::UTC, Mode::Native)),
        _ => Err(BridgeError::Unavailable(
            "actual owning point implementation required".into(),
        )),
    }
}

pub(crate) async fn remote(client: &super::DaemonClient, request: Request, limit: usize) -> Reply {
    if limit < response_limit() {
        return Reply::unavailable(request.record_access, false);
    }
    let mut params = json!({"wireVersion":1,"originalContractVersion":1,"id":request.id.to_string(),
        "recordAccess":request.record_access,"responseLimit":response_limit()});
    if let Some(name) = &request.namespace {
        params["namespace"] = json!(name);
    }
    let exchange = client.call_api1_get(params, &request).await;
    let exchange = match exchange {
        super::client::Api1GetExchange::Compatibility(value) => {
            return Reply::Compatibility(value.and_then(|value| {
                serde_json::from_value(value).map_err(|e| BridgeError::Internal(e.to_string()))
            }));
        }
        super::client::Api1GetExchange::Prepared(exchange) => exchange,
    };
    let mut evidence = vec![];
    if let Some(value) = exchange.result
        && let Some(raw) = value.get("replyJson").and_then(Value::as_str)
        // Full candidate admission precedes any owned typed reply acquisition.
        && let Ok(receipts) = acquire(raw, response_limit())
    {
        if exchange.phase == super::client::TransmissionPhase::MatchedReply
            && value.as_object().is_some_and(|o| o.len() == 5)
            && value.get("wireVersion").and_then(Value::as_u64) == Some(1)
            && let Some(timezone) = value
                .get("timezone")
                .and_then(Value::as_str)
                .and_then(|n| n.parse::<chrono_tz::Tz>().ok())
            && let Some(status) = value
                .get("httpStatus")
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok())
            && let Ok(fields) = serde_json::from_str::<BTreeMap<String, &RawValue>>(raw)
        {
            let point = request.point();
            let decoded = match value.get("kind").and_then(Value::as_str) {
                Some("native") => super::prepared_get::decode(raw, &fields, status, &point)
                    .map(|r| (r, Mode::Native)),
                Some("legacy") => {
                    decode_legacy(raw, &fields, status, &point).map(|r| (r, Mode::Legacy))
                }
                _ => Err(()),
            };
            if let Ok((reply, mode)) = decoded {
                return Reply::point(reply, timezone, mode);
            }
        }
        evidence = receipts;
    }
    let mut reply = GetReply::remote_failure(
        request.record_access,
        exchange.phase != super::client::TransmissionPhase::BeforeSend,
        vec![],
    );
    if !evidence.is_empty() {
        reply
            .envelope
            .warnings
            .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
    }
    Reply::Unverified {
        reply: Box::new(reply),
        receipts_json: evidence,
    }
}

fn require(value: bool) -> Result<(), ()> {
    value.then_some(()).ok_or(())
}

fn decode_legacy(
    raw: &str,
    fields: &BTreeMap<String, &RawValue>,
    status: u16,
    request: &GetRequest,
) -> Result<GetReply, ()> {
    if status != 200 {
        return super::prepared_get::decode(raw, fields, status, request);
    }
    let mut reply: GetReply = super::prepared_query::decode::typed_raw(raw)?;
    reply.http_status = status;
    require(
        fields
            .get("contractVersion")
            .is_some_and(|v| v.get() == "2")
            && reply.envelope.status == Status::Ok
            && reply.envelope.error.is_none()
            && reply.received_evidence.is_none(),
    )?;
    let Some(Scope::Namespace {
        namespace_id,
        namespace,
    }) = &reply.envelope.scope
    else {
        return Err(());
    };
    require(!namespace_id.is_unset())?;
    crate::model::validation::validate_namespace_name(namespace).map_err(|_| ())?;
    require(
        request
            .namespace
            .as_ref()
            .is_none_or(|n| n.eq_ignore_ascii_case(namespace)),
    )?;
    let read = reply.envelope.read_generation.ok_or(())?;
    require(!read.store_id.is_nil())?;
    let data = reply.envelope.data.as_ref().ok_or(())?;
    let memory = &data.memory;
    require(
        memory.id == request.id
            && memory.namespace == *namespace
            && data.record_access == request.record_access,
    )?;
    for time in [
        data.evaluated_at,
        memory.created_at,
        memory.last_accessed_at,
    ] {
        require(chrono::DateTime::from_timestamp_millis(time).is_some())?;
    }
    require(
        memory.strength.is_finite()
            && memory.decay_strength.is_finite()
            && memory.stability.is_finite()
            && memory.stability > 0.,
    )?;
    if let Some(labels) = &memory.labels {
        labels.validate().map_err(|_| ())?;
    }
    // Historical Ghost gets expose the retained legacy payload and record an
    // event. Never substitute Native's Full/Summary predicate here.
    if memory.phase == DecayPhase::Tombstone {
        require(
            memory.summary.as_ref().is_none_or(String::is_empty)
                && memory.full_text.is_none()
                && memory.tags.is_empty()
                && memory.strength == 0.
                && !memory.is_permastore,
        )?;
    }
    let observe = request.record_access && memory.phase != DecayPhase::Tombstone;
    require(
        reply.observation_receipts.len() == usize::from(observe)
            && reply.envelope.receipt.as_ref() == reply.observation_receipts.first(),
    )?;
    if let Some(receipt) = reply.observation_receipts.first() {
        let raw_receipt = fields.get("receipt").ok_or(())?;
        let aliases: Vec<&RawValue> =
            serde_json::from_str(fields.get("observationReceipts").ok_or(())?.get())
                .map_err(|_| ())?;
        require(aliases.len() == 1 && aliases[0].get() == raw_receipt.get())?;
        super::prepared_query::decode::receipt_raw(raw_receipt.get())?;
        require(
            !receipt.replayed
                && receipt.store_id == read.store_id
                && receipt.committed_generation > read.generation,
        )?;
        let MutationOutcome::ExposuresRecorded { accesses } = &receipt.outcome else {
            return Err(());
        };
        require(
            accesses.len() == 1
                && accesses[0].memory_id == request.id
                && accesses[0].kind == AccessKind::DirectRetrieval
                && accesses[0].accessed_at >= data.evaluated_at
                && chrono::DateTime::from_timestamp_millis(accesses[0].accessed_at).is_some(),
        )?;
        let ready = reply.envelope.readiness.as_ref().ok_or(())?;
        require(
            ready.ready
                && ready.pending_work == 0
                && ready.store_id == receipt.store_id
                && ready.committed_generation >= receipt.committed_generation
                && ready.pending_work_scope.is_none()
                && ready.observed_work.is_none()
                && ready.derived.len() == 4,
        )?;
        let mut names = BTreeSet::new();
        for view in &ready.derived {
            require(
                matches!(view.name.as_str(), "graph" | "vectors" | "fts" | "entities")
                    && names.insert(view.name.as_str())
                    && view.ready
                    && view.applied_generation == Some(ready.committed_generation),
            )?;
        }
    } else {
        require(reply.envelope.readiness.is_none())?;
    }
    Ok(reply)
}

// Count every named candidate, including invalid receipts and repeated aliases,
// before retaining a pointer or invoking the typed validator. Do not reserve
// from the peer's sequence size hint.
struct Candidates<'a> {
    raw: Vec<&'a RawValue>,
    bytes: usize,
    limit: usize,
}

impl<'a> Candidates<'a> {
    fn push(&mut self, raw: &'a RawValue) -> Result<(), ()> {
        if self.raw.len() >= MAX_FRAGMENTS {
            return Err(());
        }
        self.bytes = self
            .bytes
            .checked_add(raw.get().len())
            .filter(|n| *n <= self.limit)
            .ok_or(())?;
        self.raw.push(raw);
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for &mut Candidates<'de> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for &mut Candidates<'de> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("bounded raw receipt array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<(), A::Error> {
        while let Some(raw) = array.next_element::<&RawValue>()? {
            self.push(raw).map_err(|()| {
                serde::de::Error::custom("receipt count or byte allowance exceeded")
            })?;
        }
        Ok(())
    }
}

fn acquire(raw: &str, limit: usize) -> Result<Vec<String>, ()> {
    // All structure/count/byte admission precedes typed receipt acquisition.
    super::prepared_query::shape::check_typed(raw.as_bytes(), limit).map_err(|_| ())?;
    let fields: BTreeMap<String, &RawValue> = serde_json::from_str(raw).map_err(|_| ())?;
    if fields.contains_key("receivedEvidence") {
        return Err(());
    }
    let mut candidates = Candidates {
        raw: Vec::new(),
        bytes: 0,
        limit,
    };
    if let Some(raw) = fields.get("receipt") {
        candidates.push(raw)?;
    }
    if let Some(groups) = fields.get("observationReceipts") {
        let mut decoder = serde_json::Deserializer::from_str(groups.get());
        (&mut candidates)
            .deserialize(&mut decoder)
            .map_err(|_| ())?;
        decoder.end().map_err(|_| ())?;
    }
    let mut seen = BTreeSet::new();
    let raw: Vec<&RawValue> = candidates
        .raw
        .into_iter()
        .filter(|r| {
            crate::daemon::prepared_query::decode::receipt_raw(r.get()).is_ok()
                && seen.insert(r.get())
        })
        .collect();
    Ok(raw.into_iter().map(|r| r.get().to_owned()).collect())
}
