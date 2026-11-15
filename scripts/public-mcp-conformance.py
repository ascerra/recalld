"""Evaluator-only frozen synthetic action schedule and public-response assertions."""
import copy
import hashlib
import json

TRACK = "serialized-mcp-synthetic-conformance-v1"


def document_text(arguments):
    # Frozen production McpStorageAdapter::store_memory input composition.
    text = arguments["summary"]
    if "fullText" in arguments:
        text += "\n\n" + arguments["fullText"]
    if arguments.get("tags"):
        text += " " + " ".join(arguments["tags"])
    return text


def fixtures(base):
    actor, labels, cache, _ = base()
    texts = [("amber OBSOLETE_CONTENT", "default"), ("birch FORGET_CONTENT", "default"),
             ("foreign NAMESPACE_CONTENT", "foreign"), ("amber CORRECTED_CONTENT", "default"),
             ("rejected correction", "default")]
    sources = []
    for i, (text, namespace) in enumerate(texts):
        source = dict(actor["sources"][0], sourceId=f"turn-{i}", turnId=f"turn-{i}", ordinal=i,
                      summary=text, namespace=namespace, contentSha256=hashlib.sha256(text.encode()).hexdigest())
        source["fullText"] = text + " synthetic full payload"
        source["fullTextSha256"] = hashlib.sha256(source["fullText"].encode()).hexdigest()
        source["tags"] = ["synthetic/payload-" + str(i)]
        sources.append(source)
    actor["sources"] = sources
    steps = []
    def action(tag, tool, arguments, error=False):
        steps.append(dict(tag=tag, tool=tool, arguments=arguments, expectedError=error))
    def fetch(tag, source, namespace="default", error=False):
        arguments = dict(id="$" + source, recordAccess=False)
        if namespace is not None:
            arguments["namespace"] = namespace
        action(tag, "get_memory", arguments, error)
    queries = []
    def search(qid, text="amber", namespace="default"):
        queries.append(dict(queryId=qid, text=text))
        action("query:" + qid, "recall_memories", dict(query=text, namespace=namespace, recordAccess=False, depth=0, limit=20))
    fetch("read-before", "turn-0")
    search("before")
    steps[-1]["waitSeconds"] = 1.1
    fetch("read-after", "turn-0")
    fetch("invalid-before", "turn-1")
    for i, quality in enumerate((0, 5, 1.5, True, None, "3")):
        action(f"invalid-quality-{i}", "reinforce_memory", dict(id="$turn-1", namespace="default", quality=quality), True)
    fetch("invalid-after", "turn-1")
    for quality in (1, 2, 3, 4):
        action(f"feedback-{quality}", "reinforce_memory", dict(id="$turn-1", namespace="default", quality=quality))
    fetch("foreign-before", "turn-2", "foreign")
    fetch("wrong-namespace-fetch", "turn-2", "default", True)
    action("wrong-namespace-feedback", "reinforce_memory", dict(id="$turn-2", namespace="default", quality=4), True)
    action("wrong-namespace-forget", "forget_memory", dict(id="$turn-2", namespace="default"), True)
    fetch("foreign-after", "turn-2", None)
    action("store:turn-4", "store_memory", dict(summary=texts[4][0], namespace="default", checkDuplicates=False, supersedes="$turn-2"), True)
    action("store:turn-3", "store_memory", dict(summary=texts[3][0], fullText=sources[3]["fullText"], tags=sources[3]["tags"], namespace="default", checkDuplicates=False, supersedes="$turn-0"))
    search("corrected")
    fetch("correction-before", "turn-3")
    for source in ("turn-0", "turn-1"):
        action("forget:" + source, "forget_memory", dict(id="$" + source, namespace="default"))
        fetch("deleted:" + source, source)
        # Query immediately after the acknowledged deletion, including graph expansion.
        search("deleted-" + source, "amber" if source == "turn-0" else "birch")
        steps[-1]["arguments"]["depth"] = 1
    search("foreign", "foreign", "foreign")
    steps.append(dict(restart=True))
    fetch("restart-deleted:turn-0", "turn-0")
    fetch("restart-deleted:turn-1", "turn-1")
    fetch("restart-correction", "turn-3")
    search("restart-default")
    steps[-1]["arguments"]["depth"] = 1
    search("restart-birch", "birch")
    search("restart-foreign", "foreign", "foreign")
    search("future", "amber FUTURE_CONFORMANCE_QUERY")
    action("replay-miss", "recall_memories", dict(query="unrecorded-query", namespace="default", recordAccess=False, depth=0, limit=1), True)
    schedule = dict(scheduleVersion=1, queries=queries, steps=steps, futureQueryCanary="FUTURE_CONFORMANCE_QUERY")
    labels["relevant"] = {q["queryId"]: ["turn-2" if "foreign" in q["queryId"] else "turn-0" if q["queryId"] == "before" else "turn-3"] for q in queries}
    labels["relevant"]["restart-birch"] = []
    del labels["relevant"]["deleted-turn-1"]
    cache["entries"] = {"a6-document:" + document_text(source): [0., 1., 0., 0.] if i == 1 else [1., 0., 0., 0.] for i, source in enumerate(sources)}
    cache["entries"]["a6-document:health check"] = [0., 0., 0., 1.]
    for query in queries:
        cache["entries"]["a6-query:" + query["text"]] = [0., 1., 0., 0.] if query["text"] == "birch" else [1., 0., 0., 0.]
    return actor, labels, cache, schedule


