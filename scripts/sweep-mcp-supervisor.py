"""Actual PR128 sweep consumer; evaluator-only schedule, existing reset child owner."""
import copy
import hashlib
import json
import subprocess

TRACK = "serialized-owned-sweep-synthetic-diagnostic-v1"
ADVANCE_MILLIS = 36_500 * 86_400_000


def control_capabilities(reset):
    value = copy.deepcopy(reset)
    value["methods"] += ["sweep", "sweepStatus"]
    value["sweepControlVersion"] = 1
    value["guarantees"].update(explicitLegacySweep=True, nativeSweep=False)
    return value


def fixtures(base):
    actor, labels, cache, _ = base()
    source = actor["sources"][0]
    source.update(summary="chronological-sweep", fullText="one retained payload", tags=["sweep-payload-canary"])
    source["contentSha256"] = hashlib.sha256(source["summary"].encode()).hexdigest()
    source["fullTextSha256"] = hashlib.sha256(source["fullText"].encode()).hexdigest()
    actor["sources"] = [source]
    canary = "FUTURE_SWEEP_QUERY"
    questions = [dict(queryId=q, text="chronological-sweep" + (" " + canary if q == "deleted" else ""))
                 for q in ("full", "summary", "ghost", "deleted")]
    cache["entries"] = {"a6-document:chronological-sweep\n\none retained payload sweep-payload-canary": [1., 0., 0., 0.],
                        "a6-document:blocked-worker-canary": [0., 0., 0., 1.],
                        "a6-query:chronological-sweep": [1., 0., 0., 0.],
                        "a6-query:chronological-sweep " + canary: [1., 0., 0., 0.]}
    labels["relevant"] = {"full": ["turn-0"], "summary": ["turn-0"], "ghost": []}
    steps = []
    for question in questions:
        phase = question["queryId"]
        if phase == "summary":
            steps.append(dict(control="advance", params=dict(deltaMs=ADVANCE_MILLIS, deadlineMs=30000)))
        if phase != "full":
            steps.append(dict(control="sweep", params=dict(deadlineMs=30000), expectedPhase=phase))
        steps.extend([
            dict(tag="get:" + phase, tool="get_memory", arguments=dict(id="$turn-0", namespace="default", recordAccess=False)),
            dict(tag="query:" + phase, tool="recall_memories", arguments=dict(query=question["text"], namespace="default",
                 recordAccess=False, depth=0, limit=1, compact=False))])
    steps += [dict(tag="invalid-quality", tool="reinforce_memory", arguments=dict(id="$turn-0", namespace="default", quality=0)),
              dict(tag="replay-miss", tool="recall_memories", arguments=dict(query="unrecorded-query", namespace="default",
                   recordAccess=False, depth=0, limit=1, compact=False))]
    return actor, labels, cache, dict(scheduleVersion=1, queries=questions, futureQueryCanary=canary,
        warmQuestion=dict(queryId="warm", text="chronological-sweep"), steps=steps)


