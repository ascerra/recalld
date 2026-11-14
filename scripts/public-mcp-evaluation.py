#!/usr/bin/env python3
"""Actual synthetic MCP stdio episodes; evaluator controls deliberately unavailable."""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import runpy
import select
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import uuid

TRACK = "serialized-mcp-synthetic-diagnostic-v1"
MODEL = "a6-public-synthetic-v1"
ROOT = Path(__file__).resolve().parents[1]
CAPABILITIES = dict(publicMcp=True, publicV2=False, fullA6=False, coherentReadView=False,
                    barrier=False, reset=False, snapshot=False, clock="system-uncontrolled",
                    clockAdvance=False, deterministicRng=False, deterministicIds=False,
                    canonicalSchemaAttested=False)
VERSIONS = dict(publicContractVersion=1, canonicalSchemaVersion=None,
                canonicalRepresentation="unattested-via-public-v1", contextVersion=1)
EXTRA = ("scripts/public-mcp-evaluation.py", "scripts/public-mcp-actor.py", "scripts/test_public_mcp_evaluation.py",
         "src/storage/metadata/native_records/fixtures/phase-clock-legacy.jsonl")


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    Path(path).write_bytes(encoded(value))


def rows(path):
    return [json.loads(line) for line in Path(path).read_text().splitlines()]


def exact_keys(value, keys):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise ValueError("unsupported or missing fields")


def fixtures():
    sources = []
    for i, (text, namespace) in enumerate((("amber", "default"), ("birch", "default"), ("olive", "default"), ("foreign", "foreign"))):
        sources.append(dict(sourceId=f"turn-{i}", groupId="synthetic-regression", conversationId="synthetic-conversation",
                            sessionId="synthetic-session", turnId=f"turn-{i}", role="user", sourceTimestamp=1767225600000,
                            ordinal=i, summary=text, namespace=namespace, contentSha256=digest(text.encode())))
    actor = dict(adapterVersion=1, sources=sources)
    schedule = dict(scheduleVersion=1, queries=[dict(queryId=f"q{i}", text="amber" if i < 3 else "amber FUTURE_QUESTION_CANARY") for i in range(4)],
                    futureQueryCanary="FUTURE_QUESTION_CANARY")
    labels = dict(adapterVersion=1, annotationUnit="synthetic-unit", relevant={"q0": ["turn-0"], "q1": [], "q3": ["turn-0", "turn-3"]},
                  labelCanary="EVALUATOR_LABEL_CANARY_ONLY")
    cache = dict(model=MODEL, dimension=4, entries={"amber": [1., 0., 0., 0.], "birch": [0., 1., 0., 0.],
                 "olive": [0., 0., 1., 0.], "foreign": [1., 0., 0., 0.], "health check": [0., 0., 0., 1.]})
    cache["entries"] = {"a6-document:" + text: vector for text, vector in cache["entries"].items()}
    cache["entries"]["a6-query:amber"] = [1., 0., 0., 0.]
    cache["entries"]["a6-query:amber FUTURE_QUESTION_CANARY"] = [1., 0., 0., 0.]
    return actor, labels, cache, schedule


