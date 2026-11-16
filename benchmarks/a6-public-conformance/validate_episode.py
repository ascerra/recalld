#!/usr/bin/env python3
"""Actual same-binary conformance canary/rejection runs; no builds or mock transport."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    args.out.mkdir()
    source, inputs, baseline = args.root / "source", args.root / "output/inputs", args.root / "output/episode-0"
    binary, scorer = args.root / "target/debug/recalld", args.root / "target/debug/examples/score_mcp_episode"
    manifest = json.loads((inputs / "manifest.json").read_text())
    def run(root, output):
        return subprocess.run([sys.executable, str(source / "scripts/public-mcp-evaluation.py"), "run", "--manifest", str(root / "manifest.json"),
            "--binary", str(binary), "--scorer", str(scorer), "--source-root", str(source), "--out", str(output)], capture_output=True, text=True)
    report = dict(status="running", sourceCommit=manifest["sourceCommit"], binarySha256=sha(binary), scorerSha256=sha(scorer), checkerSha256=sha(Path(__file__)), rejections=[])
    try:
        cases = [("version-" + field, lambda m, field=field: m.update({field: 2})) for field in ("manifestVersion", "sourceMappingVersion", "traceSchemaVersion")]
        cases += [("unavailable-" + field, lambda m, field=field: m["capabilities"].update({field: True})) for field in
                  ("reset", "barrier", "fullA6", "publicV2", "clockAdvance", "deterministicRng", "coherentReadView")]
        cases += [("old-track-new-fixture", lambda m: m.update(track="serialized-mcp-synthetic-diagnostic-v1")),
                  ("native-schema", lambda m: m["versions"].update(canonicalSchemaVersion=2)),
                  ("forged-source", lambda m: m.update(sourceCommit="0" * 40)),
                  ("forged-executable", lambda m: m["executables"].update(recalld="0" * 64))]
        for name, change in cases:
            root = args.out / ("reject-" + name)
            shutil.copytree(inputs, root)
            modified = copy.deepcopy(manifest); change(modified); save(root / "manifest.json", modified)
            output = root / "must-not-exist"
            result = run(root, output)
            report["rejections"].append(dict(case=name, exitCode=result.returncode, outputCreated=output.exists(), stdout=result.stdout, stderr=result.stderr))
            save(args.out / "report.json", report)
            assert result.returncode and not output.exists(), name
        root = args.out / "label-perturbation-inputs"; shutil.copytree(inputs, root)
        labels = json.loads((root / "evaluator-labels.json").read_text())
        schedule = json.loads((root / "evaluator-schedule.json").read_text())
        labels["labelCanary"] = "CONFORMANCE_CHANGED_EVALUATOR_LABEL"
        labels["relevant"] = {q["queryId"]: ["turn-2"] for q in schedule["queries"]}
        save(root / "evaluator-labels.json", labels)
        modified = copy.deepcopy(manifest)
        modified["evaluatorLabels"]["sha256"] = sha(root / "evaluator-labels.json")
        save(root / "manifest.json", modified)
        variant = args.out / "label-perturbation-episode"
        result = run(root, variant)
        save(args.out / "label-run.json", dict(exitCode=result.returncode, stdout=result.stdout, stderr=result.stderr))
        assert result.returncode == 0, result.stderr
        assert (baseline / "actor-code.py").read_bytes() == (variant / "actor-code.py").read_bytes()
        assert (inputs / "evaluator-cache.json").read_bytes() == (root / "evaluator-cache.json").read_bytes()
        def normalized(value, directory):
            mappings = json.loads((directory / "source-mappings.json").read_text())["mappings"]
            # IDs differ between fresh stores. Substitute only exact acknowledged public IDs.
            def visit(v):
                if isinstance(v, dict): return {k: visit(x) for k,x in v.items()}
                if isinstance(v, list): return [visit(x) for x in v]
                return {"acceptedSourceIds": mappings[v]} if isinstance(v, str) and v in mappings else v
            return visit(value)
        old, new = rows(baseline / "actor-wire.jsonl"), rows(variant / "actor-wire.jsonl")
        requests = lambda wire: [r["request"] for r in wire if r["event"] == "attempt"]
        assert normalized(requests(old), baseline) == normalized(requests(new), variant)
        projections = sorted((baseline / "actor-inputs").glob("*.json"))
        assert len(projections) == len(list((variant / "actor-inputs").glob("*.json")))
        for path in projections:
            assert normalized(json.loads(path.read_text()), baseline) == normalized(json.loads((variant / "actor-inputs" / path.name).read_text()), variant)
        old_provider, new_provider = rows(baseline / "actor-provider.jsonl"), rows(variant / "actor-provider.jsonl")
        assert [(r["requestBody"],r["status"]) for r in old_provider] == [(r["requestBody"],r["status"]) for r in new_provider]
        assert all(a["cacheKeys"] != b["cacheKeys"] for a,b in zip(old_provider,new_provider))
        old_scores, new_scores = [json.loads((d / "evaluator/scores.json").read_text()) for d in (baseline, variant)]
        assert old_scores["scoring"] != new_scores["scoring"]
        assert [q["retrievedSourceIds"] for q in old_scores["trace"]] == [q["retrievedSourceIds"] for q in new_scores["trace"]]
        for directory, label in ((baseline, "EVALUATOR_LABEL_CANARY_ONLY"), (variant, labels["labelCanary"])):
            proof = json.loads((directory / "evaluator/conformance.json").read_text())
            assert proof["status"] == "passed" and len(proof["invalidActions"]) == 11
            assert json.loads((directory / "result.json").read_text())["futureQueryCanaryWithheldUntilDelivery"]
            for path in [directory / name for name in ("actor-code.py", "actor-wire.jsonl", "actor-provider.jsonl", "trace.jsonl")] + list((directory / "actor-inputs").glob("*.json")):
                assert label not in path.read_text(), str(path)
        report.update(status="passed", actorRequestsCompared=len(requests(old)), deliveredProjectionsCompared=len(projections),
                      providerRequestsCompared=len(old_provider), actualRestarts=2, invalidActionsPerEpisode=11,
                      actorCodeUnchanged=True, evaluatorScoresChanged=True, returnedSourceRanksUnchanged=True,
                      comparison="Only acknowledged public IDs normalized through per-run source mappings; raw UUIDs/times retained. No reset/determinism claim.")
    finally:
        report["artifactsSha256"] = {str(p.relative_to(args.out)): sha(p) for p in sorted(args.out.rglob("*")) if p.is_file() and p.name != "report.json"}
        save(args.out / "report.json", report)
    print(json.dumps({k:v for k,v in report.items() if k not in ("artifactsSha256", "rejections")}, sort_keys=True))


if __name__ == "__main__":
    main()
