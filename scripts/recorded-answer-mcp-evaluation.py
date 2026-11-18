"""Recorded ANSWER consumption in the existing combined public runner."""
import json
import math
from pathlib import Path
import runpy
import subprocess
import sys
import time

ROLE = runpy.run_path(str(Path(__file__).with_name("recorded_answer.py")))
TRACK = "serialized-mcp-recorded-answer-synthetic-diagnostic-v1"
VARIANTS = ("baseline", "gold", "future", "middle-miss", "refusal", "parse", "utf8", "citation")
FIXTURE_SHA = "7886bd8908b1b59d6e02aa47be66e7aec9ffba15b5aae3388e0bb830b19efe64"


def fixture():
    raw = Path(__file__).with_name("fixtures").joinpath("recorded-answer.json").read_bytes()
    if ROLE["digest"](raw) != FIXTURE_SHA: raise ValueError("frozen recorded fixture mismatch")
    return ROLE["decode"](raw)


def policy():
    return dict(roleTraceSchemaVersion=1, roleInputVersion=1, roleOutputVersion=1, recordedAnswerConsumerVersion=1,
                configuration=fixture()["roleConfig"], fixtureSha256=FIXTURE_SHA,
                trace="separate-role-trace-v1", modelTokenIdentity=None, answerQuality=None, productionCost=None)


def cache(episode, variant):
    frozen = fixture(); configuration = frozen["roleConfig"]; entries = []
    for row in frozen["records"]:
        if row["episodeId"] != episode: continue
        value = ROLE["role_input"](row["currentQuestion"], row["context"])
        qid = value["currentQuestion"]["queryId"]
        if variant == "middle-miss" and episode == "episode-3" and qid == "q0": continue
        raw = bytes.fromhex(row["outputHex"])
        if ROLE["digest"](raw) != row["outputSha256"]: raise ValueError("frozen output hash mismatch")
        if episode == "episode-0" and qid == "q0":
            if variant == "refusal": raw = ROLE["encoded"](dict(roleOutputVersion=1, status="refused", text="Recorded diagnostic refusal", citations=[]))
            if variant == "parse": raw = b'{"roleOutputVersion":'
            if variant == "utf8": raw = b'\xff'
            if variant == "citation":
                output = ROLE["decode"](raw); output["citations"][0]["memoryId"] = "00000000-0000-4000-8000-000000000001"
                raw = ROLE["encoded"](output)
        entries.append(dict(queryId=qid, input=value, inputSha256=ROLE["digest"](ROLE["encoded"](value)),
            key=ROLE["key"](configuration, value), outputHex=raw.hex(), outputSha256=ROLE["digest"](raw)))
    return dict(cacheVersion=1, role="ANSWER", roleConfigSha256=ROLE["digest"](ROLE["encoded"](configuration)), entries=entries)


def validate_policy(manifest, h):
    if h["encoded"](manifest["answerPolicy"]) != h["encoded"](policy()) or manifest["answerVariant"] not in VARIANTS:
        raise ValueError("unsupported recorded answer policy")
    expected_variant = manifest["answerVariant"] if manifest["answerVariant"] in ("gold", "future") else "baseline"
    if manifest["combinedVariant"] != expected_variant or manifest["contextPolicy"] != policy()["configuration"]["contextPolicy"]:
        raise ValueError("recorded fixture/context mismatch")
    configuration = policy()["configuration"]
    if manifest["activeEmbedding"] != configuration["activeEmbedding"]:
        raise ValueError("recorded embedding profile mismatch")
    runtime = {k: v for k, v in manifest["sourceFilesSha256"].items() if k.startswith("src/") and k.endswith(".rs") or k in ("Cargo.toml", "Cargo.lock")}
    if h["digest"](h["encoded"](runtime)) != configuration["runtimeSourcesSha256"] or manifest["executables"]["ownedEvaluator"] != configuration["ownedEvaluatorSha256"]:
        raise ValueError("recorded role runtime differs; no rekeying")


def validate_cache(actual, episode, variant, h):
    if h["encoded"](actual) != h["encoded"](cache(episode, variant)):
        raise ValueError("recorded cache differs from frozen inputs/outputs")
    if len({e["key"] for e in actual["entries"]}) != len(actual["entries"]):
        raise ValueError("duplicate recorded role key")


