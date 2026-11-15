"""Explicit native manual sweep on the actual owned prefix; no public writers."""
import hashlib
import json
import subprocess

TRACK = "serialized-native-sweep-synthetic-diagnostic-v1"
CHECKPOINT_TRACK = "isolated-native-sweep-checkpoint-v1"
ADVANCE_MILLIS = 36_500 * 86_400_000


def fixtures(base, native, future=False):
    actor, labels, cache, old = native["fixtures"](base, future)
    # Real acknowledged migration inputs, not phase records or a decay oracle.
    for source in actor["sources"]:
        source.setdefault("fullText", "retained native payload " + source["sourceId"])
        source["fullTextSha256"] = hashlib.sha256(source["fullText"].encode()).hexdigest()
    questions = old["queries"]
    steps = [next(s for s in old["steps"] if s["tag"] == "foreign-query")]
    for phase, question in zip(("full", "summary", "ghost", "deleted"), questions):
        if phase == "summary":
            steps.append(dict(control="advance", params=dict(deltaMs=ADVANCE_MILLIS, deadlineMs=30000)))
        if phase != "full":
            steps.append(dict(control="sweep", params=dict(deadlineMs=30000), expectedPhase=phase))
        steps.extend([
            dict(tag="get:" + phase, tool="get_memory", arguments=dict(id="$turn-0", namespace="default", recordAccess=False)),
            dict(tag="query:" + question["queryId"], expectedPhase=phase, tool="recall_memories",
                 arguments=dict(query=question["text"], namespace="default", recordAccess=False, depth=0, limit=3, compact=False))])
    steps += [s for s in old["steps"] if s["tag"] in {"omitted-access", "true-access", "omitted-search-access", "true-search-access", "blocked-store", "blocked-feedback", "blocked-forget", "replay-miss"}]
    # Sparse annotations stay explicit: zero-label is unscored, q2 is unknown,
    # deleted q3 is still judged against its annotation rather than normalized.
    return actor, labels, cache, dict(old, steps=steps)


def capabilities(h):
    value = h["NATIVE"]["capabilities"](h)
    value.update(track=CHECKPOINT_TRACK, sweepControlVersion=1, sweepOriginalContractVersion=1,
                 nativePhysicalCompaction=False)
    value["methods"] += ["sweep", "sweepStatus"]
    value["guarantees"]["nativeSweep"] = True
    return value


