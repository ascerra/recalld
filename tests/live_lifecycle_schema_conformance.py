"""Validate live API1 lifecycle output shapes, not storage-proof semantics."""
import copy
import json
import pathlib
import sys
from jsonschema import Draft202012Validator, FormatChecker

cases = 0
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.json")):
    fixture = json.loads(path.read_text())
    schema, value = fixture["schema"], fixture["value"]
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = list(validator.iter_errors(value))
    assert not errors, (path.name, [(list(e.path), e.message) for e in errors])
    assert fixture["fixtureProvenance"]
    def reject(v):
        assert not validator.is_valid(v), (path.name, "accepted mutation", v)
    changed = copy.deepcopy(value)
    changed["unexpected"] = True
    reject(changed)
    if "error" in value:
        for name in ["scope", "data", "receipt", "error", "readiness"]:
            changed = copy.deepcopy(value)
            del changed[name]
            reject(changed)
        changed = copy.deepcopy(value)
        changed["error"]["unexpected"] = True
        reject(changed)
        changed = copy.deepcopy(value)
        changed["error"]["commitState"] = "rolledBack"
        reject(changed)
        for scope in [{"namespaceId": 1, "namespace": "native"}, [1], [True, "native"], [1, "native", 2]]:
            changed = copy.deepcopy(value)
            changed["scope"] = scope
            reject(changed)
        if value.get("receipt"):
            for name in ["storeId", "operationId", "committedGeneration", "receiptVersion", "outcome", "replayed"]:
                changed = copy.deepcopy(value)
                del changed["receipt"][name]
                reject(changed)
            for counter in [1, True, "01", "-1", "1\n", "1\r", "1\u2028", "9223372036854775808"]:
                changed = copy.deepcopy(value)
                changed["receipt"]["committedGeneration"] = counter
                reject(changed)
            changed = copy.deepcopy(value)
            changed["receipt"]["outcome"]["unexpected"] = True
            reject(changed)
            transition = value["receipt"]["outcome"].get("transition")
            if transition:
                changed = copy.deepcopy(value)
                changed["receipt"]["outcome"]["transition"]["unexpected"] = True
                reject(changed)
                records = transition["records"]
                if records:
                    changed = copy.deepcopy(value)
                    record = changed["receipt"]["outcome"]["transition"]["records"][0]
                    record["unexpected"] = True
                    reject(changed)
                if fixture["tool"] == "forget_memory":
                    for invalid in [None, "future_rule"]:
                        changed = copy.deepcopy(value)
                        changed["receipt"]["outcome"]["transition"]["phaseTimeRule"] = invalid
                        reject(changed)
                    changed = copy.deepcopy(value)
                    changed["receipt"]["outcome"]["transition"].pop("phaseTimeRule", None)
                    assert validator.is_valid(changed), path.name
                else:
                    changed = copy.deepcopy(value)
                    changed["receipt"]["outcome"]["transition"]["records"][0]["before"]["stability"] = 1e100
                    reject(changed)
            # Matching typed malformed receipts are intentionally retained.
            # Extra edges/records and finite out-of-domain values in these fixtures
            # must not be rejected by canonical producer-only assumptions.
    elif fixture["tool"] == "forget_memory":
        for invalid in [None, 0, "true"]:
            reject({**value, "deleted": invalid})
        changed = copy.deepcopy(value)
        del changed["id"]
        reject(changed)
    else:
        for name in ["id", "strength", "stability", "phase", "isPermastore"]:
            changed = copy.deepcopy(value)
            del changed[name]
            reject(changed)
        reject({**value, "stability": None})
        reject({**value, "strength": 1e100})
    cases += 1
assert cases, "no emitted fixtures"
print(f"PASS {cases} lifecycle output cases and strict codec-shape mutations")
