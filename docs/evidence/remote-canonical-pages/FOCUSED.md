# R2 focused validation and schema correction

Run003 source `fd2e977aedc728d8fcd502e6dedb3ed61c2620d9`:20 distinct tests passed
(7 prepared Remote,7 catalog,6 canonical list). All three original malformed
count frames now reject. Actual native Unicode/empty/retained-label/configured
profile Remote and direct MCP/HTTP/reopen controls passed. All466 input hashes
matched. Executable SHA256
`e5c8fe3aad92b638e464085a53dfa531d69c797212c74a06164186985d3c3250`.
Cgroup peak4294967296bytes,max22988,oom0,oom_kill0. Container retired.

Run004 source `6ae99ee0f99c8ff15f3a2a850670ec65dc5e19d9`:23 distinct tests passed
(8 prepared Remote,9 catalog,6 canonical list), formatting exit0. Added controls
cover explicitly authored already-open legacy metadata width8193 inventory,
unchanged actual new-creation rejection, authored Bound-width peer rejection,
carried Unknown context, and actual MCP/HTTP catalog errors for both kinds.
This does not prove a genuine historical writer accepted width8193, valid native
publication at that width, or an ordinary reopen of the authored reader fixture.

All468 source input hashes matched. Executable SHA256
`1e5b1f294d94a5f2880cee66357e7217d4a0667fb5fad6907b135aba22edf7c1`
was copied and reverified before replacing package artifacts. Cgroup peak4GiB,
max22944,oom0,oom_kill0. These runs had reclaim pressure, not OOM.

The separate offline formal runner FAILED on its22 actual captured envelopes:
schema category `notFound` did not accept actual `not_found`. Inspection of
`contract.rs::ErrorCategory` confirms `rename_all="snake_case"`; `invalidInput`
was likewise incorrect in the new schema. This is a schema-definition bug, not
a production serializer/transport bug. No tolerance, numeric normalization or
runtime wire change is warranted. Correction uses serialization of the actual
typed ErrorCategory variants instead of duplicate string spellings.

All22 fixture hashes and the runner hash matched before/after the failed schema
run. Source, fixtures and pinned validator packages were readonly mounts; schema
output used a separate writable directory. Rust and schema containers retired.
Protected roots:
`/tmp/recalld-huygens-active-validation/canonical-pages-003-focus` and
`/tmp/recalld-huygens-active-validation/canonical-pages-004-tail`.
The failed formal log/source/executable remain retained; no raw deletion.

Measured escaped-field legacy-store page after a smaller-limit retry:
REST201557bytes; modern MCP456169/daemon456062; legacy MCP254591/daemon254484.
These are actual fixture measurements including complete wrappers with maximum
numeric IDs, not an all-history maximum, RSS limit or arbitrary-string-ID bound.

No full suite, Clippy, final schema GREEN or READY claim is made by these runs.
The schema-only correction needs affected emitted fixtures and formal validation
before the planned single frozen full run. Original001 and002 failures stay
separately attributed to their exact sources and executables.
