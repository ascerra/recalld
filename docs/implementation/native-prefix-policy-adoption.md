# Explicit policy adoption in the private sweep fixture

The current native physical-sweep consumer uses fixture version3 and declares
`inheritedPolicyAdoptionVersion: 1`. Before opening the evaluator or capturing a
checkpoint, its fixture builder calls the existing offline preview/apply API at
the fixture ManualClock, with the full freshly inspected source GenerationRef,
namespace scope, CAS and fixture-specific retry key. It adopts only default's
previously unknown inherited multiplier1. Foreign retains its explicit override0.
The report retains the actual preview, receipt, resulting generation and original
retained receipt key/value bytes; it verifies those bytes and the foreign namespace
remain unchanged. Export transport UUIDs are not canonical fixture state.

Versions1/2 still produce the genuine unknown-inheritance prefix and reject the
new adoption field, even null. The consumer exercises that version2 prefix through
a real failed sweep, retains the complete typed error wire and checks unchanged
state/provider count before running the separately declared adopted positive.
Its first real owner completes startup publication, acknowledging exactly four
already-applied insertion effects. The probe verifies those rows against the
original insertion receipts and preserves every other canonical row, raw record,
namespace and control generation. It records the startup logical-hash change;
startup is not claimed byte-immutable. From that completed closed baseline, the
failed sweep and subsequent fresh owner preserve the entire canonical inventory,
including effects, while failed-owner inspection remains unavailable.
Unknown inheritance does not imply every nonprojecting open/read is forbidden.
No automatic policy adoption, raw schema substitution or blanket readiness
blessing is introduced. The unchanged non-sweep native-prefix track has separate
source-specific compatibility proof; its global0 result does not certify unknown
inherited policy for dependent sweep.

This is a pinned migrated fixture, not independently deterministic native genesis,
all-RNG control, ordinary native serving, public API2 or old-vector semantic
certification. Historical measurement/checkpoint sources retain their original
fixture contracts. Integration tests and exact-head CI must pass before publication.

## Current-main integration validation

The normal main163 join preserves the text-catalog/restored-ancestry guards and
the adoption-specific retained request hash. Production Rust/Cargo at final
measured `74b44d24c452a46c1b88515e2fedf879c9d467a4` is unchanged from full-tested
`387a38856372199a3a20df8b019cc57574f90e9b`; later changes are fixture/example,
Python consumer and documentation changes.

- Full **1276 library +3 CLI tests** passed at387a, not claimed as a final74b full.
- Final74b: fmt, all-target/all-feature Clippy (warnings),30unique Python checks,
  actual startup acknowledgement and complete failed-sweep inventory proof pass.
- Adopted physical-sweep fixture: original+3restores,24actions each,12negatives,
  two canaries; exact raw suffix hash
  `533a241e0d36f533d970da7b1454c73806b5b1f0c57dbfbd2b657110433c3f38` in all four.
- Unchanged older native-prefix profile: original+3restores,15actions each,
  ten negatives,two canaries; exact raw suffix hash
  `db903be05da0f5096393eeb4ff9b76c8302ea2565e23210742b08af5f022f86a` in all four.
- Six malformed fixture inputs reject before creating roots or reports.

The failed iterations remain attributed:387a expected inspection on an invalidated
owner;452/ffa expected whole-directory immutability;8ead exposed the four legitimate
startup effect acknowledgements. No rows are normalized out of the final negative
comparison, and no passing retry substitutes for these explanations.

All execution used isolated offline Docker,2CPU,actual6GiB, Rust1.94.1 pinned image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
private HOME/cargo/target/data and no personal mounts. The nominal4GiB experiment
manifest does not establish4GiB resource conformance or performance. One serial
queue cache and package-clean source switches preserved executable provenance.
Compact reports are recorded in the integration merge ledger; completed raw runs
are released to central cleanup after publication. Exact final-head CI and green
current-main CI remain mandatory before merge.