def step(args, h, manifest, question, ordinal, wire, root, actor_path, control_fds, public_fds):
    """Called only after this query's public ACK and same-core barrier."""
    qid = question["queryId"]
    public_rows = [r for r in wire if r["event"] == "outcome" and r["tag"] == "query:" + qid and r["deliveryOrdinal"] == ordinal]
    context_rows = [r for r in wire if r["event"] == "context" and r["tag"] == "context:" + qid and r["deliveryOrdinal"] == ordinal]
    if len(public_rows) != 1 or len(context_rows) != 1: raise ValueError("missing current public/context outcome")
    response = public_rows[0]["response"]
    if response.get("error") or response.get("result", {}).get("isError"): raise ValueError("answer after failed query")
    if context_rows[0]["stepId"] != public_rows[0]["stepId"]: raise ValueError("context/query correlation mismatch")
    public = json.loads(response["result"]["content"][0]["text"])["memories"]
    token = h["CONTEXT"]["TOKEN"]
    context = token["verify"](token["load"](args.context_assets), question["text"], public, manifest["contextPolicy"], context_rows[0]["context"])
    value = ROLE["role_input"](question, context); configuration = manifest["answerPolicy"]["configuration"]
    key = ROLE["key"](configuration, value); group = manifest["groupContext"]
    dest = args.out / "evaluator"; trace_path = dest / "role-trace.jsonl"
    previous = h["rows"](trace_path) if trace_path.exists() else []
    common = dict(roleTraceSchemaVersion=1, runId=manifest["answerEnvironment"], environmentHash=manifest["answerEnvironment"],
        episodeId=group["episodeId"], groupId=group["groupId"], queryId=qid, deliveryOrdinal=ordinal,
        publicStepId=public_rows[0]["stepId"], role="ANSWER", roleConfigSha256=ROLE["digest"](ROLE["encoded"](configuration)),
        inputSha256=ROLE["digest"](ROLE["encoded"](value)), key=key,
        contextSha256=h["digest"](h["encoded"](context)), cacheArtifactSha256=manifest["roleCache"]["sha256"])
    started = time.monotonic(); outcome = None
    with trace_path.open("a") as trace:
        def emit(kind, **fields):
            trace.write(json.dumps(dict(common, sequence=str(len(previous) + (1 if kind == "attempt" else 2)), eventKind=kind, **fields)) + "\n")
            trace.flush()
        emit("attempt")  # Before consuming output; no remote mutation/durability claim.
        try:
            entries = [e for e in manifest["answerCache"]["entries"] if e["key"] == key and e["queryId"] == qid]
            if len(entries) != 1:
                h["save"](dest / f"answer-{ordinal}-miss.json", dict(actualInput=value, key=key, expectedKeys=[e["key"] for e in manifest["answerCache"]["entries"]]))
                raise ValueError("recorded-answer-cache-miss")
            entry = entries[0]
            if entry["input"] != value: raise ValueError("recorded input collision")
            envelope = dict(role="ANSWER", roleConfig=configuration, input=value, inputSha256=common["inputSha256"],
                key=key, outputHex=entry["outputHex"], outputSha256=entry["outputSha256"])
            h["save"](args.out / "actor-inputs" / f"answer-{ordinal}.json", envelope)
            raw_path = dest / f"answer-{ordinal}.bin"; raw_path.write_bytes(bytes.fromhex(entry["outputHex"]))
            command = [sys.executable, str(actor_path), "--recorded-answer", *[str(fd) for fd in (*control_fds, *public_fds)]]
            with (dest / f"answer-{ordinal}-actor.log").open("wb") as error_log:
                child = subprocess.run(command, input=ROLE["encoded"](envelope), stdout=subprocess.PIPE, stderr=error_log,
                    cwd=root, env=dict(PATH="/usr/bin:/bin", HOME=str(root / "home"), TZ="UTC", PYTHONDONTWRITEBYTECODE="1", PYTHONNOUSERSITE="1"),
                    close_fds=True, timeout=20)
            (args.out / f"answer-{ordinal}-actor.json").write_bytes(child.stdout)
            if child.returncode != 0: raise ValueError("recorded-answer-actor-failed")
            actor = ROLE["decode"](child.stdout)
            h["exact_keys"](actor, ("event", "isolation", "result"))
            if actor["event"] != "recorded-answer" or actor["isolation"] != dict(inheritedDescriptorsClosed=True, checked=len(control_fds) + len(public_fds)):
                raise ValueError("answer actor isolation evidence mismatch")
            expected = ROLE["consume"](envelope)
            if actor["result"] != expected: raise ValueError("actor consumed output mismatch")
            outcome = expected
            if expected["status"] != "accepted": raise ValueError(expected["reason"])
            ROLE["verify_citations"](expected["output"], public, context)
            emit("accepted", outputSha256=entry["outputSha256"], outputBytes=str(len(raw_path.read_bytes())),
                elapsedSeconds=time.monotonic() - started, outcome=expected, cacheStatus="hit")
        except BaseException as error:
            interrupted = isinstance(error, (subprocess.TimeoutExpired, KeyboardInterrupt))
            if isinstance(error, subprocess.TimeoutExpired):
                (dest / f"answer-{ordinal}-partial.bin").write_bytes(error.output or b"")
            emit("incomplete" if interrupted else "rejected", reason=str(error), outcome=outcome, elapsedSeconds=time.monotonic() - started,
                cacheStatus="miss" if str(error) == "recorded-answer-cache-miss" else "not-accepted")
            raise


