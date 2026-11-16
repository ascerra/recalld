# Prepared query explanation checkpoint

Source: private146+88 composition, merge base `8edc7f0f` plus the accompanying explanation consumer patch. This is an intermediate checkpoint, not publication validation or public-v2 activation.

Offline `cargo test --all-features strict_query_`: **12 passed**, exit0. Docker `recalld-ci:1.94`, Rust1.94.1, network none, 2 CPUs, 4GiB, two build jobs; source mounted read-only. All467 source hashes matched after execution; six executed Cargo binaries are recorded. The agent-private package target was cleaned before this source-root switch, with previous executable hashes already retained.

Actual MCP/REST recall fixtures prove explanation-on/off payload equality after removing explanations, returned-score equality, returned current identities, unchanged false-read bytes, and unchanged actual exposure classification. Actual similarity fixtures reject explain:true before exposure for both recordAccess flags, return matching MCP/REST errors, and leave source bytes unchanged. Similarity is not labeled baseline-v1; its own-vector/current-max scoring is unchanged. PR146 supplies ranked recall evidence only, currently RIF Off.

The other focused cases retain current/history and namespace scope, native supplied-profile priority and provider-free metadata, complete multi-namespace observation receipts, pre-observation whole-response sizing, and full receipts after a final publication-barrier failure. The lifecycle-width fixture validates typed states/transitions and complete serialized receipt groups; it does not prove all constructed histories are reachable. The 4MiB rejection/3MiB acceptance fixture uses actual stored payloads.

Earlier draft004:11 passing focused tests on14e94b8, before146/explanation composition. Earlier draft001 and003 compile failures remain preserved in their scratch roots (visibility and test field-name errors respectively); no behavior claim derives from them. No final whole-suite, lint or exact-head CI claim is made here. Output schemas, final validation and publication remain pending. Public version2 admission remains closed.
