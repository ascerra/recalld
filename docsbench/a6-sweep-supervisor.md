# Serialized owned sweep diagnostic

New track: `serialized-owned-sweep-synthetic-diagnostic-v1`. Its only parents are
actual PR127 (`31955c3dde7f0219f2ebcebb36f57a35476038af`) and PR128
(`5deb840a21ad5f1f80006e0d05a6372919292acf`, measured `42d39a4`). The mechanical
control dispatch/capabilities union preserves PR115 checkpoint construction and
PR117 FTS rebuilding. Halley reviewed that union and the shared capture guard.

The actor consumes only its current public MCP action. The evaluator alone owns
advance/sweep/status, future queries, labels, checkpoint paths and inherited
control pipes. The existing reset supervisor owns child termination/join and the
core's fresh-owner constructor; this track adds no planner, clone, hash, clock,
reset RPC or decay formula. Autonomous sweeps, RIF and autolinking stay disabled;
the explicit decay multiplier is 1.

The named synthetic fixture uses PR128's measured 36,500-day advance, then three
real sweeps at one owned time: Full → Summary → Ghost → deleted. Public false
fetch/search actions inspect the corresponding payload/phase and namespace;
rejected actions and an actual provider replay-cache miss remain recorded. The
core's complete sweep reports, actual generations, all four ready derived stamps
and pending-work count are checked. Available errors/known commit receipts and
uncertain operation identity are preserved without inventing an aggregate
receipt. Request IDs are correlation only: each suffix uses the same unused
caller-selected range beginning at 1001, and an uncertain sweep is never resent.

The prefix is captured by the actual core. Original and three fresh-owner suffixes
must agree without normalization, including complete sweep reports, public
outputs, mappings, state, readiness, provider vectors and logical hash. A separate
changed-label run must preserve actor inputs/actions and all five prefix/suffix
hashes while changing environment/cache identity. Future-query canaries must be
absent before the current step is delivered. Source events, scorer outputs and
public trace version 1 are consumed; private controls remain in separate wire and
report artifacts. Scoring is a synthetic contract diagnostic, not semantic quality.

Run inside the isolated offline 2 CPU / 4 GiB validation container, using a build
manifest and actual executables from the same frozen source:

```sh
python3 scripts/public-mcp-evaluation.py prepare --track serialized-owned-sweep-synthetic-diagnostic-v1 --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --build /work/output/build.json --out /work/output/inputs
python3 scripts/public-mcp-evaluation.py run --manifest /work/output/inputs/manifest.json --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --source-root /work/source --out /work/output/episode-0
python3 scripts/test_sweep_mcp_episode.py --source /work/source --inputs /work/output/inputs --baseline /work/output/episode-0 --binary /work/target/debug/examples/owned_evaluator --scorer /work/target/debug/examples/score_mcp_episode --out /work/output/canaries
```

Manifest/source-mapping/trace version 1 remain independent of private control,
sweep-control and checkpoint version 1. Canonical schema remains explicitly
null/unattested via public V1. Native sweep, public V2, response ReadView,
fully-controlled clocks/IDs/RNG and full A6 remain unsupported. Original #127
studies and zero-multiplier fixtures retain their frozen source/binary pins;
their strict old capability envelope is not silently upgraded to this binary.

Validation and the preserved initial fixture failure are recorded in
[`benchmarks/a6-sweep-supervisor/REPORT.md`](../benchmarks/a6-sweep-supervisor/REPORT.md):
measured e16c109 passes both actual runs, three restores per run, and all five
independent hash pairs; identical Rust/Cargo source inherits the full 13e4dd5 run.
PR128's timeout/caller-abort proof remains owner evidence; malformed-control and
old-owner/worker/restore negatives in this consumer do not imply that every
cancellation invalidates an episode.
