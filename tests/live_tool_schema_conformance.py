"""Validate the actual tools/list schemas against emitted v1 metadata results.

Offline test-only jsonschema; no production dependency. Rust tests own scope,
count sums, truncation, canonical immutability and negotiated-era behavior.
"""
import copy
import json
from pathlib import Path
import sys
from jsonschema import Draft202012Validator

cases = 0
kinds = set()
for path in sorted(Path(sys.argv[1]).glob("*.json")):
    case = json.loads(path.read_text())
    schema, value, tool = case["schema"], case["value"], case["tool"]
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    validator.validate(value)
    kinds.add(tool)
    cases += 1

    def rejects(change):
        bad = copy.deepcopy(value)
        change(bad)
        assert not validator.is_valid(bad), (path.name, bad)

    rejects(lambda v: v.update(unadvertised=True))
    for field in value:
        rejects(lambda v, f=field: v.pop(f))
    if tool == "namespace_stats":
        for invalid in [-1, 1.5, True, "1", None, 18446744073709551616]:
            rejects(lambda v, x=invalid: v.update(memoryCount=x))
        rejects(lambda v: v["phaseCounts"].update(tombstone=1))
        rejects(lambda v: v.update(avgStrength="1"))
    else:
        for bucket in ["tags", "entities", "topics", "emotions"]:
            rejects(lambda v, b=bucket: v[b].update(truncated="false"))
            rejects(lambda v, b=bucket: v[b].update(total=-1))
            rejects(lambda v, b=bucket: v[b].update(extra=0))
            if value[bucket]["items"]:
                rejects(lambda v, b=bucket: v[b]["items"][0].update(count="1"))
                rejects(lambda v, b=bucket: v[b]["items"][0].update(extra=0))
                cap = schema["properties"][bucket]["properties"]["items"]["maxItems"]
                rejects(lambda v, b=bucket, n=cap: v[b].update(items=[v[b]["items"][0]] * (n+1)))
        rejects(lambda v: v.update(namespace=1))
        rejects(lambda v: v.update(memoriesCounted="1"))

assert cases >= 13 and kinds == {"namespace_stats", "list_tags"}, (cases, kinds)
print(f"PASS {cases} actual live metadata outputs and strict schema mutations")
