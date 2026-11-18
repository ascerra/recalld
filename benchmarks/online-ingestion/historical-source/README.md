# Historical online-ingestion tools

These byte-preserved tools accompany the six-cell diagnostic measured at runtime67274c5 and sealed by07ac162. PATHS-SHA256.json maps original repository paths to their relocated copies. The original protocol, report, results and manifests remain unchanged; their source/path/hash references describe that historical execution.

They are archived source, not current-checkout Cargo examples or supported current instrumentation runners. Reproducing the old protocol requires the complete pinned historical source and its declared environment. The driver and QA expect that source layout and exact historical stage/event schema; current missing/renamed metrics must not be filled with invented values or reinterpreted as equivalent. No new diagnostic run accompanies this integration.

Current production instrumentation remains exactly the main165/173 guarded implementation, including disabled-path costs, worker context, receipts and actual-generation ACK behavior. This integration does not restore old direct trace events or unconditional counters. Existing failed instrumentation/performance gates and default-off behavior remain. The measured cells are individual traced observations, not p95, causal scaling, throughput, quality or promotion evidence.
