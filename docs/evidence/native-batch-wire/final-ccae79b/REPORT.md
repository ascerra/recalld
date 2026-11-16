# Native insertion batch wire / retained CAS validation

Frozen tested source **ccae79b88d348739148e8277933af95301383735**. Actual parents
are PR106 `fe99020926eb81a6994c7e9214ad4623180eeed1` and PR88
`3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e`, local join0f1b322. CAS runtime
dc2d7b9; wire/budget runtime1347f2f with test/fixture corrections throughccae79b.
No catalog, list, policy or moving writer parent. This is an actual gated
consumer, not public-v2 activation or complete A4 conformance.

## Results

| Frozen source / run | Result | Meaning |
| --- | --- | --- |
| dc2d7b9 CAS |6 passed24.74s | Original-origin proof, stale/fresh CAS, replay, concurrency, malformed proof, actual process exits and export/import/fork |
| Genuine BEFORE106 None producer |1 passed4.44s | Old runtimefe99020 plus test-only instrumentation produces old1/2/3-None bytes, real stores/exports |
| Genuine BEFORE106 oversized producer |1 passed482.40s |100 actual linked commits; old MCP response96,660,111bytes; closed completed and earlier pending stores retained |
|1347f2f first wire build |compile failed; no tests ran | Three test compile errors: omitted optional helper budget, String/&str Generation conversion, usize/u32 fixture setting |
|6b48232 focused |3 passed,1 failed58.09s | Real old oversize/immutability and fresh/frame budgets passed; strict replay fixture expectedfalse instead oftrue, the only differing field |
|540edaf corrected focused |5 passed60.15s + genuine old-None1 passed2.79s | Actual provider503/resume200, allfailed422, committed-unready207/fullreceipt/repair, strict replay and retained-byte equality |
|**ccae79b final wire** |**6 passed57.86s** | Includes strict selectors/keys/schema limits and exact budget boundaries |
|**ccae79b final CAS/history** |**7 passed28.06s** | Includes genuine pre-change encoded bytes/hashes and real old stores |
|**ccae79b full normal parallel** |**1,095 library tests635.12s +2 CLI tests2.57s/1.36s, all passed** | No test-thread serialization override;4doctests ignored |
|**ccae79b fmt / clippy** |**exit0 / exit0** | Clippy21.59s WITH782lib/859lib-test warnings(763duplicates),1binary warning; not warning-free |

The initial failed sources/logs are preserved rather than replaced by later
passes. No production float normalization was introduced to satisfy a fixture.
Replay tests change only the expected transient `replayed` flag and preserve
strict full-receipt equality; retained original receipts stay unchanged.

## Actual boundaries exercised

- Fresh worst-case response rejection precedes reservation identity/commit,
  provider work and owned clock sampling. Retained replay precedes fresh CAS or
  current settings; pending items use their original retained settings.
- Genuine old106 completed and pending batches are opened through the real
  native owner. Key and operation-ID retries use actual MCP private dispatch,
  REST prepared handler and the framed daemon connection handler. Current link
  settings0/u32MAX do not alter outcomes or resume work. Every canonical store
  byte remains unchanged across retries; provider and clock counters remain0.
- Historical oversize is HTTP400 / MCPisErrortrue `RESPONSE_TOO_LARGE`, absent
  data, batchOperationId and the full small reservationReceipt. This is
  reservation evidence only, not atomic batch success/readiness. Original item
  receipts remain persisted. No partial/empty outcome list is returned, no
  committed item becomes failed, and no new-key retry is suggested.
- Ordinary batches expose every input index as `data.outcomes`, decimal counts,
  resolved namespace identity, full receipts and structured errors. Real
  provider failure, resume, finalized invalid items and publication failure
  prove503/200/422/207 behavior. A publication-failed committed item remainsok,
  readyfalse and non-retryable-as-new; repaired replay returns the exact receipt.
- Typed synthetic worst-field serialization tests cover the complete REST
  envelope, daemon envelope, and MCP text+structured JSON-RPC response, not only
  a receipt vector. Exact limit/limit-minus-one and actual standard-budget item
  boundaries are checked. These stress shapes are not authored canonical data.
- Old1/2/3-None encodings/hash meanings remain exact. New optional reservation
  proof is private3-only and checked against the original reservation generation,
  including forked history. Children retain no parent CAS. Malformed, stripped,
  null and future proofs fail startup without source mutation.

## Isolation and provenance

Rust1.94.1, image `recalld-ci:1.94`, offline Docker `--network none --cpus2
--memory4g`, CARGO_BUILD_JOBS2. Immutable read-only source archive, this agent's
private cargo/target only, no home/credentials/daemon socket/personal data mounts.
738source files verified before/after inside Docker and again on host. OOMfalse.
Six executed binary hashes are recorded in manifest.json; library binary SHA256
`426da30f679114650cb169a883013da47abc5dcdeac7c9e1f2a6b251e0da8da4`.

Each source switch package-cleaned only this agent's target after recording the
previous executed binary hash. The final focused/full run used the same immutable
source. Genuine BEFORE producer archives match all607old git files except their
explicit test-only fixture instrumentation. Their source and executed binary
hashes are retained. No BEFORE/AFTER shared-target stale-executable claim.

Raw logs/manifests are under `/tmp/recalld-native-batch-validation`; immutable
source archives are `/tmp/recalld-native-batch-source-*` and the two specifically
named BEFORE106 archives. Committed compressed fixtures contain synthetic test
stores, not build caches or personal data. Per-file fixture provenance is under
`tests/fixtures/native-batch-before106*`. Durable HOME copy details follow in
COPY-SHA256 and the worknote entry; research tracker untouched.

## Review and remaining scope

Curie and Feynman each reviewed exact6b48232 budget/core assumptions and replay
ordering with no blocker. Those were source reviews, not independent test runs.
They checked the actual1,024-edge limit, primary+peer+at-most3advisory record
bound, fixed content proofs, bounded result/advisory fields, compact MCP framing,
and retained settings/receipts before completion/provider work.

Public2 remains closed. Current child preserves106 effects publication, not
whole-current-G readiness. Next approved actual composition is this sealed batch
branch plus Curie's validated profile-ID child of118/96: namespace-keyed single
wire, supplied-ID batch resolution, replay/None compatibility and current-G
publication after single/batch/replay/failure repair. Public supplied-profile
selectors, complete schemas/discovery, API1-on-native routing, actual A2semantics2
selection and remaining native writer/observer consumers are not completed by
this slice. See the native-api-activation handoff matrix. No merge/deployment or
performance-gate claim.
