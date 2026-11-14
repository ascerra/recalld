#!/usr/bin/env python3
"""Snapshot this checkout, build/test offline, and compare three fresh processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import uuid

IMAGE = "recalld-ci:1.94"
ROOT = Path(__file__).resolve().parents[1]
FIXED_INPUTS = (
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "examples/offline_eval.rs",
    "scripts/offline-evaluation.py", "scripts/offline-evaluation.Dockerfile",
    "scripts/test_offline_evaluation.py", "scripts/check-evaluation-contract.py",
    "src/storage/metadata/native_records/fixtures/phase-clock-legacy.jsonl",
)


def command(args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def snapshot_source(root, destination, extra=()):
    # Tracked target inputs only; never copy scratch, ambient Cargo config or secrets.
    tracked = {os.fsdecode(p) for p in subprocess.check_output(
        ["git", "ls-files", "-z", "--cached"], cwd=root).split(b"\0") if p}
    roots = ("src/", "tests/", "examples/", "benches/")
    legacy_data = {f"src/bench/data/{name}.json" for name in
                   ("locomo1", "locomo10", "locomo_conv30", "locomo_conv41", "locomo_first")}
    names = set(FIXED_INPUTS) | set(extra)
    for name in tracked:
        if name.startswith(roots):
            if name.endswith(".rs"):
                names.add(name)
            elif name in names:
                pass
            elif name not in legacy_data:
                raise RuntimeError(f"undeclared target asset requires explicit allowlist: {name}")
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    if "workspace" in cargo:
        raise RuntimeError("workspace snapshot requires explicit policy")
    for kind in ("lib", "bin", "test", "example", "bench"):
        targets = [cargo[kind]] if kind == "lib" and kind in cargo else cargo.get(kind, [])
        for target in targets:
            if target.get("path") and target["path"] not in names:
                raise RuntimeError(f"undeclared Cargo target: {target['path']}")
    build = cargo.get("package", {}).get("build", "build.rs" if "build.rs" in tracked else False)
    if build and build not in names:
        raise RuntimeError(f"undeclared build script: {build}")
    if not names <= tracked:
        raise RuntimeError("required source inputs must be tracked")
    hashes = {}
    for name in sorted(names):
        source = root / name
        relative = Path(name)
        if (not source.is_file() or not source.resolve().is_relative_to(root.resolve())
                or any((root / Path(*relative.parts[:i])).is_symlink()
                       for i in range(1, len(relative.parts) + 1))):
            raise RuntimeError(f"missing, nonregular or escaping source input: {name}")
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        hashes[name] = sha(target)
    return hashes


def source_patch(root, names):
    # Unrelated tracked changes can also contain credentials or bulky output.
    return subprocess.check_output(["git", "diff", "HEAD", "--binary", "--", *sorted(names)], cwd=root)


def verify_replays(output, runs):
    # Canonical receipts/store UUIDs stay in evidence but are not deterministic.
    stable = ["fixture.json", "state-before.json", "state-after.json", "replay.json"]
    for name in stable:
        if len({sha(output / f"run-{run}" / name) for run in range(runs)}) != 1:
            raise RuntimeError(f"replay mismatch: {name}")
    receipt_semantics = []
    controls = []
    for run in range(runs):
        root = output / f"run-{run}"
        for kind in ("state", "control"):
            if sha(root / f"{kind}-before.json") != sha(root / f"{kind}-after.json"):
                raise RuntimeError(f"false query changed {kind}: run-{run}")
        control = json.loads((root / "control-before.json").read_text())
        store_id = control.pop("storeId")
        if not uuid.UUID(store_id).int:
            raise RuntimeError("nil store identity")
        controls.append(control)
        sources = json.loads((root / "fixture.json").read_text())["sources"]
        state = json.loads((root / "state-before.json").read_text())
        ledger = [json.loads(line) for line in (root / "turn-ledger.jsonl").read_text().splitlines()]
        if len(ledger) != 2 * len(sources) or set(state) != {s["memoryId"] for s in sources}:
            raise RuntimeError("fixture/ledger/record count mismatch")
        operations, semantic = set(), []
        for i, source in enumerate(sources):
            attempted, accepted = ledger[2*i:2*i+2]
            identity = {"sourceId": source["sourceId"], "memoryId": source["memoryId"]}
            if attempted != {**identity, "status": "attempted"} or any(accepted[k] != v for k, v in identity.items()) or accepted["status"] != "accepted":
                raise RuntimeError("ledger identity/reference mismatch")
            receipt = accepted["receipt"].copy()
            operation = receipt.pop("operationId")
            if not uuid.UUID(operation).int or operation in operations or receipt.pop("storeId") != store_id:
                raise RuntimeError("receipt identity mismatch")
            operations.add(operation)
            if (receipt["receiptVersion"] != 1 or receipt["replayed"] is not False
                    or receipt["outcome"]["kind"] != "memoryInserted"
                    or receipt["outcome"]["memoryId"] != source["memoryId"]
                    or receipt["committedGeneration"] != str(i + 4)):
                raise RuntimeError("receipt outcome/write count mismatch")
            semantic.append({**accepted, "receipt": receipt})
        # This source initializes control, creates two fixture namespaces, inserts
        # N records, then production startup creates its default namespace.
        if control["committedGeneration"] != str(len(sources) + 4):
            raise RuntimeError("unexpected canonical write count")
        receipt_semantics.append(semantic)
    if any(value != receipt_semantics[0] for value in receipt_semantics[1:]) or any(value != controls[0] for value in controls[1:]):
        raise RuntimeError("receipt/control semantics differ beyond declared generated identities")
    return stable


def validate_execution_plan(args):
    if not (100 <= args.records <= 100_000 and args.records % 10 == 0
            and 4 <= args.dimensions <= 8192 and 1 <= args.k <= min(200, args.records // 10)
            and 1 <= args.samples <= 100_000 and 0 <= args.warmup <= 100_000
            and 0 <= args.seed <= 2**64 - 1):
        raise ValueError("unsupported execution plan")


def evaluation_inputs(args, diagnostic):
    """Generated synthetic adapter only; no external dataset/license/model claims."""
    validate_execution_plan(args)
    actor = {"adapterVersion": 1, "sources": [
        {"sourceId": f"unit-{i:06}", "turnId": f"unit-{i:06}", "ordinal": i,
         "groupId": "synthetic-regression", "conversationId": "synthetic-conversation",
         "sessionId": "synthetic-session", "role": "user", "sourceTimestamp": 1767225600000,
         "summary": f"fixture token{i}", "contentSha256": hashlib.sha256(f"fixture token{i}".encode()).hexdigest()}
        for i in range(args.records)]}
    evaluator = {"adapterVersion": 1, "annotationUnit": "synthetic-unit",
                 "relevant": {f"query{i}": [f"unit-{i:06}"] for i in range(0, args.records, 10)},
                 "labelCanary": "EVALUATOR_ONLY_CANARY_NOT_ACTOR_INPUT"}
    # Deliberately synthetic annotation coverage cases; actor artifact stays unchanged.
    evaluator["relevant"]["query10"] = []
    evaluator["relevant"].pop("query20")
    evaluator["relevant"]["query30"] = ["unit-000030", f"unit-{args.records-1:06}"]
    encode = lambda v: (json.dumps(v, indent=2, sort_keys=True) + "\n").encode()
    actor_bytes, evaluator_bytes = encode(actor), encode(evaluator)
    actor_hash = hashlib.sha256(actor_bytes).hexdigest()
    manifest = {"manifestVersion": 1, "sourceMappingVersion": 1, "traceSchemaVersion": 1,
        "track": "manifest-internal-diagnostic-v2",
        **{key: diagnostic[key] for key in ("sourceCommit", "dirtyPatchSha256", "sourceFilesSha256",
                                           "lockfileSha256", "containerImageId", "limits")},
        "sourceTreeSha256": hashlib.sha256(json.dumps(diagnostic["sourceFilesSha256"], sort_keys=True,
            separators=(",", ":"), ensure_ascii=False).encode()).hexdigest(),
        "versions": {"contractVersion": 1, "retrievalSemanticsVersion": 1, "rankingProfile": "baseline-v1",
            "lifecycleVersion": "legacy_access_v1", "rifMode": "off", "canonicalSchemaVersion": None,
            "canonicalRepresentation": "legacy-schema-none", "controlSchemaVersion": 1,
            "recordSchemaVersion": 1, "backupFormatVersion": None, "exportFormatVersion": None,
            "offlineFormats": "not-used-or-advertised-by-this-track",
            "receiptVersion": 1, "requestHashVersion": 1,
            "derivedIndexFormats": "sourceFilesSha256; no independent format version advertised"},
        "capabilities": {"publicApi": False, "publicV2": False, "nativeV2": False, "fullA6": False, "coherentReadView": False,
            "barrier": False, "snapshot": False, "reset": False, "deterministicIds": False,
            "deterministicRng": False, "clock": "fixed-lifecycle-only", "clockAdvance": False,
            "sweeps": False, "initialTimeMs": 1767225600000, "recordAccess": False, "graphDepth": 0},
        "dataset": {"kind": "generated-synthetic", "upstreamUrl": None, "upstreamRevision": None,
            "thirdPartyLicense": None, "intendedUse": "contract-diagnostic-only", "adapterVersion": 1,
            "groupSplit": "one-synthetic-regression-group; no training", "datasetRevision": actor_hash},
        "scoring": {"profile": "synthetic-unique-source-v1", "annotationUnit": "synthetic-unit",
            "deduplication": "first-source-occurrence-before-cutoff", "relevance": "binary",
            "recallCutoffs": [5, 10, 20], "ndcgCutoff": 10, "allRequired": "all-returned-unique-sources",
            "emptyLabels": "unscored", "missingAnnotations": "unknown", "aggregation": "measured-query-events-only"},
        "embedding": {"generator": "splitmix64-f32-l2-v1", "dimensions": args.dimensions,
            "modelRevision": None, "promptSha256": None, "tokenizerRevision": None,
            "role": "actor-query", "cacheMiss": "error", "externalCalls": False},
        "execution": {k: getattr(args, k) for k in ("records", "dimensions", "k", "warmup", "samples", "seed")},
        "actorInputs": {"file": "actor-inputs.json", "sha256": actor_hash},
        "evaluatorLabels": {"file": "evaluator-labels.json", "sha256": hashlib.sha256(evaluator_bytes).hexdigest()}}
    return manifest, actor_bytes, evaluator_bytes


def verify_scoring_outputs(directory, evaluator):
    """Independently recompute metrics from actual actor UUID results and committed mappings."""
    import math
    read_rows = lambda name: [json.loads(line) for line in (directory / name).read_text().splitlines()]
    mappings = {row["memoryId"]: row["sourceIds"] for row in read_rows("source-mappings.jsonl")}
    actor = {row["stepId"]: row for row in read_rows("trace.jsonl")}
    evaluator_rows = read_rows("evaluator/trace.jsonl")
    metrics = [row for row in evaluator_rows if "metrics" in row]
    expected_steps = {step for step, row in actor.items()
                      if row["action"]["kind"] == "internal-query" and row["outcome"]["status"] == "ok"
                      and row["outcome"]["warmup"] is False}
    assert len(metrics) == len(expected_steps) and {row["stepId"] for row in metrics} == expected_steps
    statuses = {"scored": 0, "zero_label": 0, "unknown": 0}
    scores = []
    for row in metrics:
        action = actor[row["stepId"]]
        assert action["outcome"]["warmup"] is False
        assert action["action"]["request"]["text"] == row["queryId"]
        retrieved = [source for result in action["outcome"]["results"] for source in mappings[result["memoryId"]]]
        assert retrieved == row["retrievedSourceIds"]
        labels = evaluator["relevant"].get(row["queryId"])
        status = "unknown" if labels is None else "zero_label" if not labels else "scored"
        result = row["metrics"]
        assert result["annotationStatus"] == status
        statuses[status] += 1
        if status != "scored":
            assert result["scored"] is False and "retrievalRecallAt5" not in result
            continue
        gold, ranked = set(labels), list(dict.fromkeys(retrieved))
        for k in (5, 10, 20):
            assert result[f"retrievalRecallAt{k}"] == len(gold & set(ranked[:k])) / len(gold)
        dcg = sum(1 / math.log2(rank + 2) for rank, source in enumerate(ranked[:10]) if source in gold)
        ideal = sum(1 / math.log2(rank + 2) for rank in range(min(10, len(gold))))
        assert abs(result["binarySourceNdcgAt10"] - dcg / ideal) < 1e-12
        assert result["allRequiredEvidence"] == gold.issubset(ranked)
        retained = set(source for sources in mappings.values() for source in sources)
        assert result["extractionEvidenceRetention"] == len(gold & retained) / len(gold)
        scores.append(result)
    summary = json.loads((directory / "evaluator/scoring-summary.json").read_text())
    assert summary["queryEvents"] == str(len(metrics))
    for status, name in (("scored", "scored"), ("zero_label", "zeroLabelUnscored"), ("unknown", "unknownAnnotations")):
        assert summary[name] == str(statuses[status])
    for metric, mean in summary["means"].items():
        assert mean == (sum(r[metric] for r in scores) / len(scores) if scores else None)
    return statuses


def verify_manifest_outputs(output, runs):
    contract = json.loads((output / "manifest.json").read_text())
    evaluator = json.loads((output / "evaluator-labels.json").read_text())
    actor = json.loads((output / "actor-inputs.json").read_text())
    canary = evaluator["labelCanary"].encode()
    count = contract["execution"]["records"]
    for run in range(runs):
        directory = output / f"run-{run}"
        for name in ("actor-inputs.json", "actor-replay-cache.json", "trace.jsonl", "source-mappings.jsonl", "replay.json"):
            if canary in (directory / name).read_bytes():
                raise RuntimeError(f"evaluator canary leaked: {name}")
        sources = [json.loads(line) for line in (directory / "source-mappings.jsonl").read_text().splitlines()]
        if (len(sources) != count or any(s["sourceMappingVersion"] != 1
                or s["datasetRevision"] != contract["actorInputs"]["sha256"] for s in sources)):
            raise RuntimeError("provenance version/revision/count mismatch")
        records = json.loads((directory / "state-before.json").read_text())
        if {s["memoryId"] for s in sources} != set(records):
            raise RuntimeError("mapping UUIDs do not match actual canonical records")
        for expected, mapping in zip(actor["sources"], sources):
            if (mapping["source"] != expected or mapping["sourceIds"] != [expected["sourceId"]]
                    or mapping["outputSha256"] != expected["contentSha256"]):
                raise RuntimeError("source mapping golden mismatch")
        events = [json.loads(line) for line in (directory / "evaluator/source-events.jsonl").read_text().splitlines()]
        if len(events) != count * 3 or [e["status"] for e in events] != ["seen", "attempted", "accepted"] * count:
            raise RuntimeError("turn accounting mismatch")
        coverage = json.loads((directory / "evaluator/turn-coverage.json").read_text())
        assert coverage["statuses"] == {"seen": str(count), "attempted": str(count), "accepted": str(count),
            "no_op": "0", "rejected": "0", "truncated": "0"}
        assert coverage["eventCount"] == str(len(events)) and coverage["processed"] == str(count)
        assert coverage["pending"] == coverage["unseen"] == "0"
        assert coverage["processedCoverage"] == coverage["acceptedMemoryCoverage"] == 1.0
        verify_scoring_outputs(directory, evaluator)
        trace = [json.loads(line) for line in (directory / "trace.jsonl").read_text().splitlines()]
        if len(trace) != count + 1 + 4 * (contract["execution"]["warmup"] + contract["execution"]["samples"]):
            raise RuntimeError("actor action/warmup/expected-failure count mismatch")
        if any(t["traceSchemaVersion"] != 1 or t["readiness"]["available"] is not False
               or t["readGeneration"] is not None for t in trace):
            raise RuntimeError("trace claimed unavailable coherent controls")
        if any("metrics" in t["outcome"] or "oracleRecallAtK" in t["outcome"] for t in trace):
            raise RuntimeError("evaluator metric in actor observation")
        misses = [t for t in trace if t["stepId"] == "replay-miss"]
        if len(misses) != 1 or misses[0]["outcome"]["status"] != "expected_error":
            raise RuntimeError("actual QueryEngine replay miss was not retained")
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--track", required=True, choices=["manifest-internal-diagnostic-v2"],
                        help="explicit opt-in; old frozen studies must use their pinned checkout")
    parser.add_argument("--cache", required=True, type=Path, help="Prior scratch Cargo home; only registry/ is copied")
    parser.add_argument("--build-cache", type=Path, help="Optional prior scratch target to copy, never share")
    parser.add_argument("--image", default=IMAGE)
    parser.add_argument("--toolchain", default="1.94.1")
    parser.add_argument("--records", type=int, default=1000)
    parser.add_argument("--dimensions", type=int, default=384)
    parser.add_argument("--k", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=20)
    parser.add_argument("--samples", type=int, default=100)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()
    if args.runs < 3:
        parser.error("at least three processes are required for the replay check")
    validate_execution_plan(args)  # before scratch/output creation
    cache = args.cache.resolve()
    if not str(cache).startswith(("/tmp/recalld-", "/private/tmp/recalld-")) or not (cache / "registry").is_dir():
        parser.error("cache must be a previous recalld scratch directory with registry/")

    # No home/config/credentials/socket mounts. Git is read only on the host.
    scratch = Path(tempfile.mkdtemp(prefix="recalld-evaluation-"))
    source = scratch / "source"
    source.mkdir()
    output = scratch / "output"
    output.mkdir()
    print(f"Artifacts: {output}", flush=True)
    hashes = snapshot_source(ROOT, source)
    shutil.copytree(cache / "registry", scratch / "cargo" / "registry")
    if args.build_cache:
        build_cache = args.build_cache.resolve()
        if build_cache.name != "target" or not (str(build_cache.parent).startswith(("/tmp/recalld-", "/private/tmp/recalld-"))
                or (build_cache.parent.name.startswith("recalld-evaluation-")
                    and build_cache.parent.parent == Path(tempfile.gettempdir()).resolve())):
            parser.error("build cache must be target/ in a prior recalld scratch; copied privately")
        shutil.copytree(build_cache, scratch / "target")
    else:
        (scratch / "target").mkdir()
    patch = source_patch(ROOT, hashes)
    (output / "source.patch").write_bytes(patch)
    image = json.loads(command(["docker", "image", "inspect", args.image]))[0]
    manifest = {
        "diagnosticManifestVersion": 1,
        "scope": "A5/A6 internal foundation; not the integrated A6 manifest/trace contract",
        "status": "running",
        "sourceCommit": command(["git", "rev-parse", "HEAD"], cwd=ROOT),
        "dirtyPatchSha256": hashlib.sha256(patch).hexdigest(),
        "sourceFilesSha256": hashes,
        "sourceInputPolicy": "tracked Rust under src/tests/examples/benches plus FIXED_INPUTS; unknown assets/targets rejected; legacy datasets excluded",
        "dirtyPatchScope": "allowlisted source inputs only; per-file hashes are authoritative",
        "sourceTreeSha256": hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest(),
        "lockfileSha256": hashes["Cargo.lock"],
        "containerImageId": image["Id"], "containerRepoDigests": image.get("RepoDigests", []),
        "architecture": image["Architecture"],
        "command": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
        "limits": {"cpus": 2, "memoryBytes": 4294967296, "cargoJobs": 2, "tokioWorkers": 2},
        "isolation": {"network": "none", "home": "fresh container tmpfs", "mount": "scratch only", "externalCalls": 0},
        "fixture": {"generator": "splitmix64-f32-l2-v1", "source": "synthetic, generated by examples/offline_eval.rs",
                    "intendedUse": "offline contract/measurement diagnostics; no semantic quality claims",
                    "externalDataset": None, "license": "no third-party dataset", "sourceMappingVersion": 1},
        "versions": {"publicContract": None, "retrievalSemantics": "legacy source-pinned internal path",
                     "lifecycle": "legacy_access_v1 materialized strength; no sweeps", "rankingProfile": "baseline-v1",
                     "canonicalSchema": "source-pinned; no v2 claim", "receipts": 1, "readiness": "production startup completion; not whole-store barrier"},
        "embedding": {"profile": "splitmix64-f32-l2-v1", "model": None, "dimensions": args.dimensions,
                      "input": "exact fixture namespace and query text; misses fail closed", "cache": "off"},
        "behavior": {"clock": "ManualClock for lifecycle; random store/operation IDs remain", "rif": "off", "graphDepth": 0,
                     "storageFilesystem": "container /tmp tmpfs; CPU/logic control, not persistent-disk ingestion measurement",
                     "recordAccess": "false through production recorder/admission",
                     "ingestion": "canonical InsertMemory receipts then production startup/completion; internal Rust, not public transport",
                     "engine": "real QueryEngine and production storage/vector/FTS/entity/cache/graph/RIF adapters",
                     "store": "fresh temporary root per process; no snapshot/restore claim"},
        "performanceGate": {"established": False, "absoluteBudgets": None,
                            "reason": "baseline characterization; no paired candidate, five-seed run or promotion gate"},
    }
    save(output / "diagnostic-run.json", manifest)
    contract, actor_bytes, evaluator_bytes = evaluation_inputs(args, manifest)
    save(output / "manifest.json", contract)  # immutable input, never amended with result hashes
    (output / "actor-inputs.json").write_bytes(actor_bytes)
    (output / "evaluator-labels.json").write_bytes(evaluator_bytes)
    # Execute the exact snapshotted script. No shell interpolation of host input.
    inner = r'''set -eu
mkdir -p /tmp/home
cd /work/source
rustc -Vv > /work/output/rustc.txt
uname -a > /work/output/kernel.txt
cat /proc/cpuinfo > /work/output/cpu.txt
cat /proc/meminfo > /work/output/memory.txt
df -T /work > /work/output/filesystem.txt
df -T /tmp > /work/output/storage-filesystem.txt
cat /sys/fs/cgroup/memory.max > /work/output/cgroup-memory.txt
cat /sys/fs/cgroup/cpu.max > /work/output/cgroup-cpu.txt
python3 scripts/test_offline_evaluation.py > /work/output/runner-tests.log 2>&1
cargo fmt --all --check > /work/output/format.log 2>&1
cargo clean -p recalld > /work/output/package-clean.log 2>&1
cargo test --locked --offline --all-features --lib --bins --tests --examples -j1 > /work/output/library-tests.log 2>&1
cargo test --locked --offline --example offline_eval > /work/output/harness-tests.log 2>&1
cargo clippy --locked --offline --all-features --all-targets > /work/output/clippy.log 2>&1
cargo build --locked --offline --release --example offline_eval -j1 > /work/output/build.log 2>&1
python3 scripts/check-evaluation-contract.py --binary /work/target/release/examples/offline_eval --manifest /work/output/manifest.json --output /work/output/manifest-rejections.json > /work/output/manifest-tests.log 2>&1
'''
    (scratch / "build.sh").write_text(inner)
    docker = ["docker", "run", "--rm", "--network", "none", "--cpus", "2", "--memory", "4g",
              "--memory-swap", "4g", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
              "--read-only", "--tmpfs", "/tmp:rw,nosuid,nodev", "--mount", f"type=bind,src={scratch},dst=/work",
              "--workdir", "/work/source", "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/work/cargo",
              "--env", "CARGO_TARGET_DIR=/work/target", "--env", "CARGO_BUILD_JOBS=2",
              "--env", f"RUSTUP_TOOLCHAIN={args.toolchain}",
              "--env", f"RECALLD_A6_SOURCE_COMMIT={contract['sourceCommit']}",
              "--env", f"RECALLD_A6_DIRTY_PATCH_SHA256={contract['dirtyPatchSha256']}",
              "--env", f"RECALLD_A6_IMAGE_ID={contract['containerImageId']}",
              "--env", f"RECALLD_A6_SOURCE_TREE_SHA256={contract['sourceTreeSha256']}", image["Id"]]
    try:
        subprocess.run(docker + ["bash", "/work/build.sh"], check=True)
        manifest["executableSha256"] = sha(scratch / "target/release/examples/offline_eval")
        for run in range(args.runs):
            cmd = docker + ["/work/target/release/examples/offline_eval", "--manifest", "/work/output/manifest.json",
                            "--out", f"/work/output/run-{run}"]
            with (output / f"run-{run}.log").open("w") as log:
                subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, check=True)
        stable = verify_replays(output, args.runs)
        verify_manifest_outputs(output, args.runs)
        manifest["status"] = "passed"
        manifest["replay"] = {"processes": args.runs, "identicalArtifacts": stable}
    except BaseException as exc:
        manifest["status"] = "failed"
        manifest["error"] = str(exc)
        raise
    finally:
        manifest["artifactsSha256"] = {str(p.relative_to(output)): sha(p) for p in sorted(output.rglob("*"))
                                       if p.is_file() and p.name != "diagnostic-run.json"}
        save(output / "diagnostic-run.json", manifest)
        print(f"{manifest['status']}: {output}", flush=True)


if __name__ == "__main__":
    main()
