# Candidate009: 20 PASS, 1 test-observation failure

Exact test source a0970a042a56e9ffe4405d8753044a88b502839c,
runtime unchanged369d8e9219441a031b5502edd3d1ea435334c576.
Focused21 ran:20PASS/1FAIL40.35s; fmt passed. The sole failure is the new
unknown-outcomes test reading the tracker via storage.read().unwrap() after
fault2 intentionally panics while holding the storage write lock. Its real
frame/unknown/full known start assertions already passed before this observation.
Capture the same tracker before injection, matching the existing lost-completion
test; preserve unresolved and reopen assertions. Huygens source-reviewed this
narrow test correction. No runtime guard or poisoned-owner behavior is changed.
Fault3 in this new loop was not reached; existing lost-child test remains distinct.

Actual negotiated400-peer/32-history full child frame:1,302,161B. Old1MiB
rejected before writes/owned IDs. Maxchunk1000-row frame:1,572,891B on negotiated8MiB.
Large target/unrelated profiles reject immutably; competing ordinary graph,
catalog and second purge reject; same-parent one-row continuation passes including
peer-then-deletion exact recordGeneration increment. Historical oversized start
stays private, exact keyed replay is unchanged, and recreated name gets a fresh ID.
Empty-directory cleanup reports actual observed zero counts/presence and readiness.

OfflineRust1.94.1/2CPU/4GiB, existing package-cleaned private target; memory.max322
reclaim events, OOM0. No full-suite or publication claim. Raw complete source/log
retained at /tmp/recalld-native-api1-purge-validation/candidate009.
