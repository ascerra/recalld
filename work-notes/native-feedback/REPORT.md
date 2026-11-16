# Accepted native feedback validation

Runtime `db88dbf` uses actualPR96 `387c7118864040c68d7d28c44fb2600ade520a45` only.
No forget, sweep, maintenance, purge, catalog or graph parent is imported. The new
accepted coordinator delegates to the existing native feedback planner/final
transaction, preserves the exact original scope/ID/quality/context hash, samples
only the caller-configured clock, and awaits actual current-generation publication.
The existing metadata/session API delegates to the same implementation with no
parent. No receipt codec, provenance, schema or ordinary-serving gate changes.

Seven real focused tests pass:

- Actual native exposure separates decay anchor from latest retrieval; feedback
  under nondefault caller policy matches the shared anchor plan and differs from
  the incorrect access-time plan. One clock sample and one event are committed.
- Matching replay ignores invalid current policy/time and old CAS, preserves the
  operation, and changed quality conflicts without another write or sample.
- Actual wrong vector-index dimension makes publication fail after numeric commit
  with no record-effect outbox. The original full receipt survives, current
  readiness fails, and matching retry repairs without another canonical mutation.
- Historical feedback replay after actual native forget does not resurrect or
  append history; new feedback on the Tombstone fails before time/writes.
- Invalid quality, contract, scope, explicit name and CAS reject without bytes/time
  changing, preserving typed namespace mismatch.
- Accepted descendants finish through closed root admission and fresh current-G
  barrier. Cancelling a waiter while its real worker blocks retains publication
  and graph guards plus unresolved work; readiness cannot succeed.
- The existing post-commit I/O fault fires on the actual worker after a durable
  transaction. Its unknown operation ID survives; the owner is unavailable and
  unresolved until reopen, where matching retry reconciles once and publishes.
- Actual v2-to-v1 migration preserves replay of the original feedback receipt and
  publishes the newer current generation, not the old receipt generation. A real
  backup/production fork then rejects the old lineage with no clock/byte change.

The initial compilation used the record's u8 permastore flag where the shared
planner requires bool. Exact source/log retained under compile-initial; no new
test executable ran. The corrected six-test run passed; adding real rollback/fork
produced seven passes. There is no hidden runtime retry or new defect claim.

Live namespace ID/name binding is immutable under actual catalog/policy APIs,
confirmed with Feynman and Huygens. A retired missing name requires the subsequent
shared validated purge-witness accessor; without it, the known receipt is returned
with failure rather than falsely claiming a matched replay or rewriting its hash.
Public wire and serving activation remain Huygens' separate consumer. Accepted
native exposure is next, using the existing query clock/policy and no legacy v2
fallback. No broad ReadView, performance, deterministic-ID or full-A3 claim.

Final source-specific validation: 1007 library tests and 2 CLI integration tests PASS; 7 focused tests PASS; 4 documentation tests ignored. Formatting PASS. Clippy exit 0 WITH 702 library warnings and 769 test warnings (681 duplicates), plus the existing one binary warning. All 225 runtime inputs match the sealed manifest. Seven executed/CLI executable identities are recorded; artifact-only tails do not change runtime.
