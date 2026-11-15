#!/usr/bin/env python3
"""Actual owned runner rejection and label/future-channel canaries; synthetic only."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    for flag in ("source", "inputs", "baseline", "binary", "scorer", "out"):
        parser.add_argument("--" + flag, type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir()
    harness = runpy.run_path(str(args.source / "scripts/public-mcp-evaluation.py"))
    save, digest, rows = harness["save"], harness["digest"], harness["rows"]
    manifest = json.loads((args.inputs / "manifest.json").read_text())
    report = dict(status="running", rejections=[], sourceCommit=manifest["sourceCommit"], executableHashes=manifest["executables"])
    def run(inputs, output):
        return subprocess.run([sys.executable, str(args.source / "scripts/public-mcp-evaluation.py"), "run",
            "--manifest", str(inputs / "manifest.json"), "--source-root", str(args.source), "--binary", str(args.binary),
            "--scorer", str(args.scorer), "--out", str(output)], capture_output=True, text=True)
    def inputs(name, change):
        root = args.out / name
        shutil.copytree(args.inputs, root)
        m = copy.deepcopy(manifest)
        artifacts = {k: json.loads((root / m[k]["file"]).read_text()) for k in ("actorInputs", "evaluatorLabels", "evaluatorSchedule", "replayCache")}
        change(m, artifacts)
        for key, value in artifacts.items():
            save(root / m[key]["file"], value)
            m[key]["sha256"] = digest((root / m[key]["file"]).read_bytes())
        m["dataset"]["actorArtifactSha256"] = m["actorInputs"]["sha256"]
        save(root / "manifest.json", m)
        return root
    try:
        cases = [("version-" + field, lambda m,a,field=field:m.update({field:2})) for field in ("manifestVersion", "sourceMappingVersion", "traceSchemaVersion")]
        cases += [("capability-" + field, lambda m,a,field=field:m["capabilities"].update({field:True})) for field in ("publicV2", "fullA6", "reset", "snapshot", "deterministicRng", "deterministicIds", "clockAdvance", "coherentReadView")]
        cases += [("native-schema", lambda m,a:m["versions"].update(canonicalSchemaVersion=2)),
                  ("fake-zero-schema", lambda m,a:m["versions"].update(canonicalSchemaVersion=0)),
                  ("control-version", lambda m,a:m["versions"].update(privateControlVersion=2)),
                  ("future-actor", lambda m,a:a["actorInputs"].update(queries=a["evaluatorSchedule"]["queries"])),
                  ("label-actor", lambda m,a:a["actorInputs"].update(answer=a["evaluatorLabels"]["labelCanary"])),
                  ("fake-source", lambda m,a:m.update(sourceCommit="0"*40)),
                  ("fake-binary", lambda m,a:m["executables"].update(ownedEvaluator="0"*64)),
                  ("replay-fallback", lambda m,a:a["replayCache"]["entries"].update({"a6-query:unrecorded-query":[1.,0.,0.,0.]}))]
        for name, change in cases:
            root = inputs("reject-" + name, change)
            out = root / "must-not-exist"
            result = run(root, out)
            report["rejections"].append(dict(case=name, exitCode=result.returncode, outputCreated=out.exists(), stdout=result.stdout, stderr=result.stderr))
            save(args.out / "report.json", report)
            assert result.returncode != 0 and not out.exists(), name
        def perturb(m, a):
            a["evaluatorLabels"]["labelCanary"] = "CHANGED_OWNED_EVALUATOR_LABEL"
            a["evaluatorLabels"]["relevant"] = {q["queryId"]:["turn-3"] for q in a["evaluatorSchedule"]["queries"]}
        changed = inputs("changed-label-inputs", perturb)
        variant = args.out / "changed-label-episode"
        result = run(changed, variant)
        save(args.out / "changed-label-run.json", dict(exitCode=result.returncode, stdout=result.stdout, stderr=result.stderr))
        assert result.returncode == 0, result.stderr
        baseline = args.baseline
        def normalized_requests(root):
            mappings = json.loads((root / "source-mappings.json").read_text())["mappings"]
            identity = {memory: "$" + values[0] for memory, values in mappings.items()}
            def normalize(v):
                if isinstance(v, dict): return {k:normalize(x) for k,x in v.items()}
                if isinstance(v, list): return [normalize(x) for x in v]
                return identity.get(v,v) if isinstance(v,str) else v
            return [normalize(r["request"]) for r in rows(root / "actor-wire.jsonl") if r["event"] == "attempt"], normalize
        old, normal_old = normalized_requests(baseline)
        new, normal_new = normalized_requests(variant)
        assert old == new
        projections = sorted((baseline / "actor-inputs").glob("*.json"))
        for path in projections:
            assert normal_old(json.loads(path.read_text())) == normal_new(json.loads((variant / "actor-inputs" / path.name).read_text()))
        assert (baseline / "actor-code.py").read_bytes() == (variant / "actor-code.py").read_bytes()
        providers = [rows(p / "actor-provider.jsonl") for p in (baseline, variant)]
        assert [(p["requestBody"],p["status"],p["responseVectors"]) for p in providers[0]] == [(p["requestBody"],p["status"],p["responseVectors"]) for p in providers[1]]
        assert all(a["cacheKeys"] != b["cacheKeys"] for a,b in zip(*providers))
        scores = [json.loads((p / "evaluator/scores.json").read_text()) for p in (baseline, variant)]
        assert [r["retrievedSourceIds"] for r in scores[0]["trace"]] == [r["retrievedSourceIds"] for r in scores[1]["trace"]]
        assert scores[0]["scoring"]["scored"] == "2" and scores[0]["scoring"]["unknownAnnotations"] == "1" and scores[0]["scoring"]["zeroLabelUnscored"] == "1"
        assert scores[1]["scoring"]["scored"] == "4" and scores[1]["scoring"]["means"]["retrievalRecallAt5"] == 0
        for episode in (baseline, variant):
            result = json.loads((episode / "result.json").read_text())
            assert result["futureQueryCanaryWithheldUntilDelivery"] is True
            wire = rows(episode / "actor-wire.jsonl")
            assert len([r for r in wire if r["tag"] == "control-fds-closed"]) == len(projections)
            for r in wire:
                if r["tag"] == "control-fds-closed": assert r["count"] == 2 and r["fdClosed"] is True
        report.update(status="passed", actionsCompared=len(old), projectionsCompared=len(projections), providerRequestsCompared=len(providers[0]),
            actorActionsAndVectorsStableUnderLabelChange=True, replayCacheIdentityChanged=True, returnedSourceRanksStable=True,
            futureQueryAbsentUntilCurrentDelivery=True, actualControlDescriptorsClosedInEveryActor=True,
            comparison="Per-run UUID mapping normalized for actions; timestamps/UUIDs retained in raw artifacts. No deterministic reset claim.")
    finally:
        report["artifactsSha256"] = {str(p.relative_to(args.out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in args.out.rglob("*") if p.is_file() and p.name != "report.json"}
        save(args.out / "report.json", report)
    print(json.dumps({k:v for k,v in report.items() if k not in ("artifactsSha256", "rejections")}))


if __name__ == "__main__":
    main()
