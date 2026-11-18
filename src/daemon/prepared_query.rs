//! Prepared-only RPC. Public application admission remains independently closed.
use crate::serialization::query_response::{QueryReply, ReceivedQueryEvidence};
use serde::Serialize;
use serde_json::Value;

#[path = "prepared_query/data.rs"]
mod data;
#[path = "prepared_query/decode.rs"]
pub(crate) mod decode;
#[path = "prepared_query/pairs.rs"]
mod pairs;
#[path = "prepared_query/shape.rs"]
pub(crate) mod shape;

pub(crate) const QUERY: &str = "query_contract_v1";
pub(crate) const SIMILAR: &str = "similar_contract_v1";
pub(crate) const PAIRS: &str = "exact_pairs_bits_v1";

pub(crate) async fn remote_pairs(
    client: &super::DaemonClient,
    mut request: crate::serialization::scan::ExactScanRequest,
) -> Result<crate::search::exact_scan::ExactScanPage, crate::search::exact_scan::ExactScanError> {
    use crate::search::exact_scan::ExactScanError as Error;
    if request.threshold == 0.0 {
        request.threshold = 0.0; // Existing public/cursor canonical zero.
    }
    let arguments = request.rpc_arguments()?;
    let exchange = client
        .call_prepared(
            PAIRS,
            serde_json::json!({"arguments":arguments,"responseLimit":response_limit()}),
        )
        .await;
    if exchange.phase != super::client::TransmissionPhase::MatchedReply {
        return Err(Error::Unavailable);
    }
    let result = exchange.result.ok_or(Error::Unavailable)?;
    if result.get("wireVersion").and_then(Value::as_u64) != Some(1)
        || result.as_object().is_none_or(|o| o.len() != 3)
    {
        return Err(Error::Unavailable);
    }
    if result.get("httpStatus").and_then(Value::as_u64) == Some(200) {
        return pairs::decode(result.get("reply").ok_or(Error::Unavailable)?, &request);
    }
    if result.get("httpStatus").and_then(Value::as_u64) != Some(400) {
        return Err(Error::Unavailable);
    }
    Err(match result.get("error").and_then(Value::as_str) {
        Some("namespaceNotFound") => Error::NamespaceNotFound,
        Some("stale") => Error::Stale,
        Some("tooLarge") => Error::TooLarge,
        Some("responseTooLarge") => Error::ResponseTooLarge,
        Some("profileUnavailable") => Error::ProfileUnavailable,
        Some("invalidCursor") => Error::Input(crate::serialization::contract::InputError {
            code: "INVALID_CURSOR",
            field: "/cursor".into(),
            message: "cursor is malformed, unsupported, or differently bound",
        }),
        _ => Error::Unavailable,
    })
}

pub(crate) async fn remote_query(
    client: &super::DaemonClient,
    request: crate::serialization::query::QueryRequest,
) -> QueryReply {
    let expected = decode::Expected::query(&request);
    let arguments = match request.rpc_arguments() {
        Ok(v) => v,
        Err(e) => return QueryReply::input(e),
    };
    remote(client, QUERY, arguments, expected).await
}
pub(crate) async fn remote_similar(
    client: &super::DaemonClient,
    request: crate::serialization::query::SimilarRequest,
) -> QueryReply {
    let expected = decode::Expected::similar(&request);
    let arguments = match request.rpc_arguments() {
        Ok(v) => v,
        Err(e) => return QueryReply::input(e),
    };
    remote(client, SIMILAR, arguments, expected).await
}
async fn remote(
    client: &super::DaemonClient,
    method: &str,
    arguments: Value,
    expected: decode::Expected,
) -> QueryReply {
    use super::client::TransmissionPhase;
    let exchange = client
        .call_prepared(
            method,
            serde_json::json!({"arguments":arguments,"responseLimit":response_limit()}),
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
                    .and_then(|n| u16::try_from(n).ok()),
                result.get("reply"),
            )
            && let Ok(reply) = decode::query(reply, status, &expected)
        {
            return reply;
        }
        // Only named wrapper evidence, never recursive searches or incoming
        // receivedEvidence promotion. No current-query attribution follows.
        if let Some(reply) = result.get("reply") {
            let alias = reply.get("receipt").into_iter();
            let groups = reply
                .get("observationReceipts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten();
            for value in alias.chain(groups) {
                if decode::receipt(value).is_ok() && !evidence.contains(value) {
                    evidence.push(value.clone());
                }
            }
        }
    }
    QueryReply::remote_failure(
        expected.access,
        exchange.phase != TransmissionPhase::BeforeSend,
        evidence,
    )
}

