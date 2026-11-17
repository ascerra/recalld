"""Validate actual record emitters; schemas describe shape, not receipt proof."""
import copy
import json
import pathlib
import sys
from jsonschema import Draft202012Validator, FormatChecker

cases = 0
tools = set()
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.json")):
    fixture = json.loads(path.read_text())
    schema, value = fixture["schema"], fixture["value"]
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = list(validator.iter_errors(value))
    assert not errors, (path.name, [(list(e.path), e.message[:1600]) for e in errors])
    assert fixture["fixtureProvenance"]

    def reject(v):
        assert not validator.is_valid(v), (path.name, "accepted malformed shape")

    reject({**value, "unexpected": True})
    # One meaningful required member per actual emitted shape.
    required = "results" if "results" in value else "error" if "error" in value else "id"
    changed = copy.deepcopy(value)
    del changed[required]
    reject(changed)
    if "commitState" in value:
        reject({**value, "commitState": "rolledBack"})
    for name in ["receipt", "reservationReceipt"]:
        if value.get(name):
            for field, invalid in [("receiptVersion", "1"), ("committedGeneration", "1\n"), ("storeId", 1)]:
                changed = copy.deepcopy(value)
                changed[name][field] = invalid
                reject(changed)
            changed = copy.deepcopy(value)
            changed[name]["outcome"]["kind"] = "futureUnsupportedOutcome"
            reject(changed)
            changed = copy.deepcopy(value)
            changed[name]["outcome"]["unexpected"] = True
            reject(changed)
    evidence = value.get("receivedEvidence", {})
    if "receiptsJson" in evidence:
        changed = copy.deepcopy(value)
        changed["receivedEvidence"]["receiptsJson"] = [{}]
        reject(changed)
    if "receipt" in evidence:
        changed = copy.deepcopy(value)
        del changed["receivedEvidence"]["receipt"]["operationId"]
        reject(changed)
        changed = copy.deepcopy(value)
        changed["receivedEvidence"]["receipt"]["outcome"]["opaqueVariantMember"] = {"unverified": True}
        assert validator.is_valid(changed), "unverified payload must remain explicitly opaque"
    cases += 1
    tools.add(fixture["tool"])
assert tools == {"get_memory", "store_memory", "store_memories", "forget_memories"}, tools
print(f"PASS {cases} actual/categorized record output cases across {len(tools)} tools and meaningful malformed-shape mutations")
