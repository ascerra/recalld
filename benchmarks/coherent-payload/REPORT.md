# Coherent public search payload and observation readiness

Measured runtime `c16e2145a816e9a8354026d5eb9095866574753f` on `impl/coherent-payload`.
Final isolated validation passed **1000 library + 2 CLI + 7 Python runner tests,
fmt, clippy (exit 0 with warnings), and 9 public-v1 episodes**. Public fixture3
passed 54 returned-only exposure cases, 144 local embedding requests and nine
expected fail-closed replay misses. Raw source/executable/input/output hashes are
in the manifests; complete allowlisted source and output are archived below.
It joins registry runtime `ddd4cc2`, actual native82 startup recovery `fb804a0`,
and evidence-only registry seal `d5e4f85` at `5443c24`. The latter changes no
`src`, Cargo.toml or Cargo.lock bytes from previously measured `f296f89`.
No broad integration/catalog/export ancestry was imported.

Actual production HTTP `/search` and MCP `recall_memories` select one canonical
redb view and graph/vector/FTS/entity leases, preserving original requested mode
and one sampled query clock. `hydrate_at` compares actual GenerationRef under the
engine and metadata publication read guards, then copies selected canonical rows,
full text and requested embeddings without awaits. Mismatch rejects the whole
page, including an empty page, before exposure. Graph relations and neighbor
selection use the same pinned graph/canonical transaction. Handlers consume owned
payload without a second hydration. REST difficulty, raw strength/decay strength,
labels and canonical namespace remain preserved.

Actual get/similar/search exposures and feedback retain their accepted root
through completion. `Observed::publish_derived` rebuilds actual views, preserves
all original receipts if publication or postcommit admission fails, and never
uses a generic generation blessing. False queries perform no repair or exposure.
A startup rebuild publishes all four views before background writers or callers
are admitted. Reopened FTS is rebuilt, not certified from an unchecked file.

## Text integrity and v1 neighbor limits

TextStore reads validate range, expected length, CRC32 and UTF-8 while the engine
pins the selected handle. This parent has no `RecordSchemaV2.text` SHA field.
Native text SHA lives in typed creation content proof; strict startup validation
calls `validate_retained_text` against that proof. `hydrate_at` does not re-read
creation receipts or independently recompute that native commitment. No reproduced
wrong-valid-entry defect or corruption fix is claimed. A future stronger per-read
integrity claim needs a source-specific valid-CRC/wrong-entry negative against
the actual proof, not a fictitious schema field or an O(history) scan per query.

Named v1 intentionally retains ten-neighbor cap before canonical scope/phase
filtering and its first-five-text rule. Equal-weight HashMap/link ties are not
deterministic. Before native-v2 or deterministic-reset promotion, add an actual
fixture with >10 candidates, foreign/ghost/tombstone candidates preceding valid
ones, and equal weights for both candidate and per-neighbor edge selection;
filter before cap and order ties by stable UUID, edge and connectedTo. No v1
result or stronger replay contract is silently redefined here.

## Validation chronology and exact limits

- `c30583e` initial compile errors retained at
  `/tmp/recalld-coherent-payload-test-7lykcf7k/output`.
- `f71d92f` focused payload/transport tests passed at
  `/tmp/recalld-coherent-payload-test-vlod36fn/output`.
- `bdb4413` full 996 pass/1 fail at
  `/tmp/recalld-coherent-payload-full-pbarkbgl/output`: the existing lock-tail
  fixture awaited feedback while holding the graph lock needed by its new real
  publication. Updated fixture proves the canonical write and released storage,
  pending ready response until graph release, exact event count and stale-sweep skip.
- `6ceadbb` 998 library + 2 CLI + 7 runner/fmt/clippy passed, then public restart
  failed closed because startup had not published all four view stamps. Evidence
  `/tmp/recalld-coherent-payload-full-1rkittr7/output` remains immutable.
- `f296f89` five focused payload tests and all nine public episodes passed;
  full suite 998 pass/1 fail. Its inherited crash probe fired at the new startup
  publication before the intended deletion. Evidence
  `/tmp/recalld-coherent-payload-full-b3fhpazv/output` is retained. The probe now
  fires only with outstanding effects; the original deletion/pending receipt/
  restart/exact generation assertions are unchanged.
- `c16e214` six focused payload tests and the crash/reopen test passed. New actual
  REST/MCP/framed-daemon get/similar/search/feedback matrix covers 12 operations,
  fresh-G barrier readiness, all four current stamps, zero pending and exact
  access increments without test-side completion after an operation. Full suite,
  lint and all nine public episodes subsequently passed at
  `/tmp/recalld-coherent-payload-full-iuvswu95/output`.

All runs use Rust1.94.1 pinned recalld-ci image, Docker network none, 2CPU/4GiB,
private scratch source/cargo/target, source-specific package clean, serial crate
builds, no home/credential/socket mounts. Raw failed logs and executable hashes
are retained. Prior HOME failed-evidence COPY SHA:
`d518f2edbf55b9867b19449f3650a0f1dd4d42a622f8e65872d1605d6df989c1`.

## Remaining scope

Get/similar have publication-completion coverage, but are not yet promoted as
whole-response GenerationRef payload consumers. Public v2 activation/list-page
wire ownership remains with A4. No deterministic canonical state, snapshot/reset,
owned clock/ID/RNG, cache/config restoration, later unjoined native producers,
semantic-quality or full-A6 claim. Provider waits hold derived leases; completion
rebuilds whole record views in O(N). Barrier readiness is an observation, not a
retained freeze lease; deadlines are cooperative over OS I/O.


## Evidence and publication composition

`passed-source-output.tar.gz` contains the allowlisted source and full raw logs,
public transcripts, synthetic forensic stores, source/binary/image provenance
and cgroup counters. `prior-run-outputs.tar.gz` retains all five prior focused or
failed attempts indexed by `prior-run-index.json`. No target/cargo cache/executable
is committed. Synthetic forensic stores are not validated A6 snapshots.

Evidence-only parent alignment after the run retains the published registry
review and native82 recovery seals; all `src`, Cargo.toml, Cargo.lock, tests and
examples are verified identical to the measured runtime before sealing. Existing
registry and failed-conformance artifacts are unchanged. This is the required
completion companion to registry `ddd4cc2`; its parent alone can remain stale
following observations until real explicit completion.

Alignment head `33c562ea8f3f1b5b798ea54e217124a85d17d896` and exact checked paths are in `source-alignment.json`.
Final cgroup oom/oom_kill counters were zero; no resource-kill claim for this run.