def suffix(owner, mappings, h, args, manifest, actor, labels, schedule, cache, server, runtime, snapshot, provider_rows):
    server.entries = {}
    first_request = len(provider_rows())
    assert owner.control.serial < 1000
    owner.control.serial = 1000  # Unused caller correlation IDs, not canonical IDs.
    initial, _ = owner.ready()
    initial_hash = snapshot(owner)["logicalHash"]
    malformed = [{}, {"deadlineMs": 0}, {"deadlineMs": 30001}, {"deadlineMs": True},
                 {"deadlineMs": 1.5}, {"deadlineMs": 100, "extra": True}]
    for params in malformed:
        owner.control.call("sweep", params, error="INVALID_PARAMS")
    owner.control.call("sweepStatus", {"extra": True}, error="INVALID_PARAMS")
    assert owner.ready()[0] == initial
    assert owner.control.call("sweepStatus", {}) == dict(episodeInvalidated=False, lastSweep=None)
    now = initial["runtimeState"]["nowMillis"]
    outputs, reports, receipts, acks, queries, hashes = [], [], [], [], [], [initial_hash]
    start_id = 100
    for index, step in enumerate(schedule["steps"]):
        ordinal = 100 + index
        server.delivery = ordinal
        if "control" in step:
            before, _ = owner.ready()
            try:
                value = owner.control.call(step["control"], step["params"])
            except BaseException:
                if step["control"] == "sweep":
                    try:
                        h["save"](owner.output / "failed-sweep-status.json", owner.control.call("sweepStatus", {}))
                    except Exception as error:
                        h["save"](owner.output / "failed-sweep-status.json", dict(statusUnavailable=str(error)))
                raise  # Never retry an uncertain control.
            if step["control"] == "advance":
                now += ADVANCE_MILLIS
                assert value == dict(nowMillis=now)
                after, _ = owner.ready()
                assert after["committed"] == before["committed"]
                assert snapshot(owner)["logicalHash"] == initial_hash
                continue
            reports.append(value)
            h["save"](owner.output / "sweep-reports.json", reports)
            h["exact_keys"](value, ("controlRequestId", "sampledAtMillis", "phase", "executionCompleted", "readiness", "result"))
            assert value["controlRequestId"] == owner.control.serial
            assert value["sampledAtMillis"] == now and value["phase"] == "completed" and value["executionCompleted"] is True
            assert owner.control.call("sweepStatus", {}) == dict(episodeInvalidated=False, lastSweep=value)
            state, ready = owner.ready()
            assert value["readiness"] == ready
            result = value["result"]
            assert result["errors"] == []
            assert all(result[k] is None for k in ("publicationError", "knownCommittedReceipt", "knownCommitError", "uncertainOperation"))
            kind = {"summary": "fullToSummary", "ghost": "summaryToGhost", "deleted": "deletions"}[step["expectedPhase"]]
            assert {k: result[k] for k in ("fullToSummary", "summaryToGhost", "deletions")} == {k: 4 if k == kind else 0 for k in ("fullToSummary", "summaryToGhost", "deletions")}
            assert result["memoriesScanned"] == 4 and result["missingSummarySkipped"] == 0
            assert result["compactionTriggered"] is False
            assert {v["id"] for v in result["transitions"]} == set(mappings.values())
            assert all(v["toPhase"] == step["expectedPhase"] for v in result["transitions"])
            assert int(state["committed"]["generation"]) == int(before["committed"]["generation"]) + 4
            snap = snapshot(owner)
            assert snap["committed"] == state["committed"] and snap["logicalHash"] not in hashes
            hashes.append(snap["logicalHash"])
            h["save"](owner.output / "phase-hashes.json", hashes)
            continue
        projection = h["CONFORMANCE"]["projection"](step, mappings, start_id, ordinal)
        query = step["arguments"].get("query")
        if query is not None and "a6-query:" + query in cache["entries"]:
            key = "a6-query:" + query
            server.entries[server.key(key)] = cache["entries"][key]
        before, _ = owner.ready()
        before_calls = len(provider_rows())
        ack, _ = owner.step(projection)
        start_id = int(ack["stepId"])
        tool = ack["response"]["result"]
        outputs.append(dict(tag=step["tag"], result=tool)); acks.append(ack)
        h["save"](owner.output / "outputs.json", outputs)
        expected_error = step["tag"] in {"get:deleted", "omitted-access", "true-access", "omitted-search-access", "true-search-access", "blocked-store", "blocked-feedback", "blocked-forget", "replay-miss"}
        assert tool.get("isError", False) == expected_error, (step["tag"], tool)
        if expected_error and step["tag"] != "replay-miss":
            assert len(provider_rows()) == before_calls
        state, ready = owner.ready()
        assert state == before, "false or rejected public action changed canonical/runtime state"
        receipts.append(dict(deliveryOrdinal=ordinal, state=state, readiness=ready))
        h["save"](owner.output / "step-receipts.json", receipts)
        if expected_error:
            continue
        payload = json.loads(tool["content"][0]["text"])
        if step["tag"].startswith("get:"):
            phase = step["tag"].split(":")[1]
            assert payload["id"] == mappings["turn-0"] and payload["phase"] == phase.capitalize()
            assert payload["summary"] == ("" if phase == "ghost" else actor["sources"][0]["summary"])
            assert payload.get("fullText") == (actor["sources"][0]["fullText"] if phase == "full" else None)
        else:
            returned = payload["memories"] + payload.get("graphContext", {}).get("neighbors", [])
            namespace = step["arguments"]["namespace"]
            assert all(v["namespace"] == namespace for v in returned)
            if step["tag"] == "foreign-query":
                assert [v["id"] for v in returned] == [mappings["turn-3"]]
            else:
                assert mappings["turn-3"] not in {v["id"] for v in returned}
                if step["expectedPhase"] in ("ghost", "deleted"):
                    assert returned == [], "erased UUID/payload returned by recall"
                else:
                    assert returned, "phase query must be nonvacuous"
                queries.append(dict(queryId=step["tag"][6:], memoryIds=[v["id"] for v in returned]))
    final = snapshot(owner)
    state, ready = owner.ready()
    provider = provider_rows()[first_request:]
    assert sum(v["status"] == 400 for v in provider) == 1
    assert sum(v["status"] == 200 for v in provider) == 5
    events = "".join(json.dumps(dict(sourceId=s, status=status, reason="acknowledged-migrated-prefix", **(dict(memoryId=m) if status == "accepted" else {}))) + "\n" for s, m in mappings.items() for status in ("seen", "attempted", "accepted"))
    (owner.output / "source-events.jsonl").write_text(events)
    score = dict(profile=manifest["scoringProfile"], environmentHash=server.environment, sourceIds=[s["sourceId"] for s in actor["sources"]], mappings={m: [s] for s, m in mappings.items()}, relevant=labels["relevant"], queries=queries, sourceEvents=events, sourceEventsSha256=h["digest"](events.encode()))
    h["save"](owner.output / "score-input.json", score)
    subprocess.run([str(args.scorer), "--input", str(owner.output / "score-input.json"), "--out", str(owner.output / "scores.json")], check=True, env=dict(PATH="/usr/bin:/bin"))
    h["save"](owner.output / "source-mappings.json", dict(sourceMappingVersion=1, mappings=score["mappings"], prefixProvenanceSha256=h["digest"]((owner.output.parent / "original-provenance.json").read_bytes())))
    with (owner.output / "trace.jsonl").open("w") as trace:
        for ack, receipt in zip(acks, receipts):
            trace.write(json.dumps(dict(traceSchemaVersion=1, runId=server.environment, episodeId=owner.output.name, groupId="synthetic-regression", stepId=ack["stepId"], environmentHash=server.environment, action=ack["request"], outcome=ack["response"], readGeneration=None, readiness=dict(available=True, scope="post-step-barrier-not-response-readview", deliveryOrdinal=receipt["deliveryOrdinal"]), snapshot=None, reward=None)) + "\n")
    wire = h["rows"](owner.output / "actor-wire.jsonl")
    canary = schedule["futureQueryCanary"]
    first = next(100+i for i, s in enumerate(schedule["steps"]) if canary in json.dumps(s))
    assert all(canary not in json.dumps(v) for v in wire + provider if v["deliveryOrdinal"] < first)
    assert canary not in (runtime / "actor.py").read_text()
    assert labels["labelCanary"] not in json.dumps(wire) + json.dumps(provider)
    for path in (owner.output / "actor-inputs").glob("*.json"):
        assert labels["labelCanary"] not in path.read_text()
    comparison = dict(outputs=outputs, state=state, readiness=ready, logicalHash=final["logicalHash"], phaseHashes=hashes, sweepReports=reports, malformedControlsRejected=7,
        provider=[dict(request=json.loads(v["requestBody"]), status=v["status"], vectors=v["responseVectors"]) for v in provider])
    h["save"](owner.output / "suffix.json", comparison)
    return comparison