def suffix(owner, mappings, h, args, manifest, actor, labels, schedule, environment, runtime, evaluator, server, reset_cache, allow):
    """Replay actual controls/public actions; never calculate decay or clone a store."""
    reset_cache()
    first_request = len(h["rows"](server.log))
    # Request IDs are caller-controlled correlation only. Use the same unused
    # range for every actual suffix so complete reports compare without edits.
    assert owner.control.serial < 1000
    owner.control.serial = 1000
    assert owner.control.call("sweepStatus", {}) == dict(episodeInvalidated=False, lastSweep=None)
    initial_state, _ = owner.ready()
    before_invalid = initial_state
    malformed = [{}, {"deadlineMs": 0}, {"deadlineMs": 30001}, {"deadlineMs": True},
                 {"deadlineMs": 1.5}, {"deadlineMs": 100, "extra": True}]
    for params in malformed:
        owner.control.call("sweep", params, error="INVALID_PARAMS")
    owner.control.call("sweepStatus", {"extra": True}, error="INVALID_PARAMS")
    assert owner.ready()[0] == before_invalid
    assert owner.control.call("sweepStatus", {}) == dict(episodeInvalidated=False, lastSweep=None)
    outputs, receipts, reports, queries, acks = [], [], [], [], []
    start_id = 100
    now = initial_state["runtimeState"]["nowMillis"]
    for index, step in enumerate(schedule["steps"]):
        ordinal = 100 + index
        server.delivery = ordinal
        if "control" in step:
            method = step["control"]
            # Retain attempts/responses in Control's wire log. On an uncertain
            # sweep response inspect status, fail the episode, and never resend.
            try:
                value = owner.control.call(method, step["params"])
            except BaseException:
                if method == "sweep":
                    try:
                        h["save"](owner.output / "failed-sweep-status.json", owner.control.call("sweepStatus", {}))
                    except Exception as error:
                        h["save"](owner.output / "failed-sweep-status.json", dict(statusUnavailable=str(error)))
                raise
            if method == "advance":
                now += ADVANCE_MILLIS
                assert value == dict(nowMillis=now)
                continue
            # Save the full report before checking it, including every available
            # known receipt/error/uncertain identity. No aggregate receipt oracle.
            reports.append(value)
            h["save"](owner.output / "sweep-reports.json", reports)
            h["exact_keys"](value, ("controlRequestId", "sampledAtMillis", "phase", "executionCompleted", "readiness", "result"))
            assert value["controlRequestId"] == owner.control.serial
            assert value["sampledAtMillis"] == now and value["phase"] == "completed" and value["executionCompleted"] is True
            assert owner.control.call("sweepStatus", {}) == dict(episodeInvalidated=False, lastSweep=value)
            state, ready = owner.ready()
            assert value["readiness"] == ready
            assert ready["committedGeneration"] == state["committed"]["generation"]
            result = value["result"]
            assert result["errors"] == []
            assert all(result[key] is None for key in ("publicationError", "knownCommittedReceipt", "knownCommitError", "uncertainOperation"))
            kind = {"summary": "fullToSummary", "ghost": "summaryToGhost", "deleted": "deletions"}[step["expectedPhase"]]
            assert {key: result[key] for key in ("fullToSummary", "summaryToGhost", "deletions")} == {
                key: int(key == kind) for key in ("fullToSummary", "summaryToGhost", "deletions")}
            assert result["memoriesScanned"] == 1
            assert result["compactionTriggered"] is (step["expectedPhase"] != "ghost")
            assert len(result["transitions"]) == 1 and result["transitions"][0]["id"] == mappings["turn-0"]
            continue
        projection = h["CONFORMANCE"]["projection"](step, mappings, start_id, ordinal)
        if "query" in step["arguments"]:
            allow(step["arguments"]["query"])
        before, _ = owner.ready()
        ack, _ = owner.step(projection)
        assert int(ack["stepId"]) > start_id
        start_id = int(ack["stepId"])
        tool = ack["response"]["result"]
        expected_error = step["tag"] in ("get:deleted", "invalid-quality", "replay-miss")
        assert tool.get("isError", False) == expected_error, (step["tag"], tool)
        outputs.append(dict(tag=step["tag"], result=tool))
        acks.append(ack)
        state, ready = owner.ready()
        assert state == before, "false read or rejected action changed canonical/runtime state"
        receipts.append(dict(deliveryOrdinal=ordinal, state=state, readiness=ready))
        h["save"](owner.output / "step-receipts.json", receipts)
        if expected_error:
            continue
        payload = json.loads(tool["content"][0]["text"])
        phase = step["tag"].split(":", 1)[1]
        if step["tag"].startswith("get:"):
            assert payload["id"] == mappings["turn-0"] and payload["phase"] == phase.capitalize()
            assert payload["summary"] == ("" if phase == "ghost" else actor["sources"][0]["summary"])
            assert payload.get("fullText") == (actor["sources"][0]["fullText"] if phase == "full" else None)
            if phase == "full":
                assert payload["tags"] == actor["sources"][0]["tags"]
        else:
            returned = payload["memories"] + payload.get("graphContext", {}).get("neighbors", [])
            assert all(row["id"] == mappings["turn-0"] and row["namespace"] == "default" for row in returned)
            if phase in ("ghost", "deleted"):
                assert returned == [], "erased memory returned by recall"
            if phase == "full":
                assert [row["id"] for row in returned] == [mappings["turn-0"]]
            queries.append(dict(queryId=phase, memoryIds=[row["id"] for row in returned]))
    state, ready = owner.ready()
    checkpoint = h["RESET"]["checked_snapshot"](h, owner.control.call("snapshot", dict(deadlineMs=30000)), runtime)
    assert checkpoint["committed"] == state["committed"]
    provider = h["rows"](server.log)[first_request:]
    assert sum(row["status"] == 400 for row in provider) == 1
    events = "".join(json.dumps(dict(sourceId=source, status=status, reason="acknowledged-checkpoint-prefix",
        **(dict(memoryId=memory) if status == "accepted" else {}))) + "\n"
        for source, memory in mappings.items() for status in ("seen", "attempted", "accepted"))
    (owner.output / "source-events.jsonl").write_text(events)
    score = dict(profile=manifest["scoringProfile"], environmentHash=environment,
        sourceIds=[s["sourceId"] for s in actor["sources"]], mappings={memory: [source] for source, memory in mappings.items()},
        relevant=labels["relevant"], queries=queries, sourceEvents=events, sourceEventsSha256=h["digest"](events.encode()))
    h["save"](owner.output / "score-input.json", score)
    subprocess.run([str(args.scorer), "--input", str(owner.output / "score-input.json"), "--out", str(owner.output / "scores.json")], check=True, env=dict(PATH="/usr/bin:/bin"))
    h["save"](owner.output / "source-mappings.json", dict(sourceMappingVersion=1, actorArtifactSha256=manifest["actorInputs"]["sha256"], mappings=score["mappings"], sources=actor["sources"]))
    h["save"](owner.output / "prefix-provenance.json", dict(prefixArtifact="../prefix-mappings.json", prefixArtifactSha256=h["digest"]((evaluator / "prefix-mappings.json").read_bytes())))
    with (owner.output / "trace.jsonl").open("w") as trace:
        for ack, receipt in zip(acks, receipts):
            trace.write(json.dumps(dict(traceSchemaVersion=1, runId=environment, episodeId=owner.output.name, groupId="synthetic-regression",
                stepId=ack["stepId"], environmentHash=environment, action=ack["request"], outcome=ack["response"], readGeneration=None,
                readiness=dict(available=True, scope="post-step-barrier-not-response-readview", receiptFile="step-receipts.json", deliveryOrdinal=receipt["deliveryOrdinal"]),
                simulatedTimeMs=None, snapshot=None, reward=None)) + "\n")
    wire = h["rows"](owner.output / "actor-wire.jsonl")
    canary = schedule["futureQueryCanary"]
    first = next(100+i for i, step in enumerate(schedule["steps"]) if canary in json.dumps(step))
    assert all(canary not in json.dumps(row) for row in wire + provider if row["deliveryOrdinal"] < first)
    assert canary not in (runtime / "actor.py").read_text()
    for path in (owner.output / "actor-inputs").glob("*.json"):
        assert labels["labelCanary"] not in path.read_text()
    assert labels["labelCanary"] not in json.dumps(wire) + json.dumps(provider)
    assert len([row for row in wire if row["tag"] == "control-fds-closed"]) == len(list((owner.output / "actor-inputs").glob("*.json")))
    comparison = dict(outputs=outputs, mappings=mappings, state=state, readiness=ready, logicalHash=checkpoint["logicalHash"],
        sweepReports=reports, malformedControlsRejected=len(malformed)+1,
        provider=[dict(request=json.loads(row["requestBody"]), status=row["status"], vectors=row["responseVectors"]) for row in provider])
    h["save"](owner.output / "suffix.json", comparison)
    return comparison
