"""Evaluator-only consumer of frozen private controlVersion1. Never imported by actor."""
import copy
import hashlib
import json
import os
import select
import time
import uuid

TRACK = "serialized-owned-mcp-synthetic-diagnostic-v1"
CAPABILITIES = dict(controlVersion=1, methods=["capabilities", "inspect", "barrier"], publicContract="legacy-v1",
    guarantees=dict(barrier="owned-work-and-required-derived-generations", logicalHash=False, snapshot=False,
                    reset=False, controlledClock=False, controlledIds=False, controlledRng=False))


def fixtures(base):
    actor, labels, cache, schedule = base()
    linked = dict(actor["sources"][1], sourceId="turn-4", turnId="turn-4", ordinal=4, summary="linked neighbor")
    linked["contentSha256"] = hashlib.sha256(linked["summary"].encode()).hexdigest()
    actor["sources"].append(linked)
    cache["entries"]["a6-document:linked neighbor"] = [0., 1., 0., 0.]
    steps = [dict(tag="store:turn-4", tool="store_memory", arguments=dict(summary=linked["summary"], namespace="default", checkDuplicates=False, parentId="$turn-0"), expectedError=False),
             dict(tag="observed-get", tool="get_memory", arguments=dict(id="$turn-0", namespace="default", recordAccess=True), expectedError=False),
             dict(tag="feedback", tool="reinforce_memory", arguments=dict(id="$turn-0", namespace="default", quality=3), expectedError=False),
             dict(tag="invalid-quality", tool="reinforce_memory", arguments=dict(id="$turn-0", namespace="default", quality=0), expectedError=True)]
    for q in schedule["queries"]:
        steps.append(dict(tag="query:" + q["queryId"], tool="recall_memories",
            arguments=dict(query=q["text"], namespace="default", recordAccess=q["queryId"] == "q0", depth=1, limit=1, compact=False), expectedError=False))
    steps.append(dict(tag="replay-miss", tool="recall_memories", arguments=dict(query="unrecorded-query", namespace="default", recordAccess=False, depth=1, limit=1), expectedError=True))
    labels["relevant"]["q0"] = ["turn-0", "turn-4"]
    schedule["steps"] = steps
    return actor, labels, cache, schedule


def projection(step, mappings, start_id, ordinal):
    arguments = copy.deepcopy(step["arguments"])
    for field in ("id", "parentId"):
        if field in arguments:
            arguments[field] = mappings[arguments[field].removeprefix("$")]
    return dict(phase="action", startId=start_id, deliveryOrdinal=ordinal,
                currentAction=dict(tag=step["tag"], tool=step["tool"], arguments=arguments))


def generation(value):
    if not isinstance(value, str) or not value.isascii() or not value.isdigit() or str(int(value)) != value or not 0 <= int(value) < 2**64:
        raise ValueError("invalid canonical generation")
    return int(value)


def checked_inspect(value):
    if not isinstance(value, dict) or set(value) != {"committed", "logicalHash"} or value["logicalHash"] is not None:
        raise ValueError("unsupported inspect response")
    at = value["committed"]
    if not isinstance(at, dict) or set(at) != {"storeId", "generation"} or str(uuid.UUID(at["storeId"])) != at["storeId"] or uuid.UUID(at["storeId"]).int == 0:
        raise ValueError("invalid store lineage")
    generation(at["generation"])
    return at


def checked_barrier(value, at):
    if not isinstance(value, dict) or set(value) != {"storeId", "committedGeneration", "ready", "pendingWork", "derived"}:
        raise ValueError("unsupported barrier response")
    if value["storeId"] != at["storeId"] or generation(value["committedGeneration"]) < generation(at["generation"]) or value["ready"] is not True or type(value["pendingWork"]) is not int or value["pendingWork"] != 0:
        raise ValueError("barrier did not certify requested lineage/generation")
    views = value["derived"]
    if not isinstance(views, list) or len(views) != 4 or {v.get("name") for v in views} != {"graph", "vectors", "fts", "entities"}:
        raise ValueError("missing or duplicate required derived view")
    for view in views:
        if set(view) != {"name", "appliedGeneration", "ready"} or view["ready"] is not True or view["appliedGeneration"] != value["committedGeneration"]:
            raise ValueError("stale derived generation")
    return value