def validate(manifest_path, binary, scorer, source_root):
    # Complete input validation before creating episode output, provider, or store.
    raw = manifest_path.read_bytes()
    m = json.loads(raw)
    exact_keys(m, ("manifestVersion", "sourceMappingVersion", "traceSchemaVersion", "track", "versions", "capabilities",
                   "sourceCommit", "sourceFilesSha256", "containerImageId", "limits", "executables", "actorInputs",
                   "evaluatorLabels", "evaluatorSchedule", "replayCache", "activeEmbedding", "scoringProfile", "dataset"))
    if any(type(m[k]) is not int or m[k] != 1 for k in ("manifestVersion", "sourceMappingVersion", "traceSchemaVersion")):
        raise ValueError("unsupported independent harness version")
    if m["track"] != TRACK or encoded(m["versions"]) != encoded(VERSIONS) or encoded(m["capabilities"]) != encoded(CAPABILITIES):
        raise ValueError("unsupported public/schema/control requirements")
    if encoded(m["limits"]) != encoded(dict(cpus=2, memoryBytes=4294967296)) or m["scoringProfile"] != "synthetic-unique-source-v1":
        raise ValueError("unsupported resources/scorer")
    hex_value = lambda s, n: isinstance(s, str) and len(s) == n and all(c in "0123456789abcdef" for c in s)
    if not hex_value(m["sourceCommit"], 40) or not m["containerImageId"].startswith("sha256:") or not hex_value(m["containerImageId"][7:], 64):
        raise ValueError("unresolved source/image identity")
    for name, sha in m["sourceFilesSha256"].items():
        relative = Path(name)
        p = source_root / relative
        if relative.is_absolute() or ".." in relative.parts or p.is_symlink() or not p.is_file() or not p.resolve().is_relative_to(source_root.resolve()) or digest(p.read_bytes()) != sha:
            raise ValueError("source pin mismatch")
    required = {str(p.relative_to(source_root)) for folder in ("src", "tests", "examples", "benches") for p in (source_root / folder).rglob("*.rs")}
    required |= {"Cargo.toml", "Cargo.lock", *EXTRA}
    if not required <= m["sourceFilesSha256"].keys():
        raise ValueError("missing source pins")
    if m["executables"] != {"recalld": digest(binary.read_bytes()), "scorer": digest(scorer.read_bytes())}:
        raise ValueError("executable hash mismatch")
    artifacts = []
    for key, name in (("actorInputs", "actor-inputs.json"), ("evaluatorLabels", "evaluator-labels.json"), ("replayCache", "evaluator-cache.json"), ("evaluatorSchedule", "evaluator-schedule.json")):
        exact_keys(m[key], ("file", "sha256"))
        p = manifest_path.parent / name
        if m[key]["file"] != name or not p.is_file() or p.is_symlink() or digest(p.read_bytes()) != m[key]["sha256"]:
            raise ValueError("artifact hash/reference mismatch")
        artifacts.append(json.loads(p.read_text()))
    actor, labels, cache, schedule = artifacts
    expected_actor, _, expected_cache, expected_schedule = fixtures()
    if encoded(actor) != encoded(expected_actor) or encoded(cache) != encoded(expected_cache) or encoded(schedule) != encoded(expected_schedule):
        raise ValueError("unsupported synthetic actor/cache fixture")
    exact_keys(labels, ("adapterVersion", "annotationUnit", "relevant", "labelCanary"))
    if type(labels["adapterVersion"]) is not int or labels["adapterVersion"] != 1 or labels["annotationUnit"] != "synthetic-unit" or not isinstance(labels["labelCanary"], str) or not labels["labelCanary"]:
        raise ValueError("unsupported annotations")
    if not isinstance(labels["relevant"], dict) or not set(labels["relevant"]) <= {q["queryId"] for q in schedule["queries"]}:
        raise ValueError("unknown evaluator query")
    for values in labels["relevant"].values():
        if not isinstance(values, list) or any(v not in {s["sourceId"] for s in actor["sources"]} for v in values) or len(set(values)) != len(values):
            raise ValueError("unknown/duplicate source labels")
    if m["dataset"] != dict(kind="generated-synthetic", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
                            actorArtifactSha256=m["actorInputs"]["sha256"], intendedUse="contract-diagnostic-only"):
        raise ValueError("unsupported dataset claim")
    profile = json.loads(subprocess.check_output([str(scorer), "--profile"], env={"PATH": "/usr/bin:/bin"}))
    if (profile["sourceCommit"] != m["sourceCommit"] or profile["containerImageId"] != m["containerImageId"] or
            profile["sourceTreeSha256"] != digest(encoded(m["sourceFilesSha256"]))):
        raise ValueError("compiled provenance mismatch")
    if encoded(m["activeEmbedding"]) != encoded(profile["activeEmbedding"]):
        raise ValueError("configuration profile mismatch")
    return m, actor, labels, cache, schedule, digest(raw)


