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
Unknown inheritance does not imply every nonprojecting open/read is forbidden.
No constructor auto-write, raw schema substitution or blanket readiness blessing
is introduced. The non-sweep native-prefix track is unchanged and needs its own
source-specific compatibility proof if projection now requires adoption.

This is a pinned migrated fixture, not independently deterministic native genesis,
all-RNG control, ordinary native serving, public API2 or old-vector semantic
certification. Historical measurement/checkpoint sources retain their original
fixture contracts. Integration tests and exact-head CI must pass before publication.