def reconcile(manifest, role_rows, trace_rows, environment):
    """Bind complete role outcomes to independently recorded query/context events."""
    expected = manifest["groupContext"]; qids = expected["queryIds"]
    contexts = {r["stepId"]: r for r in trace_rows if r["eventKind"] == "context"}
    if len(role_rows) != 2 * len(qids): raise ValueError("incomplete answer roster")
    seen = set()
    for i in range(0, len(role_rows), 2):
        attempt, outcome = role_rows[i:i+2]
        common_fields = ("roleTraceSchemaVersion", "runId", "environmentHash", "episodeId", "groupId", "queryId",
            "deliveryOrdinal", "publicStepId", "role", "roleConfigSha256", "inputSha256", "key", "contextSha256", "cacheArtifactSha256", "sequence", "eventKind")
        ROLE["exact"](attempt, common_fields)
        ROLE["exact"](outcome, (*common_fields, "outputSha256", "outputBytes", "elapsedSeconds", "outcome", "cacheStatus"))
        for offset, row in enumerate((attempt, outcome), i + 1):
            if (row["roleTraceSchemaVersion"] != 1 or type(row["roleTraceSchemaVersion"]) is not int or row["sequence"] != str(offset) or
                row["runId"] != environment or row["environmentHash"] != environment or row["episodeId"] != expected["episodeId"] or
                row["groupId"] != expected["groupId"] or row["role"] != "ANSWER"):
                raise ValueError("role trace identity/version mismatch")
        if attempt["eventKind"] != "attempt" or outcome["eventKind"] != "accepted" or outcome["cacheStatus"] != "hit":
            raise ValueError("role outcome unavailable")
        for field in attempt:
            if field not in ("sequence", "eventKind") and attempt[field] != outcome[field]:
                raise ValueError("role attempt/outcome mismatch")
        qid = attempt["queryId"]; context = contexts.get(attempt["publicStepId"])
        if (qid not in qids or qid in seen or context is None or context["deliveryOrdinal"] != attempt["deliveryOrdinal"] or
            ROLE["digest"](ROLE["encoded"](context["observation"])) != attempt["contextSha256"]):
            raise ValueError("role context correlation mismatch")
        entry = next((e for e in manifest["answerCache"]["entries"] if e["queryId"] == qid), None)
        if entry is None or any(attempt[k] != entry[k] for k in ("key", "inputSha256")) or outcome["outputSha256"] != entry["outputSha256"]:
            raise ValueError("role cache provenance mismatch")
        if attempt["cacheArtifactSha256"] != manifest["roleCache"]["sha256"] or attempt["roleConfigSha256"] != manifest["answerCache"]["roleConfigSha256"]:
            raise ValueError("role config/artifact mismatch")
        configuration = manifest["answerPolicy"]["configuration"]
        actual = ROLE["role_input"](entry["input"]["currentQuestion"], context["observation"])
        envelope = dict(role="ANSWER", roleConfig=configuration, **{k:v for k,v in entry.items() if k != "queryId"})
        if (actual != entry["input"] or attempt["deliveryOrdinal"] != qids.index(qid) + 1 or
            outcome["outcome"] != ROLE["consume"](envelope) or outcome["outputBytes"] != str(len(bytes.fromhex(entry["outputHex"]))) or
            type(outcome["elapsedSeconds"]) not in (float, int) or not math.isfinite(outcome["elapsedSeconds"]) or outcome["elapsedSeconds"] < 0):
            raise ValueError("role consumed result/context mismatch")
        seen.add(qid)
    return dict(status="passed", planned=len(qids), attempted=len(qids), accepted=len(qids), rejected=0, unattempted=0,
                answerQuality=None, modelTokenIdentity=None, productionCost=None, scope="synthetic-recorded-quotation-provenance-only")


def finish(args, h, manifest, environment):
    role_rows = h["rows"](args.out / "evaluator/role-trace.jsonl")
    result = reconcile(manifest, role_rows, h["rows"](args.out / "trace.jsonl"), environment)
    result["artifacts"] = {name: h["digest"]((args.out / name).read_bytes()) for name in ("trace.jsonl", "evaluator/role-trace.jsonl")}
    h["save"](args.out / "evaluator/recorded-answer-proof.json", result)


def status(args, h, manifest):
    path = args.out / "evaluator/role-trace.jsonl"
    rows = h["rows"](path) if path.exists() else []
    roster = []
    for qid in manifest["groupContext"]["queryIds"]:
        events = [r for r in rows if r["queryId"] == qid]
        state = "unattempted" if not events else "unknown" if events[-1]["eventKind"] in ("attempt", "incomplete") else events[-1]["eventKind"]
        roster.append(dict(queryId=qid, status=state))
    h["save"](args.out / "evaluator/role-status.json", dict(planned=len(roster), queries=roster,
        **{name: sum(r["status"] == name for r in roster) for name in ("accepted", "rejected", "unknown", "unattempted")}))


def run(args, h):
    prepared = h["COMBINED"]["validate"](args, h, answer=True)
    h["COMBINED"]["run"](args, h, prepared=prepared)
    results = [json.loads((args.out / "episodes" / row["episodeId"] / "evaluator/recorded-answer-proof.json").read_text()) for row in prepared[2]]
    h["save"](args.out / "recorded-answer-summary.json", dict(status="passed", episodes=len(results), answers=sum(r["accepted"] for r in results),
        roleTraceSchemaVersion=1, traceSchemaVersion=2, answerQuality=None, modelTokenIdentity=None, productionCost=None,
        scope="synthetic-recorded-quotation-provenance-only"))
