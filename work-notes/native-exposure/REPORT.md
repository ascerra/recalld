# Accepted native exposure validation

Corrected runtime `3d8b0090ab3db3cc2c305d06d1c4ffe2a3e1e6c5` uses published PR113
`149beb60365717f706b4a5461564a03a05bbb492` and PR116
`ea4354431b2c7cd6033cd9851854f597922b1f0d` only. PR116 runtime is `8dd1cc7`.
No optional-delta optimization, feedback, forget, maintenance, sweep, purge or
broad integration checkpoint parent. Ordinary schema2 serving remains closed.

Nine focused tests pass: eight new consumer regressions plus the existing real
native both-policy exposure/feedback core test. They exercise:

- Query supplied-time/deduplication, direct-over-associative history classification,
  no exposure of scope source, unchanged v2 anchor/feedback/stability/phase,
  regressed-clock clamping, repeated false-mode byte equality, one sampled search
  clock, actual publication and strict reopen.
- Real HTTP and MCP get/similar true and false calls on migrated/native stores;
  each true result observed once, source excluded from similarity observation,
  provider never called, false bytes unchanged and current views published.
- Full original receipt retained on real wrong-vector-dimension publication failure;
  explicit repair restores readiness without another access event.
- Actual durable commit reported as I/O uncertainty: unknown operation ID retained,
  owner unavailable, reopen finds the original receipt and exactly one event.
- Deferred coherent true selection copies payload/readGeneration without writing;
  unsupported recorder still rejects true. Later original observer commits at a
  newer generation without relabeling capture as the earlier query snapshot.
- Two genuinely migrated namespaces: the first valid batch commits once, second
  terminal batch fails; the error retains the preceding receipt, completion is
  awaited, the tombstone is byte-identical and a fresh readiness barrier succeeds.
- Accepted descendants finish with root admission closed. Cancelling an async
  waiter while its real worker is blocked retains publication and graph leases;
  unresolved work prevents readiness. No broad cancellation guarantee claimed.
- Caller-configured invalid policy rejects before canonical mutation; empty input
  records no event/receipt/work and does not sample another clock.

The original policy/time inputs are forwarded through the existing observer,
not reconstructed from defaults. Native receipt results use full publication;
there is no incremental hint or performance claim. Core native planner, proof,
receipt codec and schema are unchanged. Per-namespace capture generations are
truthful post-selection snapshots, not whole-query CAS or cross-namespace atomicity.
Internal requests are unkeyed; public keyed queries need caller-intent hashing,
replay before fresh selection and current-visibility hydration in a later A4 layer.

Preserved unsuccessful attempts:

1. First compiled source: 3PASS/2FAIL. Deferred pinned recorder lacked the original
   native capability; the other assertion expected an unclamped timestamp despite
   insertion having advanced lifecycle time. Exact source and executable hash/mtime
   are retained under attempt-1. The correction forwards capability only: every
   pinned write method still rejects; the fixture checks max(sample, previous).
2. A subsequent old unit-struct constructor in the existing read-view test failed
   compilation. No new executable ran. Source/log retained under attempt-2; the
   fixture explicitly keeps native_writer_available=false.
3. Docker rustfmt reported a truncated source read ending at line950/asse while the
   host file remained complete at 36761 bytes. Formatter failed; no test ran. Raw
   error and complete host bytes/hash are retained under format-3. Every mounted
   source hash was then checked before formatting/build. No runtime defect claim.
4. Next compiled source: 7PASS/1FAIL. Metadata-only query fixture omitted its mandatory
   filter and correctly received EmptyQuery. Source/binary identity retained under
   attempt-4. Added the required filter without changing query validation.
5. Focused run: 9PASS. The first full source3955 run then finished989PASS/27FAIL,
   with CLI/clippy not reached. Unconditional inferred-time forwarding changed
   legacy-only recorder dispatch and bypassed a legacy configured clock. All eight
   new native consumer tests passed within that full run. Exact full log,27 failed
   names, source and executable identity are preserved under full-before.
6. Runtime3d8 restricts inferred-time forwarding to native-capable recorders. It
   preserves old legacy method/clock/similar preflight and explicit pre-sampled
   APIs. No legacy test/assertion changes. All27 exact failures now PASS and all9
   native focused tests PASS; final full-suite/fmt/clippy results are recorded below.
   The final run verifies all226 mounted runtime source hashes before testing.

No personal data, credentials, daemon or MCP memory tools were accessed. Docker
runtime network is none, limited to2 CPUs/4GiB; only this worktree and private
cargo/target are mounted. The copied cache was followed by cargo clean -p recalld
on this OWN target before the first compile. Later runs use the same source mount.

Final corrected source validation: 1016 library tests and2 CLI integration tests PASS;9 native focused tests and27 exact failed legacy tests PASS;4 documentation tests ignored. Fmt PASS. Clippy exit0 WITH699 library/770 test warnings (680 duplicates), plus the existing one binary warning. All226 source inputs match the frozen manifest; seven executed/CLI executable identities are recorded and the corrected library executable differs from the failed3955 run. Final test runtime504.78s is a validation duration, not performance evidence. No further source changes are included in evidence/doc tails.