class Control:
    def __init__(self, read_fd, write_fd, log):
        self.read_fd, self.write_fd, self.log = read_fd, write_fd, log
        self.serial, self.pending = 0, b""
        self.receipts = []

    def call(self, method, params, error=None):
        self.serial += 1
        request = dict(id=self.serial, method=method, params=params)
        def record(event, **fields):
            with self.log.open("a") as stream:
                stream.write(json.dumps(dict(event=event, request=request, **fields)) + "\n")
        record("attempt")
        frame = json.dumps(request).encode() + b"\n"
        while frame:
            count = os.write(self.write_fd, frame)
            if count <= 0:
                raise RuntimeError("control write failed")
            frame = frame[count:]
        deadline = time.monotonic() + 35
        while b"\n" not in self.pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.read_fd], [], [], remaining)[0]:
                raise RuntimeError("control timeout; episode invalid, no retry")
            chunk = os.read(self.read_fd, 16385)
            if not chunk:
                raise RuntimeError("control EOF before ACK")
            self.pending += chunk
            if len(self.pending) > 16384:
                raise RuntimeError("oversized control response")
        line, self.pending = self.pending.split(b"\n", 1)
        response = json.loads(line)
        record("outcome", response=response)
        if type(response.get("id")) is not int or response["id"] != self.serial or set(response) != {"id", "error" if error else "result"}:
            raise ValueError("invalid control ACK")
        if error:
            if set(response["error"]) != {"code", "message"} or response["error"]["code"] != error or not isinstance(response["error"]["message"], str):
                raise ValueError("unexpected control error")
            return response["error"]
        return response["result"]

    def handshake(self):
        # Canonical encoding keeps JSON booleans distinct from integer1.
        if json.dumps(self.call("capabilities", {}), sort_keys=True) != json.dumps(CAPABILITIES, sort_keys=True):
            raise ValueError("unsupported private control capabilities")
        for method in ("reset", "snapshot", "advance", "logicalHash"):
            self.call(method, {}, error="UNSUPPORTED_CONTROL")

    def checkpoint(self, ordinal, acknowledged):
        at = checked_inspect(self.call("inspect", {}))
        report = checked_barrier(self.call("barrier", dict(target=at, deadlineMs=30000)), at)
        after = checked_inspect(self.call("inspect", {}))
        if after != dict(storeId=report["storeId"], generation=report["committedGeneration"]):
            raise ValueError("serialized checkpoint changed after barrier")
        if self.receipts and (at["storeId"] != self.receipts[-1]["target"]["storeId"] or generation(at["generation"]) < generation(self.receipts[-1]["after"]["generation"])):
            raise ValueError("lineage changed or generation regressed")
        receipt = dict(deliveryOrdinal=ordinal, acknowledgedThroughStepId=str(acknowledged), target=at, barrier=report, after=after)
        self.receipts.append(receipt)
        return receipt


def verify(wire, schedule, mappings, receipts):
    outcomes = [r for r in wire if r["event"] == "outcome"]
    assert [r["request"] for r in wire if r["event"] == "attempt"] == [r["request"] for r in outcomes]
    by_tag = {r["tag"]: r for r in outcomes}
    ids = {s: memory for memory, sources in mappings.items() for s in sources}
    assert set(ids) == {"turn-0", "turn-1", "turn-2", "turn-3", "turn-4"}
    assert len(receipts) == len(schedule["steps"]) + 2  # initial + ingestion + current steps
    for ordinal, step in enumerate(schedule["steps"], 1):
        row = by_tag[step["tag"]]
        assert row["response"]["result"].get("isError", False) == step["expectedError"], step["tag"]
        before, after = receipts[ordinal]["after"], receipts[ordinal + 1]["after"]
        changed = step["tag"] in {"store:turn-4", "observed-get", "feedback", "query:q0"}
        assert (generation(after["generation"]) > generation(before["generation"])) if changed else after == before, step["tag"]
        if step["tag"].startswith("query:"):
            value = json.loads(row["response"]["result"]["content"][0]["text"])
            memories, context = value["memories"], value["graphContext"]
            neighbors = context["neighbors"]
            assert value["count"] == len(memories) == 1 and context["neighborCount"] == len(neighbors) == 1
            assert memories[0]["id"] == ids["turn-0"] and memories[0]["namespace"] == "default"
            assert neighbors[0]["id"] == ids["turn-4"] and neighbors[0]["connectedTo"] == ids["turn-0"]
            returned = [m["id"] for m in memories + neighbors]
            assert len(returned) == 2 and set(returned) == {ids["turn-0"], ids["turn-4"]}, returned
    return dict(actualReadyCheckpoints=len(receipts), actualRequiredDerivedViews=4, graphReturnedUniqueIds=[ids["turn-0"], ids["turn-4"]],
        unlinkedAndForeignExcluded=True, observedGetFeedbackAndNeighborSearchAdvancedGeneration=True,
        invalidAndFalseQueriesPreservedGeneration=True, perRecordExposureCount="not-exposed-by-frozen-public-v1-or-control-v1", readGeneration=None)
