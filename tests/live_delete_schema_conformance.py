"""Actual delete emitters; no canonical proof/authenticity validation claim."""
import copy
import json
import pathlib
import sys
from jsonschema import Draft202012Validator, FormatChecker

cases = 0
codes = set()
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.json")):
    fixture = json.loads(path.read_text())
    assert fixture["tool"] == "delete_namespace"
    schema, value = fixture["schema"], fixture["value"]
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    assert validator.is_valid(value), (path.name, [e.message[:800] for e in validator.iter_errors(value)])
    assert fixture["fixtureProvenance"]

    def reject(v):
        assert not validator.is_valid(v), (path.name, "accepted malformed shape")

    reject({**value, "ready": True})
    changed = copy.deepcopy(value)
    del changed["error" if "error" in value else "name"]
    reject(changed)
    if "error" in value:
        codes.add(value["error"])
        reject({**value, "commitState": "rolledBack"})
        if value["error"] == "COMMIT_OUTCOME_UNKNOWN":
            changed = copy.deepcopy(value)
            del changed["operationId"]
            reject(changed)
            reject({**value, "operationId": 1})
        else:
            reject({**value, "operationId": None})
        if "receipt" in value:
            changed = copy.deepcopy(value)
            changed["receipt"]["committedGeneration"] = "1\n"
            reject(changed)
            changed = copy.deepcopy(value)
            changed["receipt"]["outcome"]["unexpected"] = True
            reject(changed)
            changed = copy.deepcopy(value)
            del changed["receipt"]
            reject(changed)
            wrong_scope = "purgeStep" if value["commitEvidenceScope"] == "purgeComplete" else "purgeComplete"
            reject({**value, "commitEvidenceScope": wrong_scope})
    else:
        reject({**value, "memoriesDeleted": -1})
        reject({**value, "vectorFileRemoved": "true"})
    cases += 1
assert codes == {"PURGE_METRICS_UNAVAILABLE", "SERVICE_UNAVAILABLE", "COMMIT_OUTCOME_UNKNOWN"}, codes
assert cases
print(f"PASS {cases} captured delete output cases, all three actual failure alternatives and meaningful malformed-shape negatives")
