# Ordinary API1 point get: actual BEFORE

Frozen test-only source `1316bced1ce6fab1e854074076a671cfd5bb4205`, exact runtime parent `2925a7289055b27ffda2583a81b656442f347632` (Halley's joined validation still pending when this run started). Executable SHA256 `cd4fc189f8ff1d38b492309ce08bea93f8ae35588e0ff741132a884e20ba2705`. Source and executable were reverified unchanged after execution.

**Four tests:1PASS/3FAIL/0ignored in8.46s.** These are expected failing acceptance probes, not a green runtime handoff.

- Actual ordinary direct MCP and remote Unix-daemon native API1 get each returned materialized strength `1.0`; actual O1 captured projection was `0.99999636` (f32 bits1065353216 versus1065353155). Both false-mode file-preservation, canonical namespace despite unrelated session default, content and one-clock controls passed before the projection assertions failed. No provider was used during get; native fixture setup uses existing isolated migration/adoption/certification helpers.
- The old-peer transport harness forwarded the request to the actual isolated daemon/storage adapter, allowed its exposure to finish, then dropped only the response. One actual ordinary remote API1 true call produced **two get frames and two persisted access events**, then returned a fresh success. This reproduces the source-listed retry hazard; it is not merely a mocked method classification or a process/power-loss claim.
- The corresponding read-only lost-reply control passed: two historical retry frames, zero exposure events. This control does not justify observing retries.

No production change was made. Only two new test files plus test includes in `search/get_wire_tests.rs` and `daemon/client.rs` were committed. Existing helpers supply the real configured owner and temporary daemon; no personal store/daemon, home, credentials or socket mounts.

Build:99s, offline all-features library executable, image pinned in CONTAINER.json, toolchain1.94.1,2CPU/4GiB/jobs1; test threads2. Build touched the4GiB cgroup ceiling (`max=26725`) but OOM/OOM-kill remained0. Test memory events all0. No blind retry or resource increase. The existing one private cache/target was reused; `cargo clean --offline -p recalld` avoided the known source-root-switch stale-binary hazard. The prior frozen O1 executable was SHA-verified and retained by same-filesystem hardlink before that package clean; this BEFORE executable is also retained by hardlink in its run root. No cache clone or bulk archive.

Raw run: `/tmp/recalld-curie-o1-build/runs/017-api1-before`. All owned run containers retire with `--rm`; no host resource is released to cleanup yet. Central owner retains sole cleanup ownership. This compact package records source, executable, exact logs, commands and resource provenance. No results transfer to the future production fix, main integration or a future frame-budget diagnostic.

Next: new-wrapper/API1-emitter bound diagnostic and owner review, then literal runtime-hunk release. Shared parents remain frozen. Ordinary public API1 get closure, public2/native startup and arbitrary public JSON-RPC ID framing remain unproved by these tests.
