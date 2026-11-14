# Native single profile-ID boundary evidence

Shipping runtime `1d9e7d16915941ce6dacdaaeac63bdbd50efea28`, minimal child of PR118
`f910c00d61165500c0483ec2c646de73118242d9`. No PR111 or broad checkpoint ancestry.
Public dispatch remains gated; Huygens owns wire/batch integration.

## Genuine historical fixture

Before changing the hash/resolver implementation, a test-only exporter was added
to the PR118 descriptor-input producer in `cbb2172`. The isolated command
`cargo clean -p recalld && cargo fmt --all && cargo test --offline --all-features
native_single_profile_before_fixture -- --nocapture`, with
`RECALLD_WRITE_PROFILE_BEFORE=/work/.validation/profile-before`, passed **1/1**
in 1.65 s (compile 75 s). It wrote a real native store and its original receipt,
not a hand-authored proof. The BEFORE executable SHA-256 is
`65c26540a187090cbf193f140daa4607df01755116c90fb51aedab4c6023651f`.

The AFTER replay test copies that synthetic store to another temporary directory,
opens it through native startup, and invokes the new profile-ID producer using
the original namespace ID, caller fields and supplied vector bits. It asserts
exact historical receipt equality (only `replayed=true` in the returned copy),
unchanged persisted receipt rows and control generation, and zero provider
profile/embed calls and clock calls despite invalid current graph settings.
This fixture test is explicitly enabled by `RECALLD_READ_PROFILE_BEFORE`; a
normal CI run without that local artifact does not establish this historical
compatibility experiment. The before artifact itself remains unchanged.

## Focused checks

Initial profile checks passed **4/4**, 2.58 s. The complete single family then
passed **14/14**, 18.35 s, including the two environment/subprocess entry tests.
The profile cases prove:

- Fresh supplied vectors resolve only their canonical Bound descriptor, with
  zero provider calls; persisted vectors use existing normalization.
- Matching replay precedes fresh CAS and current settings. Scaled vectors,
  signed zero, a changed ID and changed duplicate-check intent conflict.
- Missing ID, ID without vector, full descriptor injection, wrong/malformed ID,
  zero vector and wrong width reject without generation/receipt/record/provider
  or clock mutations. Generated input with no selector uses the real provider.
- Failed publication preserves the exact committed receipt; repair/retry
  returns its original operation without provider work or receipt rewrites.
- All earlier single producer process-exit, unknown-commit and close-race checks
  remain in the focused family.

AFTER library executable SHA-256:
`f28f2ca89c595873d44fd44a56e550e0a354d5aa864b144cbce70bc378713097`.
No failed runtime test was observed in this child; parent PR118 retains
its own earlier failures. This is not a power-loss claim.

## Validation isolation and scope

Docker `recalld-ci:1.94` image ID
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
`RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`, `--network none --cpus 2 --memory
4g`, private cargo/target, no home/credentials/socket mounts. The private copied
target was cleaned before the BEFORE compilation; its source root never changed.
Only normal source edits separated BEFORE and AFTER builds. Source stayed frozen
during each run; both executable hashes and source manifests are retained.

Final command, with the genuine BEFORE read fixture enabled:
`cargo fmt --all --check && cargo test --offline --all-features && cargo clippy
--offline --all-features --all-targets`.

End-to-end native purge/reused-name replay is explicitly not tested on this
minimal parent. The resolver uses immutable namespace IDs and no live namespace
read for matching replay; actual retired-scope startup/consumer closure needs the
real native purge join and existing retained-history validator. No descriptor or
purge history is fabricated to avoid that boundary. Public activation, whole
store CAS, and FTS content-correctness closure are also outside this evidence.

## Typed diagnostic follow-up and baseline

The first frozen profile implementation, `f5728bf`, passed **1,080 library tests**
in 600.35 s, **two CLI tests** (2.70 s and 1.33 s), format, and clippy with warnings
(20.78 s). All 243 source hashes and 23 original fixture files matched afterward.

A subsequent A4 contract audit identified a diagnostic category gap: a well-formed
but mismatched profile ID must remain distinguishable from malformed ID/pair or
vector shape. Shipping follow-up `1d9e7d1` adds only the typed
`StorageError::EmbeddingProfileMismatch` in the new resolver; Huygens owns its
wire `EMBEDDING_PROFILE_MISMATCH`/409 mapping. Old private descriptor paths and
hash domains remain unchanged. Fresh malformed IDs remain invalid input, while
changed keyed replay payloads still conflict by their full hash before fresh
validation. No new wire dispatch is enabled.

The shipping follow-up passed all **14 focused tests**, 15.32 s. Its executable
SHA-256 is `fb23e131bb4d90009df08d5b493996682e230d44703d336cdeff69fe64077231`.
The preceding `f5728bf` binary and full result are baseline evidence, not a claim
that the later source was covered by that earlier run. A separate full run completed on the shipping source, with results below.


## Shipping-source full result

On exact runtime `1d9e7d16915941ce6dacdaaeac63bdbd50efea28`, the final command
exited **0**: **1,080 library tests passed**, 561.06 s; **two CLI tests passed**,
2.38 s and 1.24 s; format passed; clippy passed with warnings, 11.63 s. Four
doctests were ignored. The genuine BEFORE fixture was enabled in this full run.
All **243 source hashes** and **23 original fixture file hashes** matched after
execution. The final library SHA-256 matches the focused shipping executable:
`fb23e131bb4d90009df08d5b493996682e230d44703d336cdeff69fe64077231`.

The repository contains compact result summaries and SHA-256 references; full
logs, the three source archives, and the genuine synthetic before-store artifact
are retained in the verified HOME evidence package. No profile branch was pushed.

## Verified actual HOME package

Twenty-two files were copied/generated and verified at
`/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-single-profile-1d9e7d1`.
The before-store tar members were also checked against all original source files.
`COPY.sha256` SHA-256:
`afb890db9e18ebfda24d15a62c2035457b3206879f46a5503886f52bba8eb1b3`.
The actual HOME experiment note was appended. The copied report precedes this
copy-confirmation paragraph, and its bytes are covered by that manifest.