def prepare(args):
    args.out.mkdir()
    build = json.loads(args.build.read_text())
    actor, labels, cache, schedule = fixtures()
    m = dict(manifestVersion=1, sourceMappingVersion=1, traceSchemaVersion=1, track=TRACK, versions=VERSIONS,
             capabilities=CAPABILITIES, sourceCommit=build["sourceCommit"], sourceFilesSha256=build["sourceFilesSha256"],
             containerImageId=build["containerImageId"], limits=dict(cpus=2, memoryBytes=4294967296),
             executables=dict(recalld=digest(args.binary.read_bytes()), scorer=digest(args.scorer.read_bytes())),
             activeEmbedding=json.loads(subprocess.check_output([str(args.scorer), "--profile"]))["activeEmbedding"], scoringProfile="synthetic-unique-source-v1")
    for key, name, value in (("actorInputs", "actor-inputs.json", actor), ("evaluatorLabels", "evaluator-labels.json", labels), ("replayCache", "evaluator-cache.json", cache), ("evaluatorSchedule", "evaluator-schedule.json", schedule)):
        save(args.out / name, value)
        m[key] = dict(file=name, sha256=digest((args.out / name).read_bytes()))
    m["dataset"] = dict(kind="generated-synthetic", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
                        actorArtifactSha256=m["actorInputs"]["sha256"], intendedUse="contract-diagnostic-only")
    save(args.out / "manifest.json", m)




