# Inherited global lifecycle inputs: startup participation audit

Owner Feynman; Pascal reviews A3 semantics. Source baseline is published PR143,
`0f34cd30e48b59dd655b3285d00f8b99e92c32d1`, whose real shared initializer
supports an explicitly owned native fixture while ordinary native startup stays
gated. The initial source-only audit ran while the actual135+145 consumer validated.
After its full pass/seal, test-only528cf28 reproduced the restart revision gap
through the real owned initializer; see the sealed evidence README. No runtime
fix or ordinary native serving activation is claimed.

## Actual source path

- `src/main.rs::async_main` loads configuration once before dispatch to serve,
  MCP, daemon or offline commands.
- `src/config/loader.rs::load_config` reads defaults, explicit/global TOML,
  nearest per-directory overrides, environment and CLI; it validates the result.
  It does not open canonical storage or bind a persisted policy revision.
- `src/config/mod.rs::RuntimeConfig` describes runtime-safe fields but has no
  repository callers other than its own definition/conversion. The source search
  found no hot-reload/SIGHUP implementation. A type comment is not an active reload
  protocol; do not invent one to solve this audit.
- `src/system.rs::with_owned_storage` constructs QueryEngine with
  `DecayConfig::default()` plus loaded `config.decay.decay_rate_multiplier`.
  The existing sweep runner receives that same loaded multiplier. The ordinary
  constructor feeds this initializer after opening legacy storage; the validated
  private constructor supplies an already-owned native engine. Native admission
  and provider-profile guards must remain intact.
- `src/decay/lifecycle.rs::resolve_policy` retains concrete namespace stability,
  difficulty, phase thresholds and permastore settings. `decay_rate_multiplier`
  Some overrides the global value; None inherits it. Other fields come from the
  passed immutable global DecayConfig. The TOML decay configuration is a different
  type; not every TOML field is an inherited shared-planner parameter.
- `src/search/strength.rs::StrengthContext::pin` samples the operation clock and
  resolves policy from the namespace plus QueryEngine's stored global inputs.
  It reports the namespace's persisted policy revision. ExplicitFeedbackV2
  applies current projection; legacy lifecycle retrieval keeps materialized
  strength. `get_memory_with_access(false)` reaches this real projection without
  provider work or exposure writes.
- `NativePolicyInputs` is immutable per session; capture hashes resolved policy,
  protecting stale mutation intents. This is operation fencing, not evidence that
  a different startup configuration advances canonicalG or namespace revision.
  Numeric policy writes separately advance revision; no startup linkage has yet
  been identified.

## Bounded BEFORE probe, now reproduced

Use an isolated fresh synthetic legacy store, actual schema migration with an
explicit synthetic profile attestation, then actual lifecycle-v2 migration.
Construct through the existing owned-native initializer with explicit temporary
TOML loaded through the real loader. Disable sweeps/RIF/autolink, use the same
fixed clock and one record, and make actual false get calls. Close/reopen the same
store after changing only the inherited multiplier1 to0. Compare canonical
GenerationRef, encoded namespace/record/history, policy revision, returned
projection and proposed phase. Repeat with explicit namespace multiplier1 as a
control. No network provider requests, personal config/data or serving bypass.

If the real restart changes projection while revision/G stay fixed, preserve that
BEFORE and propose the minimal canonical adoption/preview seam separately. A3
section4 requires resolved operation-frozen inputs, revision on configuration
change and phase-count preview before activation. Do not silently pin today's
provider/config as historical truth, rewrite old receipts, reset decay anchors,
or add a hot reload subsystem absent from the product. Offline metadata reads
must not require a currently compatible semantic provider.

Actual result: inherited multiplier1→0 changed false-get strength0.35273787→1 at canonicalG5/policyRevision1 and fixedclock2592000000, with identical namespace/record bytes. ExplicitSome1 control stayed unchanged. Test0PASS/1FAIL2.67s is intentional BEFORE evidence. See ../evidence/inherited-global-startup/README.md and manifest.json. Native sweep consumer1200+2 remains separately sealed; no failing audit test was added to that shipping branch.
