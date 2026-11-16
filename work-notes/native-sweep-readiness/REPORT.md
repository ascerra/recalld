# Native sweep consumer validation

Actual parents: PR96 `387c7118864040c68d7d28c44fb2600ade520a45` and PR98
`441e7b939de4c45e940c9ffdb99d042eeb831bf3`, joined at `a86d7ea`.
Sweep baseline `64261a0` passed 1021 library + 2 CLI tests, 7 focused sweep tests,
fmt, and clippy exit0 with 730 library/798 test warnings (711 duplicates).
Its source, executable hashes/mtimes and raw logs are preserved under `baseline/`.
The initial actual98+83 merge attempt is an unexecuted conflict record under
`unexecuted-98-83-join/`, not runtime validation.

Corrected runtime join `38e8598` adds actual PR98 peer-policy proof `ab8c8e7`
and cancelled-worker publication lease `66207fd`. No purge, graph or catchall
integration checkpoint is imported. The native consumer uses the existing runner,
accepted producer roots, final canonical maintenance transaction, publisher and
actual current-generation registry. It does not create another scheduler/store.

## Actual checks

Seven focused tests pass on the corrected join. Fixtures migrate actual legacy
stores, select lifecycle policy, perform native writes and reopen strict storage.
They do not author native canonical rows or forge readiness stamps.

- Startup/manual passes coalesce queued work before execution, capture one clock,
  advance one phase per ID, preserve raw lifecycle fields, retain Ghost vectors,
  delete eligible Ghosts, await all four current-generation derived views, and
  complete descendants admitted before the barrier closes root admission.
- The existing real 60-second timer registers work only at its tick. The already
  available Tokio test scheduling clock advances the timer independently of the
  injected retention clock; no scheduler override or dependency was added.
- A wrong actual vector-index dimension causes a post-commit publication failure.
  The first full receipt is retained, further unkeyed writes stop, readiness fails,
  and explicit repair publishes without another canonical generation.
- Real donor feedback and fan-out insertion commit after capture while target
  tokens stay unchanged. Final canonical reprojection preserves Full when the new
  bonus prevents loss, uses the original clock, and covers strength-only updates.
- Cancellation/queued shutdown retain unresolved accepted work. A missing actual
  completion handle rejects before canonical mutation.
- Real migration sets an unknown later phase marker; regressed sweep clocks keep
  raw timestamps/anchors and clamp the marker through actual phase transitions.

Native sweeps use internal unkeyed contract-2 contexts. Skips do not bind keys and
must not be promoted to public durable keyed no-op success. Native paths never
call legacy text compaction or side-channel phase-index persistence. There is no
whole-store promotion or performance claim; ordinary schema-2 serving stays gated.
Per-record canonical namespace/edge scans remain an explicit cost. Huygens owns
public wire parsing/envelopes; Halley owns controlled-time/readiness integration.

All validation runs use recalld-ci:1.94 / RUSTUP_TOOLCHAIN=1.94.1, Docker --network
none --cpus 2 --memory 4g, CARGO_BUILD_JOBS=2 and private cargo/target mounts. No
home, credentials, Docker socket or personal store is mounted. Initial package
clean after the worktree/cache switch is recorded. Subsequent runs use this same
private worktree/target with source manifests checked before and after.

## Final corrected join result

1023 library + 2 CLI tests PASS; 7 focused PASS; 4 doctests ignored. Formatting
PASS; clippy exit0 WITH warnings:

- warning: `recalld` (lib) generated 731 warnings (run `cargo clippy --fix --lib -p recalld` to apply 84 suggestions)
- warning: `recalld` (bin "recalld") generated 1 warning
- warning: `recalld` (bin "recalld" test) generated 1 warning (1 duplicate)
- warning: `recalld` (lib test) generated 799 warnings (712 duplicates) (run `cargo clippy --fix --lib -p recalld --tests` to apply 6 suggestions)

Runtime `38e8598` inputs remain identical after evidence-only merge `55cd8d1`
of published PR98 `8605999`. Source manifest SHA-256:
`974d9a7ade8fe844dccb172f2eac5c5590ff03bf211ac48c21dd38a06522d415` (230 files), checked before/after.
No runtime edits or test retries followed this passing full run.