class Replay(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass
    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        status = 200
        try:
            request = json.loads(raw)
            if self.path != "/v1/embeddings" or request["model"] != MODEL or request["dimensions"] != 4 or request["encoding_format"] != "float":
                raise ValueError("unsupported profile")
            vectors = [self.server.entries[self.server.key(text)] for text in request["input"]]
            body = dict(data=[dict(index=i, embedding=v) for i, v in enumerate(vectors)], usage=dict(prompt_tokens=0, total_tokens=0))
        except (ValueError, KeyError, TypeError):
            status, body = 400, dict(error=dict(message="offline replay miss; no fallback"))
        with self.server.log.open("a") as log:
            log.write(json.dumps(dict(requestBody=raw.decode(), status=status, environmentHash=self.server.environment, deliveryOrdinal=self.server.delivery, cacheKeys=[self.server.key(t) for t in json.loads(raw).get("input", [])])) + "\n")
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def run_episode(args):
    m, actor, labels, cache, schedule, environment = validate(args.manifest, args.binary, args.scorer, args.source_root)
    args.out.mkdir()
    evaluator = args.out / "evaluator"
    evaluator.mkdir()
    ingest_projection = dict(phase="ingest", sources=actor["sources"], activeEmbedding=m["activeEmbedding"], startId=0, deliveryOrdinal=0)
    save(args.out / "actor-projection.json", ingest_projection)
    (args.out / "actor-inputs").mkdir()
    # Cache server receives only the actor cache projection, never evaluator labels.
    stub = http.server.HTTPServer(("127.0.0.1", 0), Replay)
    stub.log, stub.environment, stub.delivery = args.out / "actor-provider.jsonl", environment, 0
    stub.key = lambda text: digest(encoded(dict(environmentHash=environment, model=MODEL, dimension=4, prefixedInput=text)))
    stub.entries = {stub.key(text): vector for text, vector in cache["entries"].items() if text.startswith("a6-document:")}
    threading.Thread(target=stub.serve_forever, daemon=True).start()
    result = dict(status="running", environmentHash=environment, capabilities=CAPABILITIES)
    process = None
    runtime_failure = None
    guard_read, guard_write = os.pipe()
    try:
        with tempfile.TemporaryDirectory(prefix="a6-public-runtime-") as temporary:
            root = Path(temporary)
            (root / "home").mkdir()
            (root / "disabled-daemon").write_text("explicit direct fallback; not a directory\n")
            config = f'''timezone = "UTC"
[storage]
data_dir = "{root}/data"
[embedding]
provider = "openai"
model_name = "{MODEL}"
api_key_env = "A6_SYNTHETIC_KEY"
base_url = "http://127.0.0.1:{stub.server_address[1]}"
dimensions = 4
document_prefix = "a6-document:"
query_prefix = "a6-query:"
[decay]
disable_sweep = true
decay_rate_multiplier = 0.0
[graph]
autolink_enabled = false
[rif]
enabled = false
[cache]
warm_file_enabled = false
'''
            (root / "config.toml").write_text(config)
            (args.out / "config.toml").write_text(config)
            env = dict(PATH="/usr/local/bin:/usr/bin:/bin", HOME=str(root / "home"), TZ="UTC", RUST_LOG="info",
                       A6_SYNTHETIC_KEY="synthetic-fixture-only", RECALLD_DAEMON_SOCKET=str(root / "disabled-daemon/socket"))
            with (args.out / "runtime.log").open("wb") as runtime_log, (args.out / "actor-wire.jsonl").open("wb") as wire, (args.out / "actor.log").open("wb") as actor_log:
                process = subprocess.Popen([str(args.binary), "--config", str(root / "config.toml"), "--data-dir", str(root / "data"), "mcp"],
                                           cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=runtime_log, bufsize=0)
                actor_path = root / "actor.py"
                shutil.copyfile(Path(__file__).with_name("public-mcp-actor.py"), actor_path)
                shutil.copyfile(actor_path, args.out / "actor-code.py")
                result["actorCodeSha256"] = digest(actor_path.read_bytes())
                assert digest(actor_path.read_bytes()) == m["sourceFilesSha256"]["scripts/public-mcp-actor.py"]
                start_id = 0
                # The supervisor owns this schedule. Each actor process sees only its current question.
                for ordinal in range(len(schedule["queries"]) + 2):
                    stub.delivery = ordinal
                    if ordinal == 0:
                        projection = ingest_projection
                    else:
                        miss = ordinal == len(schedule["queries"]) + 1
                        question = dict(queryId="replay-miss", text="unrecorded-query") if miss else schedule["queries"][ordinal - 1]
                        if not miss:
                            text = "a6-query:" + question["text"]
                            # Load only the current frozen cache entry, never generate on a miss.
                            stub.entries[stub.key(text)] = cache["entries"][text]
                        projection = dict(phase="query", startId=start_id, deliveryOrdinal=ordinal,
                                          currentQuestion=question, expectedMiss=miss)
                    save(args.out / "actor-inputs" / f"step-{ordinal}.json", projection)
                    child = subprocess.run([sys.executable, str(actor_path), "--write-fd", str(process.stdin.fileno()), "--read-fd", str(process.stdout.fileno()), "--deny-fd", str(guard_read)],
                        input=encoded(projection), stdout=wire, stderr=actor_log, cwd=root,
                        env=dict(PATH="/usr/bin:/bin", HOME=str(root / "home"), TZ="UTC"),
                        pass_fds=(process.stdin.fileno(), process.stdout.fileno()), timeout=90)
                    wire.flush()
                    delivered = rows(args.out / "actor-wire.jsonl")
                    acknowledgments = [r for r in delivered if r["event"] == "outcome"]
                    if child.returncode != 0:
                        runtime_failure = f"actor exited {child.returncode} at delivery {ordinal}"
                        break
                    assert acknowledgments and int(acknowledgments[-1]["stepId"]) > start_id
                    start_id = int(acknowledgments[-1]["stepId"])
                    with (evaluator / "schedule-ledger.jsonl").open("a") as log:
                        log.write(json.dumps(dict(deliveryOrdinal=ordinal, acknowledgedThroughStepId=str(start_id))) + "\n")
                process.stdin.close()
                process.wait(timeout=20)
                if runtime_failure is None and process.returncode != 0:
                    runtime_failure = f"actor/runtime exited {child.returncode}/{process.returncode}"
            if "falling back to direct mode" not in (args.out / "runtime.log").read_text():
                raise RuntimeError("isolated direct fallback not observed")
        wire = rows(args.out / "actor-wire.jsonl")
        assert wire[0] == dict(event="isolation", tag="private-fd-closed", stepId="0", deliveryOrdinal=0, fdClosed=True)
        outcomes = [r for r in wire if r["event"] == "outcome"]
        mappings, events, queries = {}, [], []
        for row in wire:
            if row["tag"].startswith("store:"):
                source_id = row["tag"][6:]
                if row["event"] == "attempt":
                    events.extend([dict(sourceId=source_id, status=s, reason="public-mcp-raw-synthetic") for s in ("seen", "attempted")])
                else:
                    tool = row["response"]["result"]
                    if tool.get("isError"):
                        events.append(dict(sourceId=source_id, status="rejected", reason="public-tool-error"))
                    else:
                        memory = json.loads(tool["content"][0]["text"])["id"]
                        assert memory not in mappings
                        mappings[memory] = [source_id]
                        events.append(dict(sourceId=source_id, status="accepted", reason="public-success-no-readiness-claim", memoryId=memory))
            elif row["tag"].startswith("query:") and row["event"] == "outcome":
                value = json.loads(row["response"]["result"]["content"][0]["text"])
                queries.append(dict(queryId=row["tag"][6:], memoryIds=[r["id"] for r in value["memories"]]))
        event_text = "".join(json.dumps(e) + "\n" for e in events)
        (evaluator / "source-events.jsonl").write_text(event_text)
        save(args.out / "source-mappings.json", dict(sourceMappingVersion=1, actorArtifactSha256=m["actorInputs"]["sha256"], mappings=mappings, sources=actor["sources"]))
        with (args.out / "trace.jsonl").open("w") as trace:
            for row in outcomes:
                trace.write(json.dumps(dict(traceSchemaVersion=1, runId=environment, episodeId=args.out.name, groupId="synthetic-regression",
                    stepId=row["stepId"], environmentHash=environment, action=row["request"], outcome=row["response"],
                    readGeneration=None, readiness=dict(available=False), simulatedTimeMs=None, snapshot=None, reward=None)) + "\n")
        if runtime_failure:
            result.update(actorActions=len(outcomes), storeMappings=len(mappings))
            raise RuntimeError(runtime_failure)
        score_input = dict(profile=m["scoringProfile"], environmentHash=environment, sourceIds=[s["sourceId"] for s in actor["sources"]], mappings=mappings,
                           relevant=labels["relevant"], queries=queries, sourceEvents=event_text, sourceEventsSha256=digest(event_text.encode()))
        save(evaluator / "score-input.json", score_input)
        subprocess.run([str(args.scorer), "--input", str(evaluator / "score-input.json"), "--out", str(evaluator / "scores.json")], check=True)
        for name in ("actor-projection.json", "actor-wire.jsonl", "actor-provider.jsonl", "source-mappings.json", "trace.jsonl"):
            if labels["labelCanary"].encode() in (args.out / name).read_bytes():
                raise RuntimeError("evaluator canary leaked to actor channel")
        providers = rows(args.out / "actor-provider.jsonl")
        assert sum(r["status"] == 400 for r in providers) == 1
        assert any(r["tag"] == "replay-miss" and r["response"]["result"]["isError"] for r in outcomes)
        canary = schedule["futureQueryCanary"]
        assert canary not in (args.out / "actor-code.py").read_text()
        assert labels["labelCanary"] not in (args.out / "actor-code.py").read_text()
        for ordinal in range(4):
            assert canary not in (args.out / "actor-inputs" / f"step-{ordinal}.json").read_text()
        assert canary in (args.out / "actor-inputs/step-4.json").read_text()
        assert all(canary not in json.dumps(row) for row in wire if row["deliveryOrdinal"] < 4)
        assert all(canary not in json.dumps(row) for row in providers if row["deliveryOrdinal"] < 4)
        assert len(rows(evaluator / "schedule-ledger.jsonl")) == 6
        result.update(futureQueryCanaryWithheldUntilDelivery=True, status="passed", actorActions=len(outcomes), expectedProviderMisses=1, storeMappings=len(mappings))
    except BaseException as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        os.close(guard_read)
        os.close(guard_write)
        stub.shutdown()
        stub.server_close()
        result["artifactsSha256"] = {str(p.relative_to(args.out)): digest(p.read_bytes()) for p in sorted(args.out.rglob("*")) if p.is_file() and p.name != "result.json"}
        save(args.out / "result.json", result)


def host(args):
    cache = args.cache.resolve()
    target = args.build_cache.resolve()
    if not str(cache).startswith(("/tmp/recalld-", "/private/tmp/recalld-")) or not (cache / "registry").is_dir():
        raise ValueError("registry must be a prior recalld scratch cache")
    if target.name != "target" or not (str(target.parent).startswith(("/tmp/recalld-", "/private/tmp/recalld-")) or
            (target.parent.name.startswith(("recalld-evaluation-", "recalld-public-mcp-")) and target.parent.parent == Path(tempfile.gettempdir()).resolve())):
        raise ValueError("build cache must be a prior private recalld target")
    helper = runpy.run_path(str(ROOT / "scripts/offline-evaluation.py"))
    root = Path(tempfile.mkdtemp(prefix="recalld-public-mcp-"))
    source, output = root / "source", root / "output"
    source.mkdir(); output.mkdir()
    print(f"Artifacts: {output}", flush=True)
    pins = helper["snapshot_source"](ROOT, source, EXTRA)
    shutil.copytree(cache / "registry", root / "cargo/registry")
    shutil.copytree(target, root / "target")
    image = json.loads(subprocess.check_output(["docker", "image", "inspect", "recalld-ci:1.94"]))[0]["Id"]
    metadata = dict(sourceCommit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), sourceFilesSha256=pins,
                    containerImageId=image, status="running")
    if args.library_evidence:
        prior = json.loads((args.library_evidence / "build.json").read_text())
        library = lambda files: {k:v for k,v in files.items() if k.startswith("src/") or k in ("Cargo.toml", "Cargo.lock")}
        if library(prior["sourceFilesSha256"]) != library(pins):
            raise ValueError("prior library evidence belongs to different runtime source")
        import re
        inherited = {}
        for name in ("session-tests.log", "structured-tests.log", "performance-tests.log"):
            data = (args.library_evidence / name).read_bytes()
            if not re.search(rb"test result: ok\. [1-9][0-9]* passed; 0 failed", data):
                raise ValueError("prior focused library check did not pass")
            inherited[name] = digest(data)
        metadata["inheritedFocusedLibraryEvidence"] = dict(sourceCommit=prior["sourceCommit"], originalOutput=str(args.library_evidence), logsSha256=inherited)
    save(output / "build.json", metadata)
    script = '''set -eu
mkdir -p /tmp/home
cd /work/source
python3 scripts/test_public_mcp_evaluation.py > /work/output/python-tests.log 2>&1
cargo fmt --all --check > /work/output/fmt.log 2>&1
cargo test --offline --locked --lib session > /work/output/session-tests.log 2>&1
cargo test --offline --locked --lib structured > /work/output/structured-tests.log 2>&1
cargo test --offline --locked --lib performance > /work/output/performance-tests.log 2>&1
cargo test --offline --locked --examples > /work/output/example-tests.log 2>&1
cargo build --offline --locked --bin recalld --example score_mcp_episode > /work/output/build.log 2>&1
python3 scripts/public-mcp-evaluation.py prepare --binary /work/target/debug/recalld --scorer /work/target/debug/examples/score_mcp_episode --build /work/output/build.json --out /work/output/inputs
python3 scripts/public-mcp-evaluation.py run --binary /work/target/debug/recalld --scorer /work/target/debug/examples/score_mcp_episode --source-root /work/source --manifest /work/output/inputs/manifest.json --out /work/output/episode-0
'''
    if args.library_evidence:
        script = "\n".join(line for line in script.split("\n") if "cargo test --offline --locked --lib " not in line)
    (root / "build.sh").write_text(script)
    command = ["docker", "run", "--rm", "--network", "none", "--cpus", "2", "--memory", "4g", "--memory-swap", "4g", "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--read-only", "--tmpfs", "/tmp:rw,nosuid,nodev", "--mount", f"type=bind,src={root},dst=/work", "--workdir", "/work/source", "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/work/cargo", "--env", "CARGO_TARGET_DIR=/work/target", "--env", "CARGO_BUILD_JOBS=2", "--env", "RUSTUP_TOOLCHAIN=1.94.1", "--env", f"RECALLD_A6_SOURCE_COMMIT={metadata['sourceCommit']}", "--env", f"RECALLD_A6_SOURCE_TREE_SHA256={digest(encoded(pins))}", "--env", f"RECALLD_A6_IMAGE_ID={image}", image, "bash", "/work/build.sh"]
    try:
        with (output / "container.log").open("w") as log:
            subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)
        metadata["status"] = "passed"
    except BaseException as error:
        metadata.update(status="failed", error=str(error))
        raise
    finally:
        save(output / "build.json", metadata)
        print(f"{metadata['status']}: {output}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    p = sub.add_parser("prepare")
    for flag in ("binary", "scorer", "build", "out"): p.add_argument("--" + flag, type=Path, required=True)
    p = sub.add_parser("run")
    for flag in ("binary", "scorer", "source-root", "manifest", "out"): p.add_argument("--" + flag, type=Path, required=True)
    p = sub.add_parser("host")
    p.add_argument("--cache", type=Path, required=True); p.add_argument("--build-cache", type=Path, required=True); p.add_argument("--library-evidence", type=Path)
    args = parser.parse_args()
    {"prepare": prepare, "run": run_episode, "host": host}[args.mode](args)


if __name__ == "__main__":
    main()
