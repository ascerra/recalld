//! Evaluator-only JSONL on caller-owned pipes, sharing the actor's actual core.
//! This is a bounded v1 diagnostic control plane, not a snapshot/reset facility.
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::Recalld;
use crate::storage::control::GenerationRef;

const MAX_REQUEST_BYTES: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    method: String,
    params: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Barrier {
    target: GenerationRef,
    deadline_ms: u64,
}

fn error(code: &'static str, message: impl ToString) -> Value {
    json!({"code":code,"message":message.to_string()})
}

async fn dispatch(core: &Arc<Recalld>, request: &Request) -> Result<Value, Value> {
    match request.method.as_str() {
        "capabilities" => {
            if request.params != json!({}) {
                return Err(error(
                    "INVALID_PARAMS",
                    "capabilities requires empty params",
                ));
            }
            Ok(json!({
                "controlVersion":1,
                "methods":["capabilities","inspect","barrier"],
                "publicContract":"legacy-v1",
                "guarantees":{
                    "barrier":"owned-work-and-required-derived-generations",
                    "logicalHash":false,"snapshot":false,"reset":false,
                    "controlledClock":false,"controlledIds":false,"controlledRng":false
                }
            }))
        }
        "inspect" => {
            if request.params != json!({}) {
                return Err(error("INVALID_PARAMS", "inspect requires empty params"));
            }
            let storage = core.storage().clone();
            // Register this readonly worker before enqueue. It does not touch
            // caches, start another session, or label a latest counter a ReadView.
            let work = crate::storage::work::accept_work(&storage, None)
                .map_err(|e| error("UNAVAILABLE", e))?;
            let state = tokio::task::spawn_blocking(move || {
                let result = storage
                    .read()
                    .map_err(|e| e.to_string())
                    .and_then(|engine| {
                        engine
                            .meta_store()
                            .publication_state()
                            .map_err(|e| e.to_string())
                    });
                crate::storage::work::finish_work(&storage, work);
                result
            })
            .await
            .map_err(|e| error("UNAVAILABLE", e))?
            .map_err(|e| error("UNAVAILABLE", e))?;
            Ok(json!({"committed":state.canonical,"logicalHash":null}))
        }
        "barrier" => {
            let params: Barrier = serde_json::from_value(request.params.clone())
                .map_err(|e| error("INVALID_PARAMS", e))?;
            if !(1..=30_000).contains(&params.deadline_ms) {
                return Err(error("INVALID_PARAMS", "deadlineMs must be 1..30000"));
            }
            let deadline = Instant::now() + Duration::from_millis(params.deadline_ms);
            let report = core
                .readiness_barrier(params.target, deadline)
                .await
                .map_err(|e| error("BARRIER_UNAVAILABLE", e))?;
            serde_json::to_value(report).map_err(|e| error("UNAVAILABLE", e))
        }
        _ => Err(error("UNSUPPORTED_CONTROL", "control is not implemented")),
    }
}

/// Serve one serialized evaluator stream. The caller must provide pipes separate
/// from actor MCP stdin/stdout; this function never changes actor tool discovery.
/// EOF ends the evaluator channel. Oversized frames fail closed and end it too.
/// Deadlines are cooperative; blocking OS I/O has no hard real-time bound.
pub async fn serve<R, W>(core: Arc<Recalld>, input: R, mut output: W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut input = BufReader::new(input);
    loop {
        let mut line = Vec::new();
        let count = (&mut input)
            .take(MAX_REQUEST_BYTES + 1)
            .read_until(b'\n', &mut line)
            .await?;
        if count == 0 {
            return Ok(());
        }
        let oversized = count as u64 > MAX_REQUEST_BYTES;
        let unterminated = line.last() != Some(&b'\n');
        let reply = if oversized {
            json!({"id":null,"error":error("INVALID_REQUEST","control frame too large")})
        } else if unterminated {
            json!({"id":null,"error":error("INVALID_REQUEST","unterminated control frame")})
        } else {
            match serde_json::from_slice::<Request>(&line) {
                Err(e) => json!({"id":null,"error":error("INVALID_REQUEST",e)}),
                Ok(request) => match dispatch(&core, &request).await {
                    Ok(result) => json!({"id":request.id,"result":result}),
                    Err(error) => json!({"id":request.id,"error":error}),
                },
            }
        };
        output.write_all(&serde_json::to_vec(&reply)?).await?;
        output.write_all(b"\n").await?;
        output.flush().await?;
        if oversized || unterminated {
            return Ok(());
        }
    }
}
