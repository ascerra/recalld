#!/usr/bin/env python3
"""Synthetic actor: current action only. No evaluator artifacts, future schedule, or controls."""
import argparse
import json
import os
import select
import sys
import uuid

TRACK = "serialized-mcp-synthetic-diagnostic-v1"

def run(args):
    projection = json.load(sys.stdin)
    phase = projection["phase"]
    expected = {"phase", "startId", "deliveryOrdinal", "sources", "activeEmbedding"} if phase == "ingest" else {"phase", "startId", "deliveryOrdinal", "currentQuestion", "expectedMiss"}
    if args.context_bundle and phase=="query":expected.add("contextPolicy")
    assert set(projection) == expected and phase in ("ingest", "query"), "unexpected actor projection fields"
    tokenizer=encoder=None
    if args.context_bundle and phase=="query":
        import context_tokenizer as tokenizer
        encoder=tokenizer.load(args.context_bundle)
        tokenizer.overhead(encoder,projection["currentQuestion"]["text"],projection["contextPolicy"])
    source = projection.get("sources", [])
    read = os.fdopen(args.read_fd, "rb", buffering=0)
    write = os.fdopen(args.write_fd, "wb", buffering=0)
    try:
        os.fstat(args.deny_fd)
    except OSError:
        print(json.dumps(dict(event="isolation", tag="private-fd-closed", stepId=str(projection["startId"]), deliveryOrdinal=projection["deliveryOrdinal"], fdClosed=True)), flush=True)
    else:
        raise RuntimeError("evaluator-only descriptor leaked to actor")
    serial = projection["startId"]
    def rpc(method, params, tag, notification=False, expected_error=False):
        nonlocal serial
        serial += 1
        request = dict(jsonrpc="2.0", method=method, params=params)
        if not notification:
            request["id"] = serial
        # Retain the attempt before waiting for a response.
        print(json.dumps(dict(event="attempt", stepId=str(serial), deliveryOrdinal=projection["deliveryOrdinal"], tag=tag, request=request)), flush=True)
        write.write(json.dumps(request).encode() + b"\n")
        response = None
        if not notification:
            if not select.select([read], [], [], 20)[0]:
                raise RuntimeError("MCP timeout; no retry of an unkeyed action")
            raw = read.readline(4 * 1024 * 1024)
            response = json.loads(raw)
            if response.get("id") != serial:
                raise RuntimeError("mismatched MCP response id")
        print(json.dumps(dict(event="outcome", stepId=str(serial), deliveryOrdinal=projection["deliveryOrdinal"], tag=tag, request=request, response=response)), flush=True)
        if expected_error:
            assert response["error"]["code"] == -32601
            return None
        if notification:
            return None
        if "error" in response:
            raise RuntimeError("MCP request rejected")
        return response["result"]
    def call(name, arguments, tag, error=False):
        result = rpc("tools/call", dict(name=name, arguments=arguments), tag)
        if error:
            assert result.get("isError") is True
            return None
        assert not result.get("isError", False)
        text = json.loads(result["content"][0]["text"])
        if "structuredContent" in result:
            assert result["structuredContent"] == text
        return text
    if phase == "ingest":
        result = rpc("initialize", dict(protocolVersion="2025-06-18", capabilities={}, clientInfo=dict(name=TRACK, version="1")), "initialize")
        assert result["protocolVersion"] == "2025-06-18"
        rpc("notifications/initialized", {}, "initialized", notification=True)
        names = {t["name"] for t in rpc("tools/list", {}, "tools")["tools"]}
        assert {"store_memory", "recall_memories"} <= names
        assert not names & {"barrier", "snapshot", "reset", "advance", "inspect", "evaluator/capabilities"}
        session = rpc("resources/read", dict(uri="recalld://session"), "session")
        context = json.loads(session["contents"][0]["text"])
        assert context["supportedContractVersions"] == [1]
        assert context["backendContext"]["contextVersion"] == 1
        assert context["backendContext"]["activeEmbedding"] == projection["activeEmbedding"]
        assert uuid.UUID(context["backendContext"]["storeId"]).int != 0
        rpc("evaluator/capabilities", {}, "private-control-denied", expected_error=True)
        call("create_namespace", dict(name="foreign", embeddingDim=4), "namespace")
        for entry in source:
            call("store_memory", dict(summary=entry["summary"], namespace=entry["namespace"], checkDuplicates=False), "store:" + entry["sourceId"])
    else:
        query = projection["currentQuestion"]
        assert set(query) == {"queryId", "text"}
        result=call("recall_memories", dict(query=query["text"], namespace="default", recordAccess=False, depth=0, limit=3 if tokenizer else 1),
             "replay-miss" if projection["expectedMiss"] else "query:" + query["queryId"], error=projection["expectedMiss"])
        if tokenizer and not projection["expectedMiss"]:
            context=tokenizer.assemble(encoder,query["text"],result["memories"],projection["contextPolicy"])
            print(json.dumps(dict(event="context",tag="context:"+query["queryId"],stepId=str(serial),deliveryOrdinal=projection["deliveryOrdinal"],context=context)),flush=True)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context-bundle")
    for flag in ("read-fd", "write-fd", "deny-fd"):
        parser.add_argument("--" + flag, type=int, required=True)
    run(parser.parse_args())
