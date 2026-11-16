# A3 positive certification setup patch — source-only handoff

Exact owner agreement covers seven test files, named setup/test hunks only. BASE-MANIFEST.json records the frozen source hashes (exposure starts after the applied four-D correction). MANIFEST.json records expected after hashes, exact changed functions, helper dependency and patch hash. Shared source, common fixture, runtime and helper were not edited.

Changes:
- feedback_consumer::setup_with_multiplier, exposure_consumer::setup, forget_consumer::setup: explicitly use the owner's real certified_fixture before existing fresh native insertion.
- sweep_tests: only setup_with_multiplier and donor/fan fresh-insertion setup. Unknown migration-marker test is byte-identical.
- replay_consumer: only retired_replay's fresh positive creation. The malformed current-purge reader remains byte-identical; genuine old ZIP replay is outside this patch and unchanged from four-D handoff.
- phase_clock: only the insertion variant of malformed_phase_rule uses certified creation. Tombstone variants and genuine old receipt coverage retain their original setup.
- maintenance: eleven explicitly named positive tests use certified_fixture before their native create/capture. All one-phase, timestamp, policy, graph, replay, fault91, terminal and malformed-witness assertions remain. The existing missing-summary/regressed-marker segment stays uncertified under its original test name and now closes/inspects immediately. Its later permastore insertion/feedback/exemption/forget portion moves intact to new native_maintenance_certified_permastore_exemption_and_forget with its own certified fixture and same 500*DAY clock.
- Source review excluded the signed-zero edge test: it seeds legacy input then remigrates, without fresh native insertion, so certification at its initial root would be unused. Its entire function is byte-identical.

Dependency: Feynman's certified_positive_fixture.rs SHA256 6a218c446a543c9d0cd3f76236207db3e0a136bd1471eb46c35ed5dc64d0e88d, imported by his tests.rs. Source-reviewed real retained-document reembedding, provider-spy assertion, closed source and fresh destination; no authored certificate or changed global common fixture. Helper was not executed here.

Checks performed: isolated git apply --check; actual patch application to seven tiny private copies reproduces every expected after hash. Existing assertion-start lines are unchanged in order; named unknown/partial/malformed/signed-zero functions verified byte-identical. This is static patch verification, not Rust compilation or runtime proof. No formatting, builds, cache copies, publication or commits. Feynman owns hash-checked integration, formatting and focused execution; any newly exposed failures must remain classified separately.

Ownership: all seven agreed hunks returned upon handoff. The isolated before/after/apply-check copies contain test source only and remain protected until the owner has recorded/integrated this patch; no runtime data or Cargo artifacts exist here.
