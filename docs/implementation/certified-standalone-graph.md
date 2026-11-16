# Standalone semantic graph certification consumer

This child composes frozen1757283a700 and139098135ad, with the exact shared
helper3f76d77e extracted ascc282956. No169/177/query/compaction ancestry.
New production admission is three lines in graph/native_producer.rs: only
Semantic selection enters with_generation_read_at(selected_G) and invokes
check_native_vector_namespace on the full canonical namespace before any vector
read. Entity, Temporal and explicit structural edge plans remain certification-
free. Automatic keys still reject before selection; explicit keys retain original
lookup/replay before current capture, policy or clock. No provider is consulted.

The callback uses the same metadata publication/read transaction, owned native
control generation, namespace equality and existing certification witness.
Outer actual engine ownership pins physical payloads; existing postselection G,
capture G and final transaction selection G/policy/endpoint checks remain. No
new cached authority, journal, hash/proof version or public routing is added.

## Parent resolution review

Local mergee6cb9d8c has six conflicts. Feynman reviewed and ACKed the exact
resolutions before broad validation:139's coordinator/cancellation leases are
unchanged;175's structural capture/check_native_policy_capture(false,false)
coexists with139's final selection_generation==actualG; module declarations keep
both sides. The175 publication/cancel/process-exit tests are preserved as
native_records/graph_publication_tests.rs with the original graph_producers
module identity and GraphRequest::Explicit argument wrappers. The139 actual
trait suite remains separately under readers::graph_producer. No test suite was
dropped. Positive fresh-insert fixture calls now use the existing real175 remedy;
retained uncertified fixture and history bytes remain unchanged. Source ACK is
not runtime proof; results are recorded separately in the evidence report.

## Actual boundary evidence

BEFORE0012508a541 failed test compilation (Tag constructor and one missing typed
Explicit wrapper); zero executed tests/binaries. BEFORE002a9304382 corrects only
those test inputs. The real Semantic trait then incorrectly succeeds on API1/2
at caps0/1: every case reads three vectors; cap0 returns None with no mutation,
cap1 commits one edge with one clock sample. One aggregate negative test fails.
The separate Entity/Temporal/explicit API1/2 structural test passes, including
full-receipt explicit replay and strict reopen. Both results remain preserved.

AFTER00357e8e119 runs six boundary tests successfully, including the same four
Semantic requests now VectorCertificationRequired with zero vector reads, zero
clock calls and unchanged canonical bytes/G. Certified positive Semantic edges,
zero cap, strict reopen, source/candidate hash-corruption rejection, exact
canonical namespace comparison, missing-owner RecoveryRequired distinction and
actual metadata feedback committed while the engine write guard is held are
covered. The unrelated metadata writer advances G without changing selected
endpoints; final graph apply rejects the stale selection and creates no edge or
receipt. Test counters are cfg(test) only, keyed by real record UUIDs and removed
on drop; observed counts make no latency or quality claim.

No ordinary native/public2 activation, full-store CAS or full A1 completion.
No query, insertion/batch autolink or creation-callback claim. Existing retained
uncertified vectors are not silently certified or re-embedded. Existing vector
integrity checks remain intact; this change adds admission, not a new vector
validator. Process-exit proofs do not establish power-loss behavior.

Final validation: frozen57e8 allfeatures/alltargets1436lib/2ignored+5CLI+10examples
pass, fmt0/clippy0warnings;65focusedexecutions plus6boundary pass. Parent175a54
then joined as90d6: one test-socket line only,2CLI/fmt/clippy pass separately.
No source/Cargo/example delta and no repeated full; final exact-head CI remains
required. Report and manifests retain each source/executable attribution.
