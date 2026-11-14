# Actual legacy online insertion diagnostic

This diagnostic is based solely on PR #75 (`8ecdd5a4b20c1f1555b98c84bd0771625e51c67d`). It exercises the production single-insert and DerivedCompletion path. It neither changes durability/publication nor imports native or batch serving.

See [PROTOCOL.md](PROTOCOL.md) for the frozen six-cell workload and interpretation limits. The typed explicit-vector adapter control is not MCP wire. The MCP tool dispatcher uses a deterministic local provider; the daemon path uses real Unix-socket RPC and explicit vectors. These paths must remain separately labeled.

The probe is `examples/online_ingestion_probe.rs`; the runner and streaming QA are `scripts/online-ingestion.py` and `scripts/online-ingestion-qa.py`. Run only in the isolated container described in the protocol. The probe refuses ambient configuration and a reused store. It preserves actual responses/errors, checks committed generation and all derived views after each acknowledgment, and writes complete warmup/measured observations. Strace preserves actual sync calls from every thread/child. No public diagnostic schema or embedding wire field is added.

A5 remains incomplete: the full-tracing and sampled-query overhead gates remain failed; this diagnostic makes no overhead or performance promotion claim. Unmeasured work includes external MCP stdio/process boundaries, network providers, parallel/load-sensitive queues, online batches, ordinary native serving and native creation-history scanning. Individual write times under full tracing plus ptrace are diagnostic observations, not p95, confidence intervals or production capacity. A later optimization requires a separate approved comparison and durability/publication parity.
