#!/usr/bin/env python3
"""Actual reset-track preflight/label canaries; requires the frozen owner binary."""
import argparse
import copy
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import sys


def main():
    parser=argparse.ArgumentParser()
    for flag in ("source","inputs","baseline","binary","scorer","out"):
        parser.add_argument("--"+flag,type=Path,required=True)
    args=parser.parse_args();args.out.mkdir()
    h=runpy.run_path(str(args.source / "scripts/public-mcp-evaluation.py"))
    m=json.loads((args.inputs / "manifest.json").read_text())
    report=dict(status="running",sourceCommit=m["sourceCommit"],executables=m["executables"],rejections=[])
    def inputs(name, mutate):
        root=args.out/name;shutil.copytree(args.inputs,root)
        manifest=copy.deepcopy(m)
        labels=json.loads((root/manifest["evaluatorLabels"]["file"]).read_text())
        mutate(manifest,labels)
        h["save"](root/manifest["evaluatorLabels"]["file"],labels)
        manifest["evaluatorLabels"]["sha256"]=h["digest"]((root/manifest["evaluatorLabels"]["file"]).read_bytes())
        h["save"](root/"manifest.json",manifest)
        return root
    def run(root, out):
        return subprocess.run([sys.executable,str(args.source/"scripts/public-mcp-evaluation.py"),"run","--manifest",str(root/"manifest.json"),
            "--binary",str(args.binary),"--scorer",str(args.scorer),"--source-root",str(args.source),"--out",str(out)],capture_output=True,text=True)
    try:
        cases=[("unsupported-"+field,lambda m,l,field=field:m["capabilities"].update({field:True})) for field in ("reset","fullA6","publicV2","coherentReadView","deterministicIds","deterministicRng")]
        cases += [("version-"+field,lambda m,l,field=field:m["versions"].update({field:2})) for field in ("checkpointVersion","privateControlVersion","canonicalSchemaVersion")]
        cases += [("full-clock",lambda m,l:m["capabilities"].update(clock="fully-controlled")),
                  ("unknown-source-label",lambda m,l:l["relevant"].update(q0=["not-a-source"])),
                  ("false-native-zero",lambda m,l:m["versions"].update(canonicalSchemaVersion=0))]
        for name,mutate in cases:
            root=inputs("reject-"+name,mutate);out=root/"must-not-exist";r=run(root,out)
            report["rejections"].append(dict(case=name,exitCode=r.returncode,outputCreated=out.exists(),stdout=r.stdout,stderr=r.stderr))
            h["save"](args.out/"report.json",report)
            assert r.returncode!=0 and not out.exists(),name
        def perturb(m,labels):
            labels["labelCanary"]="CHANGED_RESET_EVALUATOR_LABEL"
            labels["relevant"]={q["queryId"]:["turn-3"] for q in h["fixtures"](h["RESET"]["TRACK"])[3]["queries"]}
        root=inputs("changed-label-inputs",perturb);variant=args.out/"changed-label-episode";r=run(root,variant)
        h["save"](args.out/"changed-label-run.json",dict(exitCode=r.returncode,stdout=r.stdout,stderr=r.stderr))
        assert r.returncode==0,r.stderr
        baseline=json.loads((args.baseline/"result.json").read_text());changed=json.loads((variant/"result.json").read_text())
        assert baseline["status"]==changed["status"]=="passed"
        assert baseline["freshOwnerSuffixReplays"]==changed["freshOwnerSuffixReplays"]==3
        assert baseline["environmentHash"]!=changed["environmentHash"]
        compared=0
        for name in ("original","restored-0","restored-1","restored-2"):
            a,b=[root/"evaluator"/name for root in (args.baseline,variant)]
            old=h["rows"](a/"actor-wire.jsonl");new=h["rows"](b/"actor-wire.jsonl")
            actions=lambda rows:[r["request"] for r in rows if r["event"]=="attempt"]
            assert actions(old)==actions(new),name  # Actual owned IDs, no normalization.
            suffixes=[json.loads((p/"suffix.json").read_text()) for p in (a,b)]
            # Independent ingestions create legacy namespaces on wall time; their
            # private canonical hashes are not an actor-output equality contract.
            # Each run separately requires full logicalHash equality on 3 restores.
            for field in ("outputs","mappings","state","readiness","provider"):
                assert suffixes[0][field] == suffixes[1][field],(name,field)
            report.setdefault("independentLogicalHashes",{})[name]=[v["logicalHash"] for v in suffixes]
            compared+=len(actions(old))
            for p in (a/"actor-inputs").glob("*.json"):
                assert p.read_bytes()==(b/"actor-inputs"/p.name).read_bytes()
            for directory in (a,b):
                mapping=json.loads((directory/"source-mappings.json").read_text())
                assert set(mapping)=={"sourceMappingVersion","actorArtifactSha256","mappings","sources"} and mapping["sourceMappingVersion"]==1
                traces=h["rows"](directory/"trace.jsonl")
                for trace in traces:
                    assert set(trace)=={"traceSchemaVersion","runId","episodeId","groupId","stepId","environmentHash","action","outcome","readGeneration","readiness","simulatedTimeMs","snapshot","reward"}
                    assert trace["traceSchemaVersion"]==1 and trace["readGeneration"] is None
                    assert trace["action"]["id"]==trace["outcome"]["id"]==int(trace["stepId"])
            scores=[json.loads((p/"scores.json").read_text()) for p in (a,b)]
            assert scores[0]["scoring"]["scored"]=="3" and scores[0]["scoring"]["unknownAnnotations"]=="1" and scores[0]["scoring"]["zeroLabelUnscored"]=="1"
            assert scores[1]["scoring"]["scored"]=="5" and scores[1]["scoring"]["means"]["retrievalRecallAt5"]==0
            assert [v["retrievedSourceIds"] for v in scores[0]["trace"]]==[v["retrievedSourceIds"] for v in scores[1]["trace"]]
        providers=[h["rows"](p/"actor-provider.jsonl") for p in (args.baseline,variant)]
        assert [(r["requestBody"],r["status"],r["responseVectors"]) for r in providers[0]]==[(r["requestBody"],r["status"],r["responseVectors"]) for r in providers[1]]
        assert all(a["cacheKeys"]!=b["cacheKeys"] for a,b in zip(*providers))
        report.update(status="passed",rawActorActionsCompared=compared,rawActorIdsAndVectorsStableUnderLabelChange=True,
            sourceRanksStable=True,environmentAndCacheIdentityChanged=True,freshOwnerSuffixReplaysPerRun=3,
            comparison="Actual owned fixture IDs/actions compared without normalization; no all-ties/RNG/fullA6 claim.")
    except BaseException as error:
        report.update(status="failed",error=str(error));raise
    finally:
        report["artifactsSha256"]={str(p.relative_to(args.out)):h["digest"](p.read_bytes()) for p in args.out.rglob("*") if p.is_file() and not p.is_symlink() and p.name!="report.json"}
        h["save"](args.out/"report.json",report)
    print(json.dumps({k:v for k,v in report.items() if k not in ("rejections","artifactsSha256")}))


if __name__=="__main__":main()
