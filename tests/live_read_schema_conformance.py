"""Formal checks against actual live tools/list and emitted read outputs.

Rust tests label actual legacy readers separately from authored typed DTOs passed
through real serializers. Neither class claims native writer/reopen acceptance.
"""
import copy
import json
from pathlib import Path
import sys

from jsonschema import Draft202012Validator

cases = 0
kinds = set()
coverage_modes = set()
provenances = set()
empty_overlap = False
graph_context = False
for path in sorted(Path(sys.argv[1]).glob("*.json")):
    case = json.loads(path.read_text())
    schema, value, tool = case["schema"], case["value"], case["tool"]
    assert case["fixtureProvenance"]
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
    roots = ["memories", "count"]
    if tool == "list_memories":
        roots = ["memories", "total", "offset", "limit"]
    elif "coverage" in value:
        roots = ["namespace", "threshold", "clusters", "clusterCount", "coverage"]
    elif tool == "find_similar_memories":
        roots.append("sourceId")
    for field in roots:
        rejects(lambda v, f=field: v.pop(f))
        rejects(lambda v, f=field: v.update({f: None}))
    for field in [f for f in roots if f in ("count", "total", "offset", "limit", "clusterCount")]:
        for invalid in [-1, 1.5, True, "1", None, 18446744073709551616]:
            rejects(lambda v, f=field, x=invalid: v.update({f: x}))

    if tool == "list_memories":
        cap = schema["properties"]["memories"]["maxItems"]
        rejects(lambda v: v.update(limit=0))
        rejects(lambda v: v.update(limit=cap + 1))
        if value["memories"]:
            rejects(lambda v: v.update(memories=[v["memories"][0]] * (cap + 1)))

    for idx, row in enumerate(value.get("memories", [])):
        for key in ["id", "summary"] + (["createdAt"] if tool == "list_memories" else []):
            rejects(lambda v, i=idx, k=key: v["memories"][i].pop(k))
            rejects(lambda v, i=idx, k=key: v["memories"][i].update({k: None}))
        rejects(lambda v, i=idx: v["memories"][i].update(unadvertised=0))
        for key in ("entities", "topics", "emotions", "tags", "related", "labels", "fullText"):
            if key in row:
                rejects(lambda v, i=idx, k=key: v["memories"][i].update({k: None}))
        if "score" in row:
            rejects(lambda v, i=idx: v["memories"][i].update(score="1"))
            rejects(lambda v, i=idx: v["memories"][i].pop("score"))
        if row.get("related"):
            rejects(lambda v, i=idx: v["memories"][i]["related"][0].update(weight="1"))
            rejects(lambda v, i=idx: v["memories"][i]["related"][0].pop("edgeType"))
            rejects(lambda v, i=idx: v["memories"][i]["related"][0].update(extra=0))

    def check_labels(getter):
        labels = getter(value)
        provenances.add(labels["provenance"])
        for key in ["entities", "topics", "emotions", "provenance"]:
            rejects(lambda v, k=key: getter(v).pop(k))
            rejects(lambda v, k=key: getter(v).update({k: None}))
        rejects(lambda v: getter(v).update(provenance="nativeV2"))
        rejects(lambda v: getter(v).update(extra=0))
        for key in ["entities", "topics", "emotions"]:
            rejects(lambda v, k=key: getter(v).update({k: [42]}))

    for idx, row in enumerate(value.get("memories", [])):
        if "labels" in row:
            check_labels(lambda v, i=idx: v["memories"][i]["labels"])
    if "graphContext" in value:
        graph_context = True
        for key in ["neighbors", "neighborCount"]:
            rejects(lambda v, k=key: v["graphContext"].pop(k))
        rejects(lambda v: v["graphContext"].update(extra=0))
        rejects(lambda v: v["graphContext"].update(neighborCount=-1))
        for idx, row in enumerate(value["graphContext"]["neighbors"]):
            for key in ["id", "summary", "edgeType", "connectedTo", "weight"]:
                rejects(lambda v, i=idx, k=key: v["graphContext"]["neighbors"][i].pop(k))
            rejects(lambda v, i=idx: v["graphContext"]["neighbors"][i].update(entities=[]))
            if "labels" in row:
                check_labels(lambda v, i=idx: v["graphContext"]["neighbors"][i]["labels"])
    if tool == "recall_memories" and value == {"memories": [], "count": 0}:
        # Both actual emitters produce this value; oneOf would be incorrect.
        assert all(Draft202012Validator(s).is_valid(value) for s in schema["anyOf"])
        empty_overlap = True

    if "coverage" in value:
        cov = value["coverage"]
        coverage_modes.add(cov["countsAvailable"])
        for key in cov:
            rejects(lambda v, k=key: v["coverage"].pop(k))
        rejects(lambda v: v["coverage"].update(extra=0))
        rejects(lambda v: v["coverage"].update(complete=True))
        rejects(lambda v: v["coverage"].update(algorithm="exact"))
        rejects(lambda v: v["coverage"].update(sampleCap=201))
        rejects(lambda v: v["coverage"].update(neighborCap=10))
        rejects(lambda v: v["coverage"].update(countsAvailable=not cov["countsAvailable"]))
        for key in ["eligibleCount", "sampledCount", "skippedVectorCount"]:
            invalid = [None, -1, 0.5, True, "1", 18446744073709551616] if cov["countsAvailable"] else [0, "0", False]
            for bad in invalid:
                rejects(lambda v, k=key, x=bad: v["coverage"].update({k: x}))
        rejects(lambda v: v["coverage"].update(truncated=None if cov["countsAvailable"] else False))
        for invalid in [-0.1, 1.1, None, "0.85", True]:
            rejects(lambda v, x=invalid: v.update(threshold=x))
        for idx, cluster in enumerate(value["clusters"]):
            rejects(lambda v, i=idx: v["clusters"][i].pop("maxSimilarity"))
            rejects(lambda v, i=idx: v["clusters"][i].update(maxSimilarity="1"))
            rejects(lambda v, i=idx: v["clusters"][i].update(extra=0))
            for j in range(len(cluster["memories"])):
                rejects(lambda v, i=idx, n=j: v["clusters"][i]["memories"][n].pop("id"))
                rejects(lambda v, i=idx, n=j: v["clusters"][i]["memories"][n].update(similarity=1))

assert cases >= 47, cases
assert kinds == {"list_memories", "recall_memories", "find_similar_memories"}, kinds
assert provenances == {"native_v2", "legacy_retained_only"}, provenances
assert coverage_modes == {True, False}, coverage_modes
assert empty_overlap and graph_context
print(f"PASS {cases} live read outputs/specimens and strict schema mutations")
