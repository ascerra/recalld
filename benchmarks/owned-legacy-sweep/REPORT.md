# Explicit owned legacy sweep control

Measured source `42d39a4e80996706fdd1c74a24b2cb8df7b17382`, branch `impl/owned-legacy-sweep`, direct actual
PR125 parent `0b50168b64ad353e4f7a53c6459c270fe8500070`. Frozen plans 5f07844
and 9ce164a preceded runtime edits. No PR115 checkpoint, PR124 native allocation,
FTS117 or broad integration ancestry was added.

## Change and actual path

The existing isolated evaluator now accepts private `sweep` and `sweepStatus`
controls. It calls actual `DecaySweepRunner::execute_sweep_with_work` with the
production default constants, actual configured multiplier, one sampled core
clock, accepted parent ticket and existing DerivedCompletion owner. The root
finishes before actual frozen-core readiness at the current GenerationRef.
The same busy permit covers execution and readiness worker joins. No separate
planner, transition formula, clock, registry, actor tool or rebuild is introduced.

A failed/timed-out sweep invalidates the episode and blocks sweep, advance and
capture; status remains readable with the available full known committed receipt
and uncertain operation. The in-memory report is not a durable receipt store or
aggregate commit receipt. Caller cancellation alone is different: the worker
retains ownership and may complete before its deadline, with completed readiness
recoverable in status. There is no blanket cancellation-invalidates claim.
Deadlines are cooperative, not a hard bound on blocking OS I/O.

## Exact validation

PASS 1013 library tests + 2 CLI tests; 7 Python runner tests; all 9 existing
public-v1 transport episodes; fmt and Clippy exit 0 with warnings retained.
Four new focused Rust regressions pass (included in the library count): real
one-phase sweep and multiplier-zero NOOP; deadline expiry with late worker join
and persistent invalidation; caller abort with successful retained completion;
actual postcommit effect failure preserving the full committed receipt through
error and status. Inherited freeze3, ID4, native1 and terminal1 checks also pass.

Three new actual public MCP processes use a synthetic loopback embedding fixture
and private sweep control. Each publicly stores a summary/full text, advances
`36500 * 86400000` ms, then sweeps three times at `4920825600000`:
Full → Summary → Ghost → deleted. Observed generations are 5, 6, 8, all four
component stamps equal each actual generation and pendingWork is zero. Actual
compaction is triggered on calls one and three. The three complete report arrays
match without normalization. Each process also verifies seven malformed control
requests and three false fetches leave durable bytes unchanged. Full text is gone
after summary, summary is gone after ghost, and the final fetch is absent.
Three embedding requests total, one actual public store per process; no model
service calls. Inherited private-control three-process and seven launcher/framing
negative checks also pass. Raw transcripts, canonical stores and logs are archived.

## Isolation and provenance

Pinned Docker image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust 1.94.1, network none, 2 CPUs, 4 GiB memory/swap cap, read-only filesystem,
private scratch source/cargo/target, no home/credentials/socket mounts.
CARGO_BUILD_JOBS=2 and cargo -j1. The cloned private target underwent
`cargo clean -p recalld` before this source build. Full checks reuse the exact
hash-verified source without source switching; commands, source input allowlist,
image and executed binary hashes are recorded. Full public output uses the
original runner's /work/output directory; its final summary and raw evidence are
included alongside the distinct full driver/log directory. No failed run for this
runtime was discarded. Earlier parents' historical failures remain unchanged.

## Bounded scope and remaining work

This is `controlVersion:1`, `sweepControlVersion:1`, named legacy-v1 chronological
control proof. Native sweep is explicitly rejected; broad controlledClock/RNG,
snapshot/reset and public-v2 capability claims remain false on this parent.
This fixture proves phase/content changes, not rank-quality or whole-state reset.
The helper's actual PR127 plus this source consumer must validate prefix snapshot,
original/three restored chronological suffixes and exact independent hashes,
including its existing FTS117 fix. Full native controls/readiness and A4 activation,
rich chronological datasets, broader clocks/RNG/caches/ties and external human
judge calibration remain A6 work. No full-A6 completion, model/training, quality,
performance promotion or release gate is claimed.

See [control protocol and cancellation contract](../../docsbench/owned-legacy-sweep.md).
