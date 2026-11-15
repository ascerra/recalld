# Actual public-v1 conformance consumer

Validated consumer source `0978473b92f07ab9fee287d20e16347005292e55`, child of accepted PR90 seal `003d857`. The necessary PR85/PR88 runtime dependencies are inherited through PR90; no newer runtime/control branch was imported. All production Rust, Cargo files and scorer logic are unchanged. The new manifest track is `serialized-mcp-synthetic-conformance-v1`; earlier PR90 artifacts remain untouched.

| Actual check | Result |
| --- | --- |
| Python boundaries / existing scorer example | 10 / 3 passed; format passed |
| Library evidence | Exact source/Cargo hashes match PR90's passing 25 session + 3 structured + 3 performance tests; no library suite repeated |
| Conformance baseline and gold-label variant | Both passed: each 53 MCP actions, 41 current-step deliveries, 14 provider requests and one actual same-root process restart |
| Rejected actions | Each retains all 11 errors: six malformed qualities, three namespace mismatches, one rejected cross-namespace correction and one expected replay miss |
| Successful feedback | Qualities 1, 2, 3, 4 reach the existing production MCP tool; public IDs/strength/stability checked, no reward/training claim |
| Correction and source provenance | Supersedes response has original public UUID and `applied`; original/corrected UUIDs both retain source mappings; current replacement appears in search before explicit forget |
| Forget / restart | Old and separately forgotten records absent from searches after acknowledgement and restart; by-ID metadata is Tombstone with erased summary/fullText/tags and retained UUID |
| False fetch/search | Entire exposed public record equal before/after false search/fetch across a real 1.1-second interval; erased tombstone metadata and live correction unchanged across restart |
| Namespace / persistence | Cross-namespace fetch/feedback/forget fail; foreign source remains unchanged and isolated in search. Two distinct child PIDs exit0 using the same private data root and public store/profile identity |
| Source ledger/scorer | Five source turns processed, four accepted and one rejected; 15 source events, no pending/unseen. Nine queries: seven scored, one zero-label and one unknown |
| Negative manifests | 14 rejected before episode output/store creation, including old-track/new-fixture mismatch and unavailable controls/native schema |
| Label and future canaries | Actor code/cache bytes/provider bodies unchanged; all 53 requests and 41 projections match after resolving only generated public IDs to their acknowledged source mappings. Scores change, returned-source ranks do not. Future question withheld until delivery39 |
| Original diagnostic route | Same new binaries/source also pass its 15-action, four-source original fixture once; original PR90 evidence was not rewritten |

The supervisor runs the actual production `recalld ... mcp` CLI in private direct mode. The separate actor module imports only the standard library, receives a single current action, and never imports the evaluator fixture/schedule factory. Expected outcomes and evaluator labels stay supervisor-only. Every raw request/outcome, generated UUID, timestamp, public failure, process event, projection, mapping and scorer result is retained. Private evaluator descriptors remain absent from actors and evaluator methods reject on public MCP. The supplemental checker compares generated IDs only via real per-run mappings; this is not a byte-identical-response or deterministic-reset claim.

The accepted supersedes relation is tested before deletion; it is not called a delete. Public-v1 fetch intentionally exposes content-free Tombstone metadata with the original UUID. Whole serialized search responses were also audited to exclude deleted UUIDs after the corresponding acknowledgements. False-read evidence covers exposed public fields including lastAccessedAt; no private counter, canonical generation or hidden-observation claim is inferred. The ledger's accepted mappings are historical provenance, including later-forgotten records, not a current-live-record census.

The first consumer source `5d8dddce3a7e4e3ce5ca33a7ad10adef15a2043e` passed Python/scorer checks but its actual ingestion failed closed: the synthetic replay fixture pinned summary-only inputs while production `McpStorageAdapter::store_memory` composes summary + fullText + tags (`src/mcp/bridge_adapters.rs:1067`). Its first store returned an actual unavailable/provider-miss error; no store mapping was accepted. The correction pins the complete observed production request and does not introduce generation/fallback or runtime changes. That failure's seven attempted/outcome actions, logs and source-bound metadata remain in `failed-5d8dddc.tgz`.

The successful host command uses `scripts/public-mcp-evaluation.py host --track serialized-mcp-synthetic-conformance-v1 --cache /tmp/recalld-canonical-harness-test-a1hpl2g2/cargo --build-cache /var/folders/lg/pd7snpz15137ng972h102kqc0000gp/T/recalld-public-mcp-u250qe0k/target --library-evidence /var/folders/lg/pd7snpz15137ng972h102kqc0000gp/T/recalld-public-mcp-zwoern3z/output`. Exact source pins and inherited log hashes are in build.json. All executions use network-none Docker, 2 CPUs/4 GiB, private explicit home/config/data, loopback synthetic replay and no personal socket/credentials. The supplement mounts source/binaries read-only and performs no builds. The scorer executable was rebuilt only to bind changed consumer provenance; no new production binary was needed.

- Production binary SHA256 (identical to PR90): `d77dab88431bf14e9e8f23d42d5966dbd8863c7cc9885e41f75d44f6ed01343a`.
- Scorer binary SHA256: `cfb7bd719e5ba2dd236972c5f29f8fd11c79fdaf4de674560fc4d651313ab5f9`.
- Manifest SHA256: `ab165d27d5ef22e484600a235e5ca67ff39039cc7005dfbcc742291038048f73`.
- Executed actor source SHA256: `8db20c032532474b54935bb11bc65cb0e288d1812763485af5c604a0804ee7b1`.
- Pinned image: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.

This is synthetic contract conformance, not upstream dataset/scorer semantics or semantic quality. Public-v2, ReadView, barrier, reset, keyed retries, frozen clock/RNG and full A6 remain unsupported. Ordinary EOF shutdown/reopen is not crash recovery or reset. Halley's actual frozen private-control constructor is still awaited; no substitute control framework is implemented. These are actual episode results where stated, separately from the source-supported v1 terminal contract and inherited runtime test evidence.
