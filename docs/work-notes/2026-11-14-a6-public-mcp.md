# A6 serialized MCP consumer — 2026-11-14

User accepted scorer seal70012c8 and authorized owner publication: PR85 https://github.com/calebevans/recalld/pull/85 targets PR84. Next task is an actual serialized public MCP episode consumer with actor/evaluator isolation. Main/Halley/Huygens received the parent/interface plan before implementation. Only frozen validated Huygens runtime2e4ef99 (now PR88 seal3961f66) is consumed; Halley's moving control work remains unimported.

Join52c5309 combines accepted scorer70012c8 and validated public consumer2e4ef99. Single source conflict: structured-label search body versus PR71 measurement indentation; preserve the validated body and restore the existing mcp.search measure wrapper. Inherited evidence whitespace fails unrestricted git diff --check; raw logs/patches are preserved byte-for-byte and runtime-source whitespace checks pass. This is not a runtime validation failure.

No new engine, clock, RNG, barrier or production-restore implementation. Actual existing v1 CLI direct fallback and public session resource are consumed. Schema/readiness cannot be inferred from backendContext. Runtime checks remain pending; no public-v2/full-A6/reset or semantic-quality claim.
