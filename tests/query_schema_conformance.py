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
        if "similar" in path.name and "data" in value:
            for field in ["rankingProfile", "explanation"]:
                bad = copy.deepcopy(value)
                bad["data"]["results"][0][field] = "baseline-v1"
                assert not validator.is_valid(bad), (path.name, field)
        if "error" in value:
            bad = copy.deepcopy(value)
            del bad["error"]["commitState"]
            assert not validator.is_valid(bad), path.name
assert checked >= 40, f"missing actual fixtures: {checked}"
print(f"PASS {checked} actual output/input cases plus output mutation negatives")
