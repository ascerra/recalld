# Frozen PR119: bounded graph determinism source audit

Date: 2026-11-14. Audited published commit
`8ab79b45cf53f277f96c587808615273b0055882` using `git show`, not moving WIP.
All file/line references below refer to that exact commit. This is source audit
evidence only: no builds, experiments, runtime reproductions, or semantic-quality
claims. Findings and test requirements were sent directly to Hume and main;
Hume acknowledged them as separate reproduction work.

| Scope | Exact source | Potential ordering mechanism and required proof |
| --- | --- | --- |
| Legacy V1 graph expansion | `src/search/pipeline.rs:1152–1171`; `src/search/adapters.rs:707–721,762–770`; `src/graph/activation.rs:434–440,602–609,653–655,706–717,723–748` | Both legacy and canonical-strength V1 adapters use the priority-queue activation implementation. Heap priority omits UUID ties; adjacency iteration follows stored order; nodes fire once under a finite budget; floating contributions accumulate before queueing. These mechanisms can make traversal/budget membership or accumulated values depend on insertion/rebuild order. Final result tie sorting cannot repair earlier differences. **Not runtime proven here.** Require actual coherent-query tests over equivalent canonical graphs with permuted insertion/rebuild order: an equal-priority frontier at the configured budget and a converging diamond with unequal contributions. Compare exact returned IDs/order and floating-point bits, with fixed query, owned time, policy, and canonical generation; retain failures without normalization or tolerance relaxation. |
| Enabled legacy V1 RIF | `src/search/pipeline.rs:1224–1234,1243–1245`; `src/search/adapters.rs:468–488`; `src/rif/engine.rs:129–148` | Neighbor lists are concatenated; the adapter chooses the first matching edge's type/weight; activation-only sorting precedes the neighbor cap. Distinct edge types or reciprocal stored edges and capped equal-activation neighbors can therefore select different suppression inputs/targets under changed adjacency order. Suppressions affect candidate effective strength. **Not runtime proven here.** Require actual graph permutation/rebuild cases with multiple edge types/weights and a tie at `max_neighbors`, comparing enabled V1 with Off. The adapter's raw `node.strength` is also a clock/policy projection boundary to test, not an established policy defect. |

These are V1 determinism gates, not evidence of a V2 regression or failure of
the current RIF-Off diagnostic fixture. Do not silently alter named V1 formulas
or budget membership as part of a V2 selector change.

Inspected coverage and limits:

- `src/search/read_view.rs:377–383` samples the owned clock once into a
  `ManualClock`, forwards global decay policy/multiplier, and uses the canonical
  namespace resolver. `src/search/strength.rs:40–52,69–83` applies namespace
  policy and pinned time. No clock/policy-forwarding omission was established.
- V2 graph traversal uses ordered seeds/layers (`src/graph/query.rs:110–131`),
  rejects partial adjacency contribution at its edge budget and sorts complete
  directed edge identities (`:167–178`), then uses UUID result ties (`:219`).
  `src/search/semantics.rs:227,337–346,432–445` orders candidates and intentionally
  omits RIF. This inspection established no additional V2 ordering defect.
- Canonical-strength connection bonuses sort contributions before accumulation
  (`src/graph/activation.rs:267–271,346–352`); unordered containers alone are not
  evidence of a defect on that path.
- The coherent public payload remains explicitly V1
  (`src/search/read_view.rs:202,314`). Main's previously identified unconditional
  neighbor cap/filter and HashMap tie issue around `:445–480` remains Hume's
  existing ownership; it is not a new finding in this report.

This bounded audit is closed after recording this report. It does not promote
native serving, A2/V2, full A6, or general replay determinism. The actual sweep
consumer remains the priority when its validated owner handoff is available.
