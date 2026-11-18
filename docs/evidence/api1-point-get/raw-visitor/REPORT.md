# Borrowed API1 receipt admission and owning timezone diagnostic

Source `90b086b4` (full hash in SOURCE.json), executable SHA256 `5044eeab167f2100eb4bb5caf6add3ca7321e130d285e5990a4595a475faaf39`: seven focused diagnostics pass, zero fail/ignored, 0.53s. Runtime remains2925; this is test-only evidence, not the ordinary consumer implementation/full proof.

All five earlier raw-carrier checks pass again. The new borrowed sequence visitor rejects before retaining candidate129 or overflowing its checked byte sum. Input duplicates count before dedup;127/128 admitted duplicate candidates reduce to one raw fragment,129 rejects. It never reserves from a peer size hint. Existing structural duplicate/depth/node/all-number admission and strict receipt validation remain separate checks.

Enumerating every supported chrono_tz variant finds a maximum serialized timezone name of34 bytes (`America/Argentina/ComodRivadavia`). Every proposed fixed native tag, with actual owner timezone and empty replyJson, fits256 bytes. This bounds this pinned enum/tag, not arbitrary peer strings or public JSON-RPC IDs. Actual producer counters still must run before exposure and use the owner's timezone.

Run023 on4554bf7d failed compilation because the new timezone diagnostic used an unqualified json macro. Zero tests ran there. The one-line qualification is the entire delta to90b; original diagnostic/source/container retained under compile-before. No prior failure was relabeled as green. Build024 completed in1m21s, source/executable reverified after test. Captured actual container IDs/limits: networknone,2CPU/4GiB,jobs1/tests2, one existing cache and read-only source, no personal data. Seven checks are not a full suite.

After the earlier022 seal, A1/Huygens source-ACKed e5df4fd5. Halley formally released only the listed ordinary-get runtime hunks on the actual2925 child; frozen parent branches and D1 schemas/session/description remain excluded. Legacy Ghost observation/payload behavior must stay distinct from strict native O1; new capable failures retain full typed receipts and unknown tickets before Display conversion. Next work is that real consumer and its RPC/fault/full validation, not another O1 rerun.

Cleanup: containers completed with --rm; zero host bytes released. Central cleanup alone owns deletion. Active `/tmp/recalld-curie-o1-build/{cargo,target,runs}` remains protected, including023/024; no new cache clone/HOME archive. Compact results are recorded here.
