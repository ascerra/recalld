"""External research-reference metrics consumed by the actual public MCP runner."""
import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys

TRACK = "serialized-mcp-beir-graded-synthetic-diagnostic-v1"
POLICY = dict(metricAdapterVersion=1, reference="beir-2.0.0-pytrec_eval-0.5-trec_eval-9.0.8",
              scoreMapping="public-order-descending-integers-v1", depth=100,
              grades=[0, 1, 2, 3], relevanceLevel=1, ignoreIdenticalIds=False,
              unknownAnnotations="explicitly-excluded", failedQueries="invalidate-aggregate",
              intendedUse="isolated-noncommercial-research-only")
LOCK_PATH = Path(__file__).with_name("beir-reference-lock.json")


def fixtures(h, changed_future=False):
    actor, _, _, _ = h["fixtures"]()
    prototype = actor["sources"][0]
    sources, entries = [], {"a6-document:health check": [0., 0., 0., 1.]}
    for i in range(122):
        source = dict(prototype, sourceId=f"doc{i:03d}", turnId=f"doc{i:03d}", ordinal=i,
                      summary=f"Synthetic document ordinal {i:03d}", namespace="default" if i < 121 else "foreign")
        source["contentSha256"] = h["digest"](source["summary"].encode())
        sources.append(source)
        cosine = 0.99 - i * 0.006 if i < 121 else 1.0
        entries["a6-document:" + source["summary"]] = [cosine, math.sqrt(1 - cosine * cosine), 0., 0.]
    actor["sources"] = sources
    future = "FUTURE_QUESTION_CHANGED_CANARY" if changed_future else "FUTURE_QUESTION_CANARY"
    queries = [dict(queryId=f"q{i}", text="retrieval probe" + (" " + future if i == 3 else "")) for i in range(4)]
    qrels = {"q0": {s["sourceId"]: 1 + i % 3 for i, s in enumerate(sources[:121])},
             "q1": {"doc000": 3, "doc060": 1, "doc120": 2}, "q2": {"doc000": 0}}
    labels = dict(adapterVersion=1, annotationUnit="synthetic-unit", metricQrels=qrels,
                  relevant={q: [d for d, grade in values.items() if grade > 0] for q, values in qrels.items()},
                  labelCanary="EVALUATOR_LABEL_CANARY_ONLY")
    steps = [dict(tag="query:" + q["queryId"], tool="recall_memories",
                  arguments=dict(query=q["text"], namespace="default", recordAccess=False, depth=0, limit=100, compact=True)) for q in queries]
    steps.append(dict(tag="replay-miss", tool="recall_memories",
                      arguments=dict(query="unrecorded-query", namespace="default", recordAccess=False, depth=0, limit=100, compact=True)))
    for q in queries:
        entries["a6-query:" + q["text"]] = [1., 0., 0., 0.]
    return actor, labels, dict(model=h["MODEL"], dimension=4, entries=entries), dict(
        scheduleVersion=1, queries=queries, steps=steps, futureQueryCanary=future, futureVariant="changed" if changed_future else "baseline")


def validate_labels(labels, actor, schedule):
    qrels = labels["metricQrels"]
    questions = {q["queryId"] for q in schedule["queries"]}
    documents = {s["sourceId"] for s in actor["sources"] if s["namespace"] == "default"}
    if questions & documents or not isinstance(qrels, dict) or not set(qrels) <= questions:
        raise ValueError("invalid graded query roster or identity collision")
    for values in qrels.values():
        if not isinstance(values, dict) or not values or not set(values) <= documents:
            raise ValueError("unknown document or ambiguous empty judgments")
        if any(type(v) is not int or v not in POLICY["grades"] for v in values.values()):
            raise ValueError("invalid relevance grade")
    binary = {q: [d for d, grade in values.items() if grade > 0] for q, values in qrels.items()}
    if binary != labels["relevant"]:
        raise ValueError("graded and binary relevance projections differ")


