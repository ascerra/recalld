# Coherent payload / bounded RIF consumer

Exact measured source `ce67c24785c2b434dc6457fe87260c7824c182cf` passed
**1,017 library + 2 CLI + 7 Python runner tests, 18 focused RIF + 6 focused coherent
payload tests, fmt, clippy (exit 0 with warnings), and 9 public-v1 episodes**.
The public fixture3 run made 144 local embedding requests, including 9 expected
fail-closed misses, and passed 54 returned-only exposure cases.

## Exact dependency relation

The only direct parents are published PR #96 payload seal
`387c7118864040c68d7d28c44fb2600ade520a45` and published PR #89 bounds seal
`58223dacb99297a00d2baa9059fb2d826a9f8dba`. PR #96's measured runtime is c16e214;
its evidence alignment preserves those source bytes. This PR is based on #96 with #89
as the additional required dependency. No catalog #93, native batch #91, or numeric policy #94
checkpoint parents are imported; `composition.json` records the ancestry checks.

The integration-specific patch beyond additive parent merge resolution is two
runtime files: `src/search/read_view.rs` carries `self.rif_mode` into the pinned
QueryEngine; `src/search/adapters.rs` uses synchronous pinned-aware
`ReadHandle::read()` in `rif_edges`. Default Off, operator mode selection, namespace
filtering, budgets, formulas and error mapping are unchanged. A pinned query reuses
its existing lease rather than trying to acquire the live graph lock again.
Halley reviewed the coherent constructor; Hume reviewed the bounded RIF consumer.

## Relation to earlier integration evidence

A broader validation-only checkpoint first exposed the missing initializer field
and incompatible block_on call. Its frozen failed source 1e34e426 and raw compile log
are retained; this directory includes that log with its distinct source context.
Checkpoint runtime 972151a3 subsequently passed 1,094 library + 2 CLI + 7 runner tests + 9 public episodes and was
sealed locally at 3f106d3bc4ce877ad9857779d5ab5de11c5278f3, with HOME copy recorded in
the notebook. That checkpoint also contains separate catalog/policy readiness
gates and batch completion evidence. Its fixture corrections are not imported.
The four implementation paths listed in `composition.json` are byte-identical
between that checkpoint and this minimal source. This package independently
validates the exact minimal source; it does not substitute the broader run.

## Reproduction and provenance

Run `/tmp/recalld-coherent-rif-ce67c247-6cmp8pfm`. `source-output.tar.gz` contains
complete allowlisted source, actual commands/driver, all raw logs, public transcripts,
synthetic forensic stores and source/executable/image provenance.
`archive-members.json` verifies every archived member. No target/cargo cache or
executable is committed. The source and each executed test/public binary are
hashed before and after their run and match. Cgroup OOM/oom_kill counters are 0.

Pinned image sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef,
Rust 1.94.1, Docker network none, 2 CPU/4 GiB, -j1, immutable read-only source, independent
source-specific target. Dependency artifacts were cloned from the checkpoint;
`cargo clean -p recalld` precedes recompilation. No ambient HOME/config/credential
or socket mounts. The existing separately hashed fixture3 public harness is
consumed without importing PR #90/#92 or private endpoint #99.

These are synthetic legacy-v1 contract checks. Semantic quality is not measured.
Native-v2/full-A6/reset/owned clock/IDs/RNG and later unjoined writer guarantees
remain outside this consumer. Public output/readiness inherits the frozen #96
scope; the RIF experiment retains its original operator gates.
