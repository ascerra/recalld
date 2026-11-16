# Native feedback original-contract validation

Runtime `f128582dc11f05ac0c94e6aa3edea1577de4ec92`, branch
`impl/native-origin-propagation`, direct [PR126](https://github.com/calebevans/recalld/pull/126)
`179edf0d14d383466253a62b892c3741b5d4d9b1`. No compound/catalog/purge parent.

## Results and source boundaries

- Unchanged126 BEFORE writer: real API2 feedback/forget fixture and API1 rejection
  before clock/record changes. Probe passed; complete exact probe source retained.
- Intermediate: original_api3, unsupported1, native_lifecycle_wire9 passed.
- Final baseline: original_api4 + unsupported1 + feedbacknegative1 focused passed;
  full **1140 library +2 CLI passed**, 4 doctests ignored; fmt passed;
  Clippy exit0 with890 library/954 test warnings (865duplicates).
- Post-END final test-only tail: **original_api5 passed**, fmt passed, Clippy exit0
  with890 library/954 test warnings (865duplicates). No1141-test full run claimed.

The final baseline and tail each seal274 source/Cargo/test/toolchain/fixture inputs.
Only `src/storage/metadata/native_records/origin_tests.rs` differs: boolean selector
negatives and the added emulated old-handler socket proof. The runtime is identical
between those validated stages. `tail/baseline-diff.json` records this boundary;
all tail inputs match the runtime commit. No source changed during either run.
Source archives, raw logs (gzip), checks, and executable SHA256/size/mtime_ns are
preserved for every stage. The manifests record three final executable artifacts;
these are artifacts present in the private target, not a claim all three ran in
the focused tail. The library test executable is identified in focused.log.
No discarded failing run or timing performance claim. No executable/cache committed.

## Historical proof and old-handler distinction

BEFORE executable SHA256:
`620aed3c1b48b84128379a3b1a91ef5278ae9a00c2f56a7c67a9b9da7dd0a8f6`.
The32,657-byte embedded fixture SHA256:
`b1205839f62ebc7abb383296a137be1de4ce3dc4c9f14a6a8c0a5ec91a58040b`.
This is genuine synthetic historical writer output, not fabricated receipt rows.
Tests strict-reopen and retain exact raw receipts, replay, export/import, fork
restore, and reject old-lineage retry without sampling time.

The separate old-private-handler test uses actual RemoteStorageAdapter/client
framing plus an emulated pre-change handler running the unchanged strict2 parser.
It rejects explicit API1 for feedback and forget with no mutating fallback/retry.
It is not execution of a historical daemon binary.

## Isolation and reproduction

`recalld-ci:1.94`, toolchain1.94.1, jobs2, offline Cargo, Docker network none,
2CPU/4GiB. Only this assigned worktree and its private cargo/target mounted;
no home, credentials, daemon socket, or personal data. See validation-command.txt,
final/run.sh and tail/run.sh. The final tail reused this same-source private target;
source switched test file was rebuilt. Before/intermediate exact source remains
available in its archive. `seal.py` hashes all source/test/Cargo/toolchain inputs.

## Scope and remaining work

Explicit private numeric origin1|2 reaches native feedback/exposure core context;
strict prepared public2 parser remains strict2. Replay precedes fresh CAS/policy/
clock and cross-origin keys conflict. Unsupported selectors reject before writes.
Known receipts, terminal identity, accepted root and real publication remain.
Curie owns native forget1 record-context admission; Huygens owns public routing.
Exposure119/manual107 origin consumers remain separate independent children.
No ordinary native serving, public API2 activation, keyed-query semantics,
deterministic IDs, or complete A3/A4 claim. See changed-files and implementation docs.
