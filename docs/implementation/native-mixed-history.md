# Native graph/store/deletion history composition

This child joins actual PR105 native purge, PR101 corrected linked insertion,
and PR108 keyed graph no-change. It uses their existing record/proof codecs,
receipt hashes, outbox and owning engine. It adds no serving path or writer.

The composition needed two strict-history changes:

- Build namespace bindings from both live catalog and validated native purge
  start witnesses before checking graph, no-change and linked-insertion receipts
  from every permitted origin. The retained witness supplies immutable namespace
  ID/name/dimension/profile and numeric policy. Missing, ambiguous or invalid
  evidence remains an error. No foreign-origin exception bypasses binding or
  per-origin policy checks; keyed no-change still requires its original key.
- Reverse exact hard-delete and purge removed edges into the validation-only edge
  map before checking earlier graph/no-change and insertion proofs. A supposedly
  deleted edge already present is invalid. Apply only validated pending effects
  to the logical committed map first. Foreign physical edges are never replayed,
  and no deleted payload is reconstructed.

The additive dependency merge retains each parent's typed control/effects arms,
creation proof accessor, namespace/profile validation, phase history and vector
ownership rules. Same-origin terminal ordering also applies to the joined graph
and linked-insertion outcomes. Numeric policy rewinds and retired namespace
identity remain separate; a recreated name cannot rebind an old namespace ID.

Actual probes on the unresolved composition produced two failures: hard deletion
could no longer validate earlier graph edges, and purge could no longer find the
namespace for historical binding validation. The corrected source passes both.
Additional tests use real policy changes, linked insertion, changed graph,
keyed no-change replay, phase loss and hard deletion, then purge, logical
export/import and production fork restore. They assert exact retained payload,
structured labels/provenance, f32 bits, receipt bytes and ledger entry origins.
Pending purge outbox recovery is exercised against those same graph proofs,
with unfinished offline rejection and two idempotent reopens.

A restore fork gets new conservative ledger coverage. Capture also marks
unproven pre-lineage coverage Unknown rather than blessing historical writers.
The origin boundary and actual ledger entries remain. Two initial test
assertions incorrectly expected coverage flags to remain identical; those
fixture failures and executable hashes are retained separately from the two
real composition defects. No restore runtime was changed to satisfy them.

Foreign retained-history probes alter linked-insertion dimension, graph
no-change profile or policy revision, and remove the no-change key reservation.
Each altered receipt still passes its local shape decoder, but composed
canonical inspection, native reopen and logical export reject it without
changing source bytes. These are strict cross-history checks, not an outcome
allowlist.

Exact final test counts, source/tree/executable hashes and failed-run evidence
are in ../evidence/native-mixed-history/README.md. Validation is offline
Rust1.94.1 with2CPUs/4GiB and a new private target cleaned of package outputs
before the first source build. No source-root switching, credentials, personal
data, network, socket mounts or external providers.

Next: one shared validated retired namespace binding accessor for Pascal and
Huygens' existing replay-name hashes/assertions, using checked guards without
publication reacquisition. The ID/name/dimension/profile comes from existing
proofs, never a guessed name or duplicate raw parser. Their current96-only
consumers remain fail-closed with a known receipt when binding is unavailable;
the actual accessor-consumer join must close this replay gap before activation.

Ordinary native engine/system/daemon admission remains Feynman's later slice
only after participating writer and coherent-reader closure. Public routing and
capabilities remain Huygens', and the sole publication registry remains Halley's.
This child does not claim whole-query readiness or native batch participation.
