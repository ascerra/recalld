# Isolated checkpoint and fresh-owner replay

Measured runtime `2920dec86c3d9f7a0788ef01433a3ab4f9093bab`, branch `impl/owned-reset`, actual parent [PR104](https://github.com/calebevans/recalld/pull/104)
`a99f9eeead595fe352272719a5098f535e7581dc`. No supervisor PR109, scored-FTS optimization,
new native consumers, broad integration or public-v2 activation ancestry.

The same owned freeze now stages only canonical inputs, validates disposable
redb copies and the complete canonical snapshot, and captures actual generation,
clock and allocation state. A strict manifest pins canonical file hashes, the
shared typed receipt-inclusive logical hash, effective configuration, exact
executable/Cargo.lock, cache/rebuild policy and fixture ancestry. A fresh-process
constructor requires the checkpoint ACK hash, an absent sibling destination and
released old ownership. Existing A1 source/ancestor locks remain with the new
actual engine until its last retained worker exits. No source overlay or second
registry. The actual rebuilt owner must pass all-four-view freeze, canonical
hash and cursor verification before actor actions begin.

See [the interface and limits](interface.md) and
[changed runtime files](changed-files.txt).

## Exact isolated validation

Rust 1.94.1, image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`, network none, 2 CPUs / 4 GiB,
CARGO_BUILD_JOBS=2 with serial crate compilation, fresh source-specific private
cargo/target. Dependencies were privately cloned and `cargo clean -p recalld`
ran before builds. No source-root switch reused a package executable. No personal
home, credentials, data, daemon or socket mounts.

PASS **1009 library + 2 CLI**, fmt, clippy (exit 0 with warnings), **7 runner tests**,
**9 public-v1 episodes** (3 each: MCP stdio/REST/HTTP-MCP), **3 inherited private
process replays + 7 launcher/framing negatives**. Focused checks: 4 retained-freeze/
stage/batch, 4 ID, 1 actual native clock/replay and 1 unchanged terminal-ID regression.
Final driver status passed / exit 0, not inherited from a different source.

Actual checkpoint experiment: **3 fresh-owner suffix replays**, including one
nested checkpoint, matched exact UUIDs/references, manual clock/cursor, full
compact-v1 tool values, provider requests, generation and canonical logical hash.
There is no ID normalization. Prefix hash `631bf0d265d9670fc183d68417e0c099633acb751063c9fef18793234029b430`;
suffix hash `5a00c13f9072e0f76cd5048875e5bc26d1f8d22c8743714ca515d8fb46d8f19c`. All 13 negative cases passed: active old
owner; unknown schema/field; regressed clock; exhausted cursor; foreign generation;
configuration mismatch; unchanged existing destination; oversized manifest;
3 live-clone exclusions; unsupported writable clock rejected before publication.
The full synthetic canonical stores/receipts and transcripts are retained in the
archives. Compact responses omit scores: this does not establish raw FTS score
parity or all ranking schedules.

Public conformance retained false-read durable-file equality, default/explicit
scope, strict invalid inputs, deletion/restart and 54 returned-only exposure cases.
The 144 synthetic provider requests include 9 expected fail-closed misses. Synthetic
coverage numbers are contract diagnostics, not semantic quality or release gates.

Private example binary SHA256 `621ff77911028b19cf9fc040cbddd613a87e5f86f0c647627ce5847bd87e5eac`. Source manifest
pins 232 allowlisted inputs. Per-source binaries, full
commands, Docker resource settings, fmt/clippy output and cgroup memory events
are in the retained evidence. Cgroup oom and oom_kill were both 0; max reclaim
events were 5648. No OOM is inferred from reclamation or peak memory.

## Failed and earlier evidence

- `9dddbb4` initially failed compilation on 2 SHA-array formatting errors; source/logs retained.
- `f9f75e0` focused runtime passed 3 suffix replays / 7 negatives before ancestor retention.
- `b33094f` full 1009 + 2 / 9 public / 3 suffix / 12 negatives passed its then-current suite. It is
  not the published runtime: the separate actual clock RED advanced to 2^48,
  successfully published a checkpoint, and restore rejected it. Probe exit 2,
  exact binary hash, raw requests/responses, manifest and source are preserved.
- `eee8656` added `allocation.validate_clock(now)` before stage/publication; final
  2920dec adds accurate source comments. Actual final private control rejects the
  same unsupported clock with CONTROL_UNAVAILABLE and leaves no new manifest.

No historical pass was rewritten as a failure, and no known clock failure was
hidden by the earlier green full suite. `prior-runs.json` distinguishes driver
statuses from the separate boundary RED. The original failed format setup
(`/tmp/recalld-owned-reset-format-1qc_79fs`) also remains unchanged: an untracked
new module was correctly excluded until explicitly staged for the allowlist.

## Boundaries and remaining scope

This is `isolated-legacy-checkpoint-v1`: explicit sweeps, RIF off and autolink off.
The actual Recalld constructor hardcodes provider-local embedding caching off;
`RecalldConfig.embedding` has no cache_embeddings field. The initial provider
cache concern therefore does not apply to this constructor. Existing warm-query
and suffix observations reached the offline stub equally in all owners. No new
configuration field or provider-cache rejection was invented.

Record cache entries are cleared/drained under freeze; this does not replay Moka
admission history, telemetry, network pools or OS/FTS caches. Query latency,
provider retry jitter, arbitrary graph ties and all behavior-affecting RNG are
not claimed deterministic. Captures/rebuilds remain O(state/history), with
cooperative deadlines, not hard OS-I/O timeouts. Immutable checkpoint evidence
is a capture of one synthetic lineage, never production no-resurrection restore.

The richer supervisor consumer is separate and must retain raw scores/order.
Hume confirmed unchanged-96 tests-only source `8075219f` (runtime `387c711`): SQLite FTS5 3.49.1
aggregate/score drift across clear/rehydrate and real completion at the same generation; order
unchanged in his first fixture. His subsequent source `504f92d4` / seal `495b8fb` also
changed multi-term query order `[1,2,3]` → `[2,3,1]` after clear/rehydrate of unchanged content.
He owns the minimal fix. This compact fixture is not
evidence that the broader score-parity gate passed; the richer supervisor keeps
raw scores/order and any actual replay failure until that fix is joined.
Native/public-v2 joins, explicit native sweep schedules, complete schema/fault
matrices, general deterministic ranking/cache controls, dataset manifests/scorers
and calibrated human judging remain real prerequisites. No full A6 or quality,
performance, paid-model or training claim.

Artifacts are copied and hash-verified at
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-owned-checkpoint-2920dec`.
The final COPY-MANIFEST hash is handed off separately to avoid a self-hash cycle.
