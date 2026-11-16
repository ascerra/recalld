# Batch postcommit receipt preservation

Base: `9ed1cb8b5d6be5dfbdf2edb73ac3a9c10faddeaa` (queue181 composition).
Owner: Huygens, isolated `fix/batch-committed-receipt` worktree. Halley explicitly
released batch adapter, BatchFailure/API1 projection, and focused test hunks;
queue runtime remains frozen. Curie confirmed zero duplicate edits/builds and
reviews only. Pascal reviews the existing Nth certification publication hook;
Feynman reviews receipt contracts. User resolved the ownership collision and
authorized one fresh approval review after clarifying Curie's source-only
restriction. That review approved the original worktree command. No bypass.

## Bounded scope

1. Reproduce actual post-TX certification publication failure while finalizing
   the first/later rejected item. Preserve the Catalog-scoped BatchItemRejected
   receipt, preceding receipts, rejection state, and unavailable readiness.
2. Reproduce actual reservation publication failure through API1 real Remote.
   Preserve validated BatchReserved evidence as reservation commitment only,
   including original-contract caller hash and exact store/scope/operation ID.
3. Use typed in-hand errors only. No poisoned-handle readback, receipt invention,
   new persistence/hash domain, exposure edits, or automatic fresh-key retry.
   Genuine commit-unknown and lost-worker outcomes remain distinct.

## Evidence before implementation

Use existing certified native fixture, accepted owner, real Remote daemon, and
`vector_publication_failure_nth` after setup. Pin immutable source, run isolated
Docker with network none, 2 CPUs/4 GiB/jobs2 and Rust 1.94.1. Reuse one owned
cache/target; seal executable hashes before package cleaning/source switches.
Prove durable receipts/progress and generation through strict reopen BEFORE
asserting response retention. Source findings alone are not executed REDs.

After the minimal mapping fix, verify first/later rejected items, preceding
successful items where applicable, reservation local/Remote/public projection,
malformed/foreign evidence, genuine unknown controls, and same-key replay with
no extra IDs/generation/history/provider calls. Then frozen full/lints and exact
handoff to Halley; no frozen-branch push or public activation.

Cleanup: protect this worktree, new run evidence, and the existing sole Huygens
cache/target until results are recorded. Release completed raw runs centrally
after publication; no additional cache clone or raw HOME archive.
