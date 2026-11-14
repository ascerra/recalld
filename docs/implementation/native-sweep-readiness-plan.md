# Native sweep consumer and readiness boundary

Actual parents are PR96 `387c7118864040c68d7d28c44fb2600ade520a45`
and PR98 `441e7b939de4c45e940c9ffdb99d042eeb831bf3` (maintenance runtime
`86ad274`). Join `a86d7ea` preserves the accepted producer roots, current-generation
view registry, native proofs and the optional final-transaction BatchChild argument.
The earlier 98+83 merge attempt is preserved separately as an unexecuted conflict
record; it is not validation or implementation evidence.

The existing DecaySweepRunner owns startup, interval and queued manual-trigger
registration. This change adds native dispatch to execute_sweep_with_work when
the existing metadata handle was opened through strict native validation. It
does not create another scheduler, tracker, storage session or clock source, and
does not enable the ordinary schema-2 opener.

Native execution requires DerivedCompletion for the same owning engine. One pass
samples its injected clock once, captures its ID/policy/token set before mutation,
and passes those existing intents to apply_native_decay_accepted under the
existing parent ticket. Each namespace capture retains its truthful read generation;
there is no claim that separate namespace snapshots share a global read generation.
One ID advances at most one destructive phase per pass. Final canonical donor/fan
reprojection and exact raw timestamp/anchor preservation remain in PR98's writer.
The captured phase is private diagnostic context, not caller hash identity.

Every known native commit awaits actual publication. Counts/events describe known
canonical commits even when publication fails. The first full committed receipt
or unknown-operation identity remains in SweepResult, and the native loop stops
after failure rather than scheduling more fresh unkeyed writes. The outer accepted
work controller retains its existing completion/finish rules. Cancellation and
unknown outcomes cannot be reported as drained work.

Background maintenance uses explicit internal unkeyed contract-2 contexts. Skipped
unchanged/stale/exempt/missing-summary outcomes remain diagnostics, commit nothing
and bind no key. Future public/background adapters must not promote a supplied-key
skip to durable successful keyed no-op semantics without a typed finalization
contract. A policy/graph A -> B -> retry must not silently reinterpret earlier
success. Legacy-specific permastore/multiplier counters are not synthesized from
the native aggregate exempt diagnostic.

Native execution does not enter legacy text compaction or side-channel phase-index
persistence. The canonical writer and awaited publisher own its effects. Projection
still scans canonical namespace rows/edges per record; this is not a performance
claim or ordinary-serving promotion.

Validation targets actual startup/manual passes through the fresh-generation
readiness barrier (all four views), closed-admission descendants, one clock sample,
phase/vector retention and strict reopen, current donor/fan changes while the target
token stays unchanged, partial publication failure plus explicit repair, cancellation,
queued shutdown, and refusal without completion handles. Tests use real migration
and native mutations, not authored native rows or invented registry stamps.
The interval check uses the already-present tokio-test scheduling clock to advance
the real 60-second timer, separately from the injected retention clock. Migration
marker checks use real migration with a later unknown phase marker and two actual
passes with regressed sample time. No new test dependency or timer override is used.