def verify_reference(root):
    if root is None:
        raise ValueError("external research reference bundle required")
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("reference must be an actual bundle directory")
    lock = json.loads(LOCK_PATH.read_text())
    if sys.version_info[:3] != (3, 11, 2) or hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest() != lock["pythonSha256"]:
        raise ValueError("reference interpreter identity mismatch")
    actual = {}
    for p in sorted(root.rglob("*")):
        if p.is_symlink():
            raise ValueError("reference symlinks unsupported")
        if p.is_file():
            actual[str(p.relative_to(root))] = hashlib.sha256(p.read_bytes()).hexdigest()
    if actual != lock["files"]:
        raise ValueError("reference installed-file roster/hash mismatch")
    return hashlib.sha256(LOCK_PATH.read_bytes()).hexdigest()


def reference_command(root, *arguments):
    env = dict(PATH="/usr/local/bin:/usr/bin:/bin", HOME="/tmp", PYTHONPATH=str(Path(root).resolve()),
               PYTHONDONTWRITEBYTECODE="1", OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS="1")
    return subprocess.run([sys.executable, "-P", str(Path(__file__).resolve()), "--reference", str(Path(root).resolve()), *arguments],
                          env=env, cwd="/tmp", capture_output=True, text=True)


def preflight(m, labels, actor, schedule, root):
    if json.dumps(m["metricPolicy"], sort_keys=True) != json.dumps(POLICY, sort_keys=True) or m["referenceBundleSha256"] != verify_reference(root):
        raise ValueError("unsupported metric/reference policy")
    validate_labels(labels, actor, schedule)
    probe = reference_command(root, "--probe")
    if probe.returncode or json.loads(probe.stdout) != {"knownEmptyAndZeroKeys": True}:
        raise ValueError("reference empty/zero population gate failed: " + probe.stderr)


def score_reference(qrels, runs, roster):
    from beir.retrieval.evaluation import EvaluateRetrieval
    import pytrec_eval
    if len(roster) != len(set(roster)) or set(runs) != set(roster) or not set(qrels) <= set(roster):
        raise ValueError("incomplete/duplicate query roster")
    for q, values in qrels.items():
        if not isinstance(values, dict) or not values or any(type(v) is not int or v not in POLICY["grades"] for v in values.values()):
            raise ValueError("invalid or ambiguous judgments")
        if set(values) & set(roster):
            raise ValueError("query/document identity collision")
    for values in runs.values():
        if not isinstance(values, dict) or any(type(v) not in (int, float) or not math.isfinite(v) for v in values.values()):
            raise ValueError("invalid ranking values")
    known = {q: runs[q] for q in roster if q in qrels}
    if not known:
        return dict(status="unscored", knownQueries=[], unknownQueries=roster, official=None, perQuery=None)
    # Completeness is checked before invoking the reference, which otherwise skips keys.
    raw = pytrec_eval.RelevanceEvaluator(qrels, {"ndcg_cut.10", "map_cut.100", "recall.5,10,20"}).evaluate(known)
    if set(raw) != set(known):
        raise ValueError("reference dropped known query keys")
    ndcg, mean_ap, recall, precision = EvaluateRetrieval.evaluate(qrels, copy.deepcopy(known), [5, 10, 20, 100], ignore_identical_ids=False)
    mrr = EvaluateRetrieval.evaluate_custom(qrels, copy.deepcopy(known), [10], metric="mrr")
    return dict(status="scored", knownQueries=list(known), unknownQueries=[q for q in roster if q not in known],
                perQuery=raw, official=dict(ndcg=ndcg, map=mean_ap, recall=recall, precision=precision, mrr=mrr),
                officialDenominator=str(len(known)), binaryZeroPositivePolicy="unscored-separate-output",
                qualityClaim=False)


