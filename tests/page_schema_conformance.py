"""Offline JSON Schema 2020-12 check of actual prepared page envelopes.

Shape proof only. Rust/backend tests own exact totals, byte bounds, source
provenance, cursor/scope binding and generation authority. Use the existing
pinned test-only jsonschema installation; no production dependency.
"""
import copy
import json
from pathlib import Path
import sys
from jsonschema import Draft202012Validator, FormatChecker

root = Path(sys.argv[1])
count = 0
kinds = set()
errors_seen = set()
successes_seen = set()
for path in sorted(root.glob("*.json")):
    case = json.loads(path.read_text())
    schema, value = case["schema"], case["value"]
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    validator.validate(value)
    count += 1
    kind = path.name.split("-", 1)[0]
    kinds.add(kind)

    def rejects(change):
        bad = copy.deepcopy(value)
        change(bad)
        assert not validator.is_valid(bad), (path.name, bad)

    rejects(lambda v: v.update(undeclared=True))
    rejects(lambda v: v.update(receipt={}))
    rejects(lambda v: v.update(readiness={"ready": True}))
    if value["status"] == "error":
        errors_seen.add(kind)
        rejects(lambda v: v["error"].update(commitState="committed"))
        rejects(lambda v: v.update(data={}))
        continue
    successes_seen.add(kind)
    for invalid in [1, "01", "-1", "9223372036854775808", "1\n", "1\r", "1\u2028", "1\u2029"]:
        rejects(lambda v, x=invalid: v["readGeneration"].update(generation=x))
    rejects(lambda v: v["data"].update(undeclared=True))
    rejects(lambda v: v["data"].update(nextCursor="bad="))
    if value["data"]["complete"]:
        rejects(lambda v: v["data"].update(nextCursor="abc"))
    else:
        rejects(lambda v: v["data"].update(nextCursor=None))
    data = value["data"]
    if "total" in data:
        rejects(lambda v: v["data"].update(total=1))
    for field in ["memories", "namespaces"]:
        if data.get(field):
            rejects(lambda v, key=field: v["data"][key][0].update(undeclared=True))
    for bucket in ["tags", "entities", "topics", "emotions"]:
        if bucket in data:
            rejects(lambda v, key=bucket: v["data"][key].update(total=1))
            if data[bucket]["items"]:
                rejects(lambda v, key=bucket: v["data"][key]["items"][0]["sources"].update(rawTags=1))

assert kinds == {"list", "namespaces", "tags"}, kinds
assert errors_seen == kinds, ("missing actual per-kind error envelopes", errors_seen)
assert successes_seen == kinds, ("missing actual per-kind successes", successes_seen)
print(f"PASS {count} actual page envelopes plus strict shape/counter mutation negatives")
