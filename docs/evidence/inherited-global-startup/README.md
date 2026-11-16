# Inherited global startup audit: reproduced BEFORE

Actual runtime parent is published143 `0f34cd30e48b59dd655b3285d00f8b99e92c32d1`.
Test-only source528cf28 adds a child test module; no production initializer,
policy, schema or admission behavior changes. The source trace and ownership
are recorded in docs/implementation/inherited-global-startup-audit.md.

The actual loader reads an explicit temporary TOML, then the validated owning
initializer opens a genuinely schema-migrated and lifecycle-v2 synthetic store.
The fixture explicitly attests its synthetic descriptor, keeps the real profile
guard and uses false get through QueryEngine. It does not call a provider.
Sweep/RIF/autolink are disabled. Each case uses the same store, fixed clock
2592000000, close/reopen, and changes only global multiplier1 to0. Loader output
exactly matches the supplied config. No personal home/config/data or socket
mounts; offline Docker Rust1.94.1,2CPU/4GiB/jobs2,single reused target.

| Namespace multiplier | Global before/after | CanonicalG | Policy revision | Returned strength before/after |
| --- | --- | --- | --- | --- |
| Some(1), explicit control |1 /0 |5 /5 |1 /1 |0.35273787 /0.35273787 |
| None, inherited |1 /0 |5 /5 |1 /1 |0.35273787 /1.0 |

Within each pair the storeID, encoded canonical namespace/record, original clock
and committed Full phase remain identical. The explicit control passes. The
inherited case then fails the revision-consistency assertion:0 passed/1 failed
in2.67s, test exit101. BEFORE driver exit0 only means the failed assertion was
recorded; it is not GREEN. Exact source/executable/log hashes are in manifest.json.
No OOM; source matched the immutable snapshot after execution. No full suite
was run for this intentionally failing audit.

The reproduced surface is the actual validated private owned-native initializer;
ordinary native startup remains gated. Existing ordinary legacy retrieval uses
materialized lifecycle-v1 strength, so this does not claim ordinary legacy ranking
changed or a deployed native exploit. Source tracing does establish the same
loaded multiplier reaches the shared initializer and existing sweep runner.
RuntimeConfig has no callers and no hot-reload subsystem was found.

A3 section4 requires a canonical revision change for newly adopted inherited
configuration and a phase-count preview before activation. Per-operation policy
freezing already exists; it does not make restart adoption canonical. A later
minimal fix must preserve None inheritance, original clocks/historical receipts
and unknown historical global provenance. It needs explicit canonical adoption
and preview, not a new watcher/hot-reload system or automatic reinterpretation
of historical policy. No runtime fix is claimed in this audit. Feynman owns the
follow-up; Pascal reviews lifecycle semantics.
