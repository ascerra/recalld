
### 2026-11-15: heterogeneous native vector owner, actual BEFORE and focused GREEN

Question: can the existing canonical publisher retain namespaces with differing valid vector widths without changing semantic admission? Worktree `/tmp/recalld-implementation/heterogeneous-vector-owner`, actual PR178 base `3361ddb2309593493326ace621d4fdcc9ba89d8c`.

- BEFORE001 `3f7bface`: test provider Result alias compile failure, no executed test; retained separately.
- BEFORE002 `e7674702`: real insertion + schema migration + three explicit reembeddings produced five namespaces/five records at G15. Two intended failures in 3.23s: private startup native_profile rejection; independent publisher expected4/got5, retaining the prior committed G3 insertion receipt, not inventing a new G15 commit. Canonical content/G unchanged and all four view stamps None. Binary SHA256 `184c2803ccfe343e466e00321399a8cd9b276deffd43044ed40164e6cc4465c7`.
- Candidate003 `3f6f66ef`: two owner tests and three actual heterogeneous tests passed (actual tests5.69s), including publisher exact bits/G15/four views, real purge/rebuild and corrupt-source immutable rejection. Private profile loop remained a deliberate rejection.
- Candidate004 `30e9d58f`: missing Flat VectorIndex trait import after cleanup, compile-only failure; no full result.
- Candidate005 `3ec4ff51289e23bf217afbd1efa40a067d74e4fd`: import restored plus typed zero-cap legacy-autolink rejection; three owner + three actual tests passed (actual5.13s). Full default all-target suite and all-feature lint still running at this entry; no final full claim.

Rust1.94.1, offline Docker2CPU/4GiB/jobs2, one existing private target, read-only source/registry, no credentials/socket/personal-data mounts. New shared Rust handle is VectorIndexOwner; Flat kernel/trait/CHVI remain unchanged. Main approved implementation after actual RED and Hume/Huygens/Curie hunk ACKs. No second registry/G/backend, no constructor loop removal or activation. Repo evidence: `docs/evidence/heterogeneous-vector-owner/`; exact hunk/API scope: `docs/implementation/heterogeneous-owner-hunks.md` and `heterogeneous-vector-owner.md`.
