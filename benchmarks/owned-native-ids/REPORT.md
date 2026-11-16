# Owned allocation joined with actual native producers

Measured runtime `b1ebb6892b428f65b81ab52499d817b4c6d41d19`, branch
`impl/owned-native-ids`. Exact merge parents:

* PR104 `a99f9eeead595fe352272719a5098f535e7581dc` (owned allocation/freeze).
* PR118 `f910c00d61165500c0483ec2c646de73118242d9` (actual native single/batch
  producer on PR106 plus coherent consumer PR96).

No broad integration, CAS/wire activation, PR115 snapshot/reset, catalog/export
consumer join, or performance candidate is added here. This is an allocation
consumer compatibility join. Existing default production constructors still use
SystemClock/SystemIds. The allocator remains owned by the same MetadataStore;
there is no second clock, RNG or readiness registry.

## Implemented proof

Three new actual producer tests cover generated single insert memory/operation
identity with one sampled clock, exact unchanged cursor on keyed replay/conflict;
native batch reservation/child operation IDs and generated memory IDs from one
source, including actual close/reopen and replay with an unavailable provider;
and a real post-commit vector-publication failure retaining the child receipt,
followed by repair/replay without reallocating identity or invoking the provider.
All successful paths end at the real four-component readiness barrier.

The tests begin from an existing valid synthetic native fixture, then open that
actual engine with the owned source. They do not prove deterministic native
migration/genesis, snapshot cloning, native public serving, or arbitrary whole
state replay. The closed-owner test restores an allocation cursor against its
unchanged existing store, not an unchecked-file snapshot. Public native version3
remains gated. Public checks below exercise the named legacy-v1 path.

PR104 allocation helpers merge into PR118 production paths. The single manual
merge conflict was the union of test modules with the existing owned-ID test;
no native proof/commit/publication implementation was replaced wholesale. The
initial unused 104+106 conflict attempt was aborted before manual changes and
is retained in local validation notes. Actual production compatibility uses the
already validated PR118 resolution, not a duplicate 17-file reimplementation.

## Exact validation

PASS **1086 library tests + 2 CLI tests**, **7 Python runner tests**, **9 public
legacy-v1 episodes** (stdio MCP, HTTP MCP and REST, three fresh processes each),
fmt, Clippy exit0 with warnings. The full library run took 401.65 seconds; this
is test runtime, not a performance result. Public checks include false durable
bytes, restart, default scope, malformed inputs, similarity thresholds and
returned-only exposures. No quality, release gate, public-v2 or full-A6 claim.

Focused PASS: 3 new owned-native tests; all 18 native producer regressions
(including those three); 3 freeze/control tests, 4 ID tests, existing native
one-clock/replay test and terminal-ID rejection; 3 actual private processes and
7 constructor/framing negatives. These counts overlap and must not be summed
as independent tests.

Offline Docker image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust1.94.1, 2CPU/4GiB, CARGO_BUILD_JOBS=2 and crate build -j1; no network,
home/credentials/socket mounts. Private cargo/target; package clean before the
source build. Full validation reused that exact source and source-specific
binary target without any source-root switch. Source hashes were checked before
and after. See `validation-proof.json` for executable hashes and actual command.

The follow-up full driver's manifest is in `full-output-final`; its shell
explicitly writes logs in `full-output`. Both original directories and their
contents are archived and independently indexed here. Earlier host-side source
checks stopped before Docker because six frozen harness inputs originate in the
runner worktree; the final check verifies both explicit source origins. There
was no failed crate test in this joined source. Resource cgroup events are
retained; no inference from wall time is made.

## Remaining A6 controls

The independent namespace-clock failure is separately diagnosed by
`study/checkpoint-hash-inputs` and has a separate direct PR104 fix child, without
this native producer ancestry. Exact independent startup hashes remain gated
until that fix joins the real supervisor. Native snapshot/readiness/purge/sweep
consumer joins, owned source integration with final native CAS/wire contracts,
stable equal-score ties and other behavior-affecting RNG/cache/scheduler
coverage remain explicit prerequisites. No paid models, dataset downloads,
training or personal recalld access occurred.
