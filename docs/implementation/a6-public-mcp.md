# Serialized public MCP synthetic diagnostic

This actual consumer launches the production recalld MCP stdio process, negotiates the existing legacy MCP transport, reads `recalld://session`, and performs public store/query actions. It is explicitly `serialized-mcp-synthetic-diagnostic-v1`: public contract1, manifest/sourceMapping/trace1, system clock, no reset/barrier/native-schema guarantee. Public-v1 context reports store identity and configured embedding profile, not canonical schema, attestation or readiness. The manifest records canonical schema as null/`unattested-via-public-v1`; requiring canonical2, full-A6 or controls fails closed before episode output/runtime creation.

Actual dependencies: PR85 scorer seal70012c8 and PR88 validated runtime2e4ef99 (session context; corrected82/a698, structured13, session19 and actual snapshot dependencies). The merge preserves PR71's mcp.search measurement wrapper around the frozen structured-label body. No moving coherent-core or public-v2 activation branch is consumed. Halley owns the future same-engine constructor/private control endpoint; Huygens owns public contract activation.

The supervisor validates immutable manifest/artifact/source/executable hashes and the compiled scorer's provenance before creating episode output or launching a store. It uses a fresh private home/config/data root and the existing validated direct-mode fallback: a regular file blocks the private daemon path. No personal socket or ambient credentials/config are available. The only provider endpoint is a loopback replay cache inside network-disabled Docker. Its complete prefixed input/profile/environment keys distinguish document and query roles and reject unrecorded inputs with HTTP400; no generator/fallback exists.

Each actor step is a separate Python process running a minimal actor-only module with no evaluator fixture factory or project imports. Ingestion receives the fixed source/configuration projection; later steps receive only the current question after the previous step has acknowledged completion. All steps use the same owned production MCP session. The supervisor alone retains the future question schedule and evaluator labels; future query cache entries are loaded only at their delivery boundary. The actor receives only its current projection and MCP stdin/stdout descriptors. It receives neither evaluator labels nor evaluator-control descriptors. This proves channel projection/isolation for the synthetic actor, not an OS sandbox against malicious code in the same container. Private evaluator method names are absent from tool discovery and reject on the actual MCP transport. No alternate discovery handler is implemented.

Raw wire attempts/outcomes are retained. Successful public store UUIDs map to complete source provenance; no receipt/readGeneration is fabricated from those UUIDs. The accepted Rust source scorer is extracted unchanged into a shared example module and is invoked on actual returned UUID mappings and evaluator-only labels. The source ledger and partial wire records remain distinct from readiness. Public generated UUIDs and wall-clock fields are retained; there is no deterministic-reset claim or silent UUID erasure.

Run from this frozen checkout with private prior cache roots:

```sh
python3 scripts/public-mcp-evaluation.py host \
  --cache /tmp/recalld-PRIOR-SCRATCH/cargo \
  --build-cache /tmp/recalld-PRIOR-SCRATCH/target
```

The supervisor copies allowlisted source and private build/registry caches, uses the existing pinned local image offline at 2 CPUs/4 GiB, runs focused session/structured/performance and example checks, and executes the real episode. Unsupported controls remain a manifest gate until Halley supplies a validated frozen endpoint. Proposed future endpoint uses separate inherited evaluator-only JSONL pipes `{id,method,params}`; it is not implemented or advertised by this diagnostic. Validation is pending.

The future-query canary is part of the evaluator-only schedule, absent from executed actor code and ingestion/earlier question projections, wire requests/outcomes and provider logs until its current-question delivery. Gold-label canaries remain absent throughout actor channels. Exact executed actor source bytes and every delivered input are retained. This is fixed-state synthetic ingestion with sequential question delivery, not a chronological dialogue or semantic-quality claim.