def consume(args, h, m, actor, labels, schedule, wire, score_input):
    expected = [q["queryId"] for q in schedule["queries"]]
    mappings = score_input["mappings"]
    source_by_id = {s["sourceId"]: s for s in actor["sources"]}
    if len(mappings) != len(source_by_id) or any(len(v) != 1 for v in mappings.values()) or sorted(v[0] for v in mappings.values()) != sorted(source_by_id):
        raise ValueError("non-bijective acknowledged document mapping")
    query_rows = [r for r in wire if r["tag"].startswith("query:")]
    attempts = [r for r in query_rows if r["event"] == "attempt"]
    outcomes = [r for r in query_rows if r["event"] == "outcome"]
    if [r["tag"][6:] for r in attempts] != expected or [r["tag"][6:] for r in outcomes] != expected:
        raise ValueError("missing/duplicate/failed query roster")
    if [r["request"] for r in attempts] != [r["request"] for r in outcomes]:
        raise ValueError("query attempt/outcome differs")
    runs, observations = {}, {}
    for q, row, step in zip(expected, outcomes, schedule["steps"]):
        if row["request"]["params"] != dict(name=step["tool"], arguments=step["arguments"]):
            raise ValueError("actual retrieval policy differs")
        result = row["response"].get("result", {})
        if result.get("isError") or "error" in row["response"]:
            raise ValueError("failed query is not an empty ranking")
        payload = json.loads(result["content"][0]["text"])
        if "structuredContent" in result and result["structuredContent"] != payload:
            raise ValueError("public payload mismatch")
        memories = payload["memories"]
        if payload["count"] != len(memories) or len(memories) != 100:
            raise ValueError("saturated121-to100 public depth gate failed")
        ids = [v["id"] for v in memories]
        if len(set(ids)) != len(ids) or any(v not in mappings for v in ids):
            raise ValueError("duplicate/unknown public memory")
        documents = [mappings[v][0] for v in ids]
        if any(source_by_id[d]["namespace"] != "default" for d in documents):
            raise ValueError("foreign result")
        runs[q] = {d: float(100 - i) for i, d in enumerate(documents)}
        observations[q] = [dict(rank=str(i + 1), memoryId=v["id"], sourceId=d,
                                publicScore=v.get("score"), referenceOrdinalScore=float(100 - i))
                           for i, (v, d) in enumerate(zip(memories, documents))]
    value = dict(qrels=labels["metricQrels"], runs=runs, roster=expected)
    evaluator = args.out / "evaluator"
    h["save"](evaluator / "beir-score-input.json", value)
    result = reference_command(args.beir_reference, "--input", str((evaluator / "beir-score-input.json").resolve()))
    h["save"](evaluator / "beir-reference-execution.json", dict(exitCode=result.returncode, stdout=result.stdout, stderr=result.stderr))
    if result.returncode:
        raise ValueError("official reference consumer failed")
    scores = json.loads(result.stdout)
    h["save"](evaluator / "beir-scores.json", dict(scores, metricPolicy=POLICY, publicOrder=observations,
              referenceBundleSha256=m["referenceBundleSha256"], defaultDocuments="121", returnedPerQuery="100",
              binaryDiagnosticsFile="scores.json", allRequiredIsSeparateBinaryDiagnostic=True))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--input", type=Path)
    args = parser.parse_args()
    verify_reference(args.reference)
    if args.probe:
        for qrels, run in [({"q": {"a": 1}}, {"q": {}}), ({"q": {"a": 0}}, {"q": {"a": 100.}})]:
            value = score_reference(qrels, run, ["q"])
            if set(value["perQuery"]) != {"q"} or any(v != 0 for v in value["perQuery"]["q"].values()) or value["official"]["mrr"]["MRR@10"] != 0:
                raise ValueError("known empty/zero reference gate")
        print(json.dumps(dict(knownEmptyAndZeroKeys=True)))
    else:
        value = json.loads(args.input.read_text())
        print(json.dumps(score_reference(value["qrels"], value["runs"], value["roster"])))


if __name__ == "__main__":
    main()
