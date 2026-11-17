"""Validate fixtures emitted by strict_query_ tests with JSON Schema 2020-12.

Run offline with pinned test-only jsonschema 4.25.1; no production dependency.
These schemas describe wire shape. Storage validates receipt transitions and
bindings; the parser additionally enforces UTF-8 byte limits, vector norm,
time ordering and lexical integer tokens (JSON Schema integers are numeric).
"""
import copy
import json
from pathlib import Path
import sys

from jsonschema import Draft202012Validator, FormatChecker

root = Path(sys.argv[1])
checked = 0
for path in sorted(root.glob("*.json")):
    fixture = json.loads(path.read_text())
    cases = fixture if isinstance(fixture, list) else [fixture]
    for case in cases:
        schema, value = case["schema"], case["value"]
        Draft202012Validator.check_schema(schema)
        validator = Draft202012Validator(schema, format_checker=FormatChecker())
        errors = list(validator.iter_errors(value))
        expected = case.get("valid", True)
        assert (not errors) == expected, (path.name, value, [str(e) for e in errors])
        checked += 1
        if "valid" in case:
            continue
        # Output schemas must reject accidental extra fields and numeric G.
        bad = copy.deepcopy(value)
        bad["undeclared"] = True
        assert not validator.is_valid(bad), path.name
        if "readGeneration" in value:
            for generation in [1, "01", "-1", "9223372036854775808", "1\n", "1\r", "1\r\n", "1\u2028", "1\u2029"]:
                bad = copy.deepcopy(value)
                bad["readGeneration"]["generation"] = generation
                assert not validator.is_valid(bad), (path.name, generation)
            for generation in ["0", "9223372036854775807"]:
                good = copy.deepcopy(value)
                good["readGeneration"]["generation"] = generation
                validator.validate(good)
        if "similar" in path.name and value.get("data", {}).get("results"):
            for field in ["rankingProfile", "explanation"]:
                bad = copy.deepcopy(value)
                bad["data"]["results"][0][field] = "baseline-v1"
                assert not validator.is_valid(bad), (path.name, field)
        # Actual evidence is strictly typed at every nested boundary. These
        # shape checks do not replace the producer's score/count invariants.
        data = value.get("data") or {}
        for index, row in enumerate(data.get("results", [])):
            explanation = row.get("explanation")
            if not explanation:
                continue
            bad = copy.deepcopy(value)
            bad["data"]["results"][index]["explanation"]["undeclared"] = True
            assert not validator.is_valid(bad), (path.name, "nested explanation")
            if explanation.get("algorithm") == "cosineMax":
                for field, invalid in [
                    ("algorithm", "baseline-v1"), ("ownAdmission", "unknown"),
                    ("ownCosine", "0.5"), ("minScore", 2),
                    ("omittedOrigins", -1), ("finalSimilarity", None),
                    ("origins", [dict(originId="bad", cosine=0.5)]),
                    ("origins", [dict(originId=row["memoryId"], cosine=0.5)] * 65),
                ]:
                    bad = copy.deepcopy(value)
                    bad["data"]["results"][index]["explanation"][field] = invalid
                    assert not validator.is_valid(bad), (path.name, field)
            elif explanation.get("own"):
                for multiplier in [None, "1", 0.9]:
                    bad = copy.deepcopy(value)
                    bad["data"]["results"][index]["explanation"]["own"]["rifMultiplier"] = multiplier
                    assert not validator.is_valid(bad), (path.name, "prepared Off multiplier")
        coverage = data.get("coverage", {})
        for name, channel in coverage.get("channels", {}).items():
            for field, invalid in [("examinedUnit", "records"), ("examined", -1), ("eligible", "0")]:
                bad = copy.deepcopy(value)
                bad["data"]["coverage"]["channels"][name][field] = invalid
                assert not validator.is_valid(bad), (path.name, name, field)
        if coverage.get("canonicalEligibility"):
            for field, invalid in [("examinedUnit", "vectorEntries"), ("passed", -1), ("firstRejections", {"other": 1}), ("firstRejections", {"requiredTag": "1"})]:
                bad = copy.deepcopy(value)
                bad["data"]["coverage"]["canonicalEligibility"][field] = invalid
                assert not validator.is_valid(bad), (path.name, field)
        if "error" in value:
            bad = copy.deepcopy(value)
            del bad["error"]["commitState"]
            assert not validator.is_valid(bad), path.name
        if "receivedEvidence" in value:
            evidence = value["receivedEvidence"]["receipts"][0]
            headers = {"storeId", "operationId", "committedGeneration", "receiptVersion", "outcome", "replayed"}
            assert set(evidence) == headers
            for field in headers:
                bad = copy.deepcopy(value)
                del bad["receivedEvidence"]["receipts"][0][field]
                assert not validator.is_valid(bad), (path.name, "missing evidence header", field)
            for field, invalid in [("storeId", 1), ("operationId", 1), ("committedGeneration", 1),
                                   ("receiptVersion", 2), ("replayed", 1), ("extra", True),
                                   ("outcome", []), ("outcome", {}), ("outcome", {"kind": 1})]:
                bad = copy.deepcopy(value)
                bad["receivedEvidence"]["receipts"][0][field] = invalid
                assert not validator.is_valid(bad), (path.name, "evidence header", field)
            bad = copy.deepcopy(value)
            bad["error"]["commitState"] = "committed"
            assert not validator.is_valid(bad), (path.name, "unverified evidence is not commitment")
            bad = copy.deepcopy(value)
            bad["warnings"] = []
            assert not validator.is_valid(bad), (path.name, "unverified warning required")
            # Deliberately opaque in this HEADER schema. The separate Rust peer
            # regression rejects this unknown kind before evidence acquisition.
            opaque = copy.deepcopy(value)
            opaque["receivedEvidence"]["receipts"][0]["outcome"] = {"kind": "futureUnknownKind", "opaque": [1, None]}
            validator.validate(opaque)
assert checked >= 40, f"missing actual fixtures: {checked}"
print(f"PASS {checked} actual output/input cases plus output mutation negatives")
