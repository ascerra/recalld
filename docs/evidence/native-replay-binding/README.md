# Checked native replay-binding evidence

Validated source `55bcf43db4518a3c4d6d5bc4329141394813b984`, tree `951ab3592ffcdc24cf9669256016a05b4085df33`, actual PR111/23f99c5 parent. Shared
borrowed retired witness authority is consumed by strict lifecycle history;
MetadataStore capture is handed to the actual Pascal/Huygens replay adapters.
No new codec/hash, provider attestation, publication owner or serving engine.

**23focused passed**:4namespace-binding tests(3new,1existing),5mixed-history,
14purge. `cargo fmt --all` completed; all-target Clippy exit0 WITH732lib and
765libtest warnings(725duplicates). Exact timings/command in final-manifest.json.
No full1004-library claim: parent PR111's1001+2 run is separate evidence.

Actual capture holds a checked MutationGuard and an unchanged write transaction,
returns matching current GenerationRef without reacquiring publication, and
keeps original ID/name/dimension/profile after purge and same-name recreation.
Foreign guards reject; missing IDs return None; unknown provenance remains
unknown; capture does not sample clock or advance canonical generation.

Initial37f2604 compile failed solely because the new test omitted GenerationRef
import; no test executable was produced.55bcf43 adds the import; failed log and
source/tree provenance retained. This is not a reproduced runtime defect.

Offline Rust1.94.1,2CPU/4GiB,jobs2,testthreads2; own new private target cleaned of
package outputs before first compile, no source-root switching. No personal
data, credentials, network, sockets or external providers. Log SHA256
`68b6aca3124fe206ba677a1f8a353f1566bf5be51237c6524163351c431a1c3e`; final manifest SHA256 `0e5669fcc51a4da10cb51368c6fee57d9e41c68baaa4bc6270b5cfa9c8c27605`.

The actual replay-name hash/assertion consumer join remains required before
activation. Known committed receipts must survive unavailable/missing binding
errors. No helper-only publication or whole-query readiness claim.