def projection(step, mappings, start_id, ordinal):
    # Substitute only IDs obtained from prior acknowledged public stores.
    arguments = copy.deepcopy(step["arguments"])
    for field in ("id", "supersedes"):
        if field in arguments:
            arguments[field] = mappings[arguments[field].removeprefix("$")]
    return dict(phase="action", startId=start_id, deliveryOrdinal=ordinal,
                currentAction=dict(tag=step["tag"], tool=step["tool"], arguments=arguments))


def value(row):
    result = row["response"]["result"]
    assert not result.get("isError", False), row["tag"]
    parsed = json.loads(result["content"][0]["text"])
    if "structuredContent" in result:
        assert parsed == result["structuredContent"]
    return parsed


def verify(wire, schedule, mappings, sources, process_events):
    outcomes = [r for r in wire if r["event"] == "outcome"]
    by_tag = {r["tag"]: r for r in outcomes}
    attempted = [r["request"] for r in wire if r["event"] == "attempt"]
    assert attempted == [r["request"] for r in outcomes], "dropped action/outcome"
    invalid = []
    for step in schedule["steps"]:
        if step.get("restart"):
            continue
        row = by_tag[step["tag"]]
        assert row["response"]["result"].get("isError", False) == step["expectedError"], step["tag"]
        if step["expectedError"]:
            message = json.dumps(row["response"]["result"]).lower()
            if step["tag"].startswith("invalid-quality"):
                assert "quality" in message
            if step["tag"].startswith("wrong-namespace") or step["tag"] == "store:turn-4":
                assert "namespace" in message
            invalid.append(dict(tag=step["tag"], request=row["request"], response=row["response"]))
    ids = {source: memory for memory, source_ids in mappings.items() for source in source_ids}
    assert set(ids) == {"turn-0", "turn-1", "turn-2", "turn-3"}
    correction = value(by_tag["store:turn-3"])
    assert correction["supersedes"] == dict(target=ids["turn-0"], status="applied")
    # These are public observations, not a private counter/generation audit.
    for before, after in (("read-before", "read-after"), ("invalid-before", "invalid-after"),
                          ("foreign-before", "foreign-after"), ("correction-before", "restart-correction"), ("deleted:turn-0", "restart-deleted:turn-0"),
                          ("deleted:turn-1", "restart-deleted:turn-1")):
        assert value(by_tag[before]) == value(by_tag[after]), (before, after)
    for quality in (1, 2, 3, 4):
        feedback = value(by_tag[f"feedback-{quality}"])
        assert feedback["id"] == ids["turn-1"] and 0 <= feedback["strength"] <= 1 and feedback["stability"] > 0
    deleted_at = {}
    for source in ("turn-0", "turn-1"):
        ack = by_tag["forget:" + source]
        assert value(ack) == dict(id=ids[source], deleted=True)
        deleted_at[source] = int(ack["stepId"])
        assert sources[int(source.split("-")[1])]["tags"]
        assert sources[int(source.split("-")[1])]["fullText"]
        for prefix in ("deleted:", "restart-deleted:"):
            tombstone = value(by_tag[prefix + source])
            assert tombstone["id"] == ids[source] and tombstone["phase"] == "Tombstone"
            assert tombstone["summary"] == "" and tombstone["fullText"] is None and tombstone["tags"] == []
    for row in outcomes:
        if not row["tag"].startswith("query:"):
            continue
        memories = value(row)["memories"]
        namespace = row["request"]["params"]["arguments"]["namespace"]
        assert all(sources[int(mappings[m["id"]][0].split("-")[1])]["namespace"] == namespace for m in memories)
        for source, ack in deleted_at.items():
            if int(row["stepId"]) > ack:
                assert ids[source] not in [m["id"] for m in memories]
                assert sources[int(source.split("-")[1])]["summary"] not in json.dumps(memories)
    for tag in ("query:corrected", "query:restart-default", "query:future"):
        returned = [m["id"] for m in value(by_tag[tag])["memories"]]
        assert ids["turn-3"] in returned and ids["turn-0"] not in returned
    for tag in ("query:foreign", "query:restart-foreign"):
        assert [m["id"] for m in value(by_tag[tag])["memories"]] == [ids["turn-2"]]
    sessions = [json.loads(r["response"]["result"]["contents"][0]["text"])["backendContext"] for r in outcomes if r["tag"] == "session"]
    assert len(sessions) == 2 and sessions[0] == sessions[1]
    assert len(process_events) == 2 and process_events[0]["pid"] != process_events[1]["pid"]
    assert process_events[0]["dataRoot"] == process_events[1]["dataRoot"]
    assert all(event["exitCode"] == 0 for event in process_events)
    return dict(status="passed", invalidActions=invalid, successfulFeedbackQualities=[1, 2, 3, 4],
                restartStoreId=sessions[0]["storeId"], processEvents=process_events,
                tombstoneFetch="empty public metadata retains UUID; recall excludes UUID/content",
                falseReadProof="public record fields unchanged; no private counter/ReadView claim",
                sourceMappingCount=len(mappings), actorActions=len(outcomes))
