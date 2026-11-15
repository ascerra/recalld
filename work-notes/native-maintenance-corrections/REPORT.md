# Native maintenance proof corrections

Actual parent: PR98 `441e7b939de4c45e940c9ffdb99d042eeb831bf3`.
Peer-policy runtime: `ab8c8e72a35904a6796090ed64ca4b89b863ac1c`.
No purge, graph, catalog or sweep parent is imported.

## Historical peer-policy binding

A real native hard delete updates a surviving peer count/token and retains its
before/after lifecycle state in the payload-free receipt. After actual explicit
v2-to-v1 rollback and production fork restore, changing both peer policy revisions
from 1 to 0 while retaining v2 was accepted by inspection. The receipt's local
transition remained valid, but its policy was not backed by actual history.
`peer-policy-before/` preserves the exact source archive, test patch, raw failure
and executable SHA/mtime. BEFORE: 0 passed, 1 failed, 940 filtered. A prior fixture
compile error used a nonexistent Generation constructor; its separate log is
preserved and is not a runtime defect reproduction.

The fix reuses the existing borrowed policy/version/order validator for each
hard-delete peer's own namespace. It validates retained foreign receipts as well
as current-origin operations; generation ordering is compared only within the
same origin. Local proof already requires before/after policy equality. It does
not clone the full receipt per peer, change receipt formats, rewrite provenance,
reconstruct deleted payloads or reset original publication evidence.

AFTER focused maintenance: 14 passed, 0 failed. Both invalid policy revision and
version are rejected by strict inspection and native open without changing source
bytes; the legitimate later rollback and fork keep the original receipt unchanged.
Existing phase, replay, recovery and malformed historical proof checks pass.
Feynman owns the analogous native purge correction; Hume verified graph and linked
insertion already bind peer policy to their header, so neither is edited here.

## Cancellation lease lifetime

A separate real native decay fixture holds graph W until the accepted caller owns
publication R, holds engine W in another blocking task, then releases graph W to
queue the actual mutation worker. Aborting the async waiter incorrectly released
publication R while the worker remained blocked. The test separately confirms
the worker retained graph R and the tracker retained unresolved work. This proves
lease lifetime failure, not false readiness. BEFORE: 0 passed, 1 failed, 941
filtered. Exact source and executable identity are in `lease-before/`.

The correction transfers publication R into the existing blocking worker before
its graph guard. It releases after worker completion, before awaited publication,
even if its waiter is cancelled. No new scheduler, hooks or tracker are added.
This is runtime commit `66207fdacfd185f847b89f6ba88cc64496058cb4`, separate
from peer-policy binding. Four reader/publication tests pass, including the real
cancellation probe and committed-receipt publication failure/replay paths.

## Boundaries

Ordinary schema-2 serving remains gated. This does not certify all historical
receipt fields or provide a public keyed no-op contract. The sweep child baseline
`64261a0` (actual96+98) remains separately preserved: 1021 library + 2 CLI and 7
focused passed; fmt passed, clippy exit0 with 730 library/798 test warnings
(711 duplicates). It must consume the corrected parent before publication.

## Final validation

Corrected runtime `66207fdacfd185f847b89f6ba88cc64496058cb4`: 942 library + 2 CLI
tests PASS; 4 doctests ignored. Formatting PASS; clippy exit0 WITH warnings:

- warning: `recalld` (lib) generated 624 warnings (run `cargo clippy --fix --lib -p recalld` to apply 81 suggestions)
- warning: `recalld` (bin "recalld") generated 1 warning
- warning: `recalld` (bin "recalld" test) generated 1 warning (1 duplicate)
- warning: `recalld` (lib test) generated 662 warnings (622 duplicates) (run `cargo clippy --fix --lib -p recalld --tests` to apply 3 suggestions)

Source manifest verified before and after: `d1c2bef2f481210b1b76a58eb8629308e87d3e3998f8f6a9cb86c532eeb294b7` (212 input files).
Exact Docker settings and commands are in `validation-command.txt`. All data were
synthetic; no personal daemon/store, credentials, home or socket mounts were used.
