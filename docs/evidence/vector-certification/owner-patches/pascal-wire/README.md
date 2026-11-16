# Explicit certified wire positives after run035 — source-only patch

Feynman reports run035 finished 237 passed / 3 failed / 1 ignored. Those three paths stop at fresh native insertion before the intended publication/decoder boundary. This patch is not an independently executed fix.

Owner agreed exactly three files:
- wire_tests.rs adds setup_certified. It first executes existing setup (actual selected lifecycle/adoption), drops both adapters and closes the owner, then invokes the existing real certify_positive helper on that Data. It reopens using the same clock/default policy, rebuilds and constructs the existing configured adapters. Existing setup/setup_policy bodies are unchanged.
- forget_origin_tests.rs uses the helper only for the known-publication-failure survivor insertion.
- public_api1_tests.rs uses it for the 32-insertion retained-edge/Remote decoder test and ONLY Operation::Forget in the first publication-failure loop. The feedback branch and separate FaultClock unknown-commit loop remain uncertified.

All prior assertions remain unchanged. No old receipt, policy, Ghost, terminal or uncertainty guard is weakened. No common fixture, runtime or certification helper changes. The helper source snapshot/hash is in reference/ and MANIFEST.json; it is source reviewed, not executed by this agent.

BASE-MANIFEST.json records exact three frozen before hashes; MANIFEST.json adds expected after hashes and patch SHA256. Static checks: git apply --check and application to three small isolated copies reproduce expected hashes; default setup/policy and FaultClock segment verified unchanged; existing assertion-start lines remain in order. No Rust compile/test/fmt, container, cache copy, shared-source edit, commit or publication. Feynman owns hash-checked application, formatting and focused/full validation. Keep this tiny isolated source snapshot until the owner records its copy.
