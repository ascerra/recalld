# Explicit inherited-policy adoption

Implementation candidate on `0d50ee864f83514dba23ffe73e57abea4be29e4f`:
actual143 `0f34cd30e48b59dd655b3285d00f8b99e92c32d1` plus actual135
`24da9206ba141493cdb9ca988188be6d5690a814`. Runtime3e5fd806 passed the sealed validation in ../evidence/inherited-policy-adoption/README.md. This
candidate does not enable ordinary native startup or public API2.

The genuine143 restart probe at audit commit528cf28 observed inherited multiplier
1→0 change projected strength 0.35273787→1 at the same canonical G5 and policy
revision1. The explicit Some(1) control did not change. The new regression uses
the actual loader, owned initializer and false get; it does not replace the
unknown migrated policy with a convenient declared default.

## Operator flow

Stop the process owning the store. Obtain its full canonical generation reference
through offline inspection, then run:

```
recalld-cli policy preview-inherited --store PATH --namespace-id ID \
  --multiplier 1 --expected-store-id UUID --expected-generation G
recalld-cli policy adopt-inherited --store PATH --request request.json
```

The first command is read-only, holding exclusive ownership while validating
canonical files and examining a disposable metadata copy. Its JSON output is the
`preview` value of the second command's request:

```
{
  "context": {
    "storeId": "ORIGINAL-PREVIEW-STORE-UUID",
    "scope": {"kind": "namespace", "namespaceId": 1},
    "idempotencyKey": "operator-chosen-lifetime-key",
    "expectedGeneration": "ORIGINAL-PREVIEW-G",
    "contractVersion": 1
  },
  "preview": "REPLACE THIS STRING WITH THE COMPLETE PREVIEW OBJECT"
}
```

Review the candidate phase counts and whether `prior` is unknown before adopting.
Do not change its clock, source, namespace, counts, digest or token. Apply reads
at most16KiB of request data, rejects malformed/incomplete evidence, rechecks the
complete canonical evidence under the owner's write guard and commits exactly
one receipt/G. JSON contract1 and2 preserve their original hash identity.
Successful output includes the full canonical receipt and `reopen_required`;
it does not certify live derived readiness. Both commands dispatch before HTTP
client construction and do not contact a daemon or embedding provider.

No startup write adopts configuration. A migrated absent binding remains unknown;
raw inspection, export and history reopen remain supported. Dependent operations
must explicitly adopt it or use an explicitly declared namespace override. Once
adopted, a changed effective inherited multiplier rejects until a new preview is
accepted. Two f64 spellings that round to the same finite f32 are equivalent;
negative/nonfinite/overflow/nonzero-underflow values reject. Version1 supports
only the built-in shared DecayConfig. It cannot certify changed custom connection
caps or access weights.

New namespace creation anchors declared present-day inherited inputs in its
actual creation receipt. Some→None numeric replacement clears a retained old
binding, proves that clearance, and leaves dependent operations blocked until
explicit adoption. This prevents a stale prior adoption from springing back.
Some(value) overrides an unused changed global without automatic revision churn.

## Proof and fixture boundaries

Adoption changes policy/record revision tokens only. It preserves timestamps,
anchors, phase, materialized strengths, payloads, provenance, graph and vectors.
The preview uses the existing single-step native phase planner and actual graph
bonus inputs at one clock; a Full row cannot cascade through several phases in
one preview. Its output contains fixed-size counts and hashes, not memory IDs or
payload. Metadata commitments stream in canonical order. Planner limits count
all examined rows/edges, including excluded namespaces and terminal rows; any
exhaustion returns an error with no usable token. Before creating any disposable inspection copies, the command checks the combined
meta.db/edges.db length against512MiB under the same source owner. TempDirs clean up
on every error/rejection. Existing full canonical-file validation still precedes
planning and is not advertised as a bounded-memory replacement validator. Apply
validates replay/CAS/digest on the disposable copy and transfers that same owner
into live open; no unlocked gap or independent authority is introduced.

A required typed adoption outcome anchors the optional namespace binding. Strict
history rewinds all origins' policy revisions and binding changes, including
retired namespace witnesses. Forks keep the original adoption store/G/operation;
foreign counters are not compared with the current store's counter. Removing a
current binding while its receipt remains must fail raw inspection and reopen.
This is protocol validation, not authenticity against rewriting all trust anchors.

Tests must preserve genuine migrated None/unknown history. New dependent fixtures
must call this real explicit preview/adopt flow, or declare an explicit policy via
an actual writer. Raw schema mutation is only appropriate for deliberate corrupt
negative fixtures; it is never a substitute for adoption or compatibility proof.

## Remaining activation composition

This slice's actual143 initializer passes built-in shared DecayConfig plus the
configured multiplier to query projection; the direct native writers validate
those frozen inputs against canonical adoption. It does not imply an override
installed on one adapter reaches another adapter. A4's separate148/153 source
review identifies the single-store producer constructing its own built-in global
inputs with the configured multiplier. Ordinary startup composition must pass the
same accepted inputs through insertion/selection capture and final validation,
and reject any unsupported configured planner inputs rather than silently losing
them. That composition remains separate from this bounded143+135 slice.

Old Bound vector certification is an independent unresolved semantic admission
gate, recorded in native-vector-producer-admission-audit.md. Policy adoption is
not embedding certification. Public legacy-disk API2 catalog/purge participation
and ordinary native/public API2 activation remain separately owned work; this
operator command does not open those gates.