pub(crate) async fn dispatch(
    search: &dyn crate::mcp::bridge::SearchPipeline,
    method: &str,
    params: Value,
    peer_limit: Option<usize>,
) -> Result<Value, crate::mcp::bridge::BridgeError> {
    use crate::mcp::bridge::BridgeError;
    if peer_limit.is_none_or(|n| n < super::protocol::MAX_MESSAGE_SIZE)
        || !search.supports_prepared_queries()
    {
        return Err(BridgeError::Unavailable(
            "prepared queries require a capable negotiated full-frame daemon".into(),
        ));
    }
    let Some(object) = params.as_object() else {
        return Err(BridgeError::InvalidInput(
            "prepared request must be an object".into(),
        ));
    };
    if object.len() != 2
        || object.get("responseLimit").and_then(Value::as_u64) != Some(response_limit() as u64)
    {
        return Err(BridgeError::InvalidInput(
            "prepared response allowance must match the negotiated operation".into(),
        ));
    }
    let arguments = object
        .get("arguments")
        .ok_or_else(|| BridgeError::InvalidInput("missing prepared arguments".into()))?;
    if method == PAIRS {
        use crate::search::exact_scan::ExactScanError as Error;
        let request = crate::serialization::scan::ExactScanRequest::from_rpc(arguments)
            .map_err(|error| BridgeError::InvalidInput(error.to_string()))?;
        let outcome = search.scan_duplicate_pairs(request).await;
        let error = match outcome {
            Ok(page) => {
                let value = pair_reply(&page)?;
                // Exact pages are read-only; whole-page refusal keeps the caller's
                // cursor unchanged and never truncates matches or coverage.
                let full = super::protocol::DaemonResponse::success(u64::MAX, value.clone());
                if serde_json::to_vec(&full)
                    .map(|v| v.len() <= response_limit())
                    .unwrap_or(false)
                {
                    return Ok(value);
                }
                "responseTooLarge"
            }
            Err(Error::NamespaceNotFound) => "namespaceNotFound",
            Err(Error::Stale) => "stale",
            Err(Error::TooLarge) => "tooLarge",
            Err(Error::ResponseTooLarge) => "responseTooLarge",
            Err(Error::ProfileUnavailable) => "profileUnavailable",
            Err(Error::Input(_)) => "invalidCursor",
            Err(Error::Unavailable) => "unavailable",
        };
        return Ok(serde_json::json!({"wireVersion":1,"httpStatus":400,"error":error}));
    }
    if method == QUERY && arguments.get("namespace").and_then(Value::as_str).is_none() {
        return Err(BridgeError::InvalidInput(
            "prepared query requires resolved namespace".into(),
        ));
    }
    let reply = match method {
        QUERY => match crate::serialization::query::QueryRequest::from_mcp(arguments, "default") {
            Ok(r) => search.query_contract_bounded(r, response_limit()).await,
            Err(e) => QueryReply::input(e),
        },
        SIMILAR => match crate::serialization::query::SimilarRequest::from_mcp(arguments) {
            Ok(r) => search.similar_contract_bounded(r, response_limit()).await,
            Err(e) => QueryReply::input(e),
        },
        _ => {
            return Err(BridgeError::InvalidInput(
                "unsupported prepared method".into(),
            ));
        }
    };
    compact_value(&Reply {
        wire_version: 1,
        http_status: reply.http_status,
        reply: &reply,
    })
}

pub(crate) fn pair_reply(
    page: &crate::search::exact_scan::ExactScanPage,
) -> Result<Value, crate::mcp::bridge::BridgeError> {
    Ok(serde_json::json!({"wireVersion":1,"httpStatus":200,"reply":pairs::encode(page)?}))
}

/// This is an INNER ToolCallResult/daemon ceiling, not a bound on arbitrary
/// outer MCP string request IDs or HTTP headers. Public2 stays closed.
pub(crate) fn response_limit() -> usize {
    static LIMIT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| {
        let mut overhead = 0;
        for access in [false, true] {
            for sent in [false, true] {
                for evidence in [false, true] {
                    let mut reply = QueryReply::remote_failure(access, sent, vec![]);
                    if evidence {
                        reply.received_evidence = Some(ReceivedQueryEvidence { receipts: vec![] });
                        reply
                            .envelope
                            .warnings
                            .push("UNVERIFIED_RECEIVED_EVIDENCE".into());
                    }
                    let result = serde_json::to_value(reply.mcp()).expect("fixed error shape");
                    overhead = overhead.max(
                        serde_json::to_vec(&super::protocol::DaemonResponse::success(
                            u64::MAX,
                            result,
                        ))
                        .expect("fixed framed error shape")
                        .len(),
                    );
                }
            }
        }
        // Admitted compact scalar-canonical input is no smaller than its Value
        // reserialization. Escaping JSON a second time costs at most 2*raw:
        // JSON has no unescaped control bytes. Count fixed error shapes above.
        (super::protocol::MAX_MESSAGE_SIZE - overhead) / 3
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Reply<'a, T> {
    pub(crate) wire_version: u32,
    pub(crate) http_status: u16,
    pub(crate) reply: &'a T,
}

/// Only after the raw frame passed this check may the caller allocate a whole
/// Value or acquire typed receipts. Over-limit/invalid bytes remain an
/// unattributed sent response, never a proven non-commit.
pub(crate) fn admitted_frame(bytes: &[u8]) -> bool {
    shape::check(bytes, response_limit()).is_ok()
}

pub(crate) fn compact_value(
    value: &impl Serialize,
) -> Result<Value, crate::mcp::bridge::BridgeError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        crate::mcp::bridge::BridgeError::Internal("prepared reply serialization failed".into())
    })?;
    serde_json::from_slice(&bytes).map_err(|_| {
        crate::mcp::bridge::BridgeError::Internal("prepared reply serialization failed".into())
    })
}
