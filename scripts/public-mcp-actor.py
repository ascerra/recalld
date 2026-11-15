#!/usr/bin/env python3
"""Synthetic actor: current action only. No evaluator artifacts, future schedule, or controls."""
import argparse
import json
import os
import select
import sys
import time
import uuid

TRACK = "serialized-mcp-synthetic-diagnostic-v1"

def accounted_exchange(read, write, request, notification, emit, timeout=20, limit=4 * 1024 * 1024):
    """One real RPC; partial writes are not retried and ambiguous outcomes stay unknown."""
    start = time.monotonic_ns()
    outgoing = json.dumps(request).encode() + b"\n"
    raw = bytearray()
    facts = dict(accountingVersion=1, requestBytesIntended=str(len(outgoing)), requestWireHex=outgoing.hex(),
                 requestBytesWritten=None, responseBytesRead="0", responseWireHex="", observedClientElapsedNs=None,
                 category="attempt", mutationOutcome="unknown")
    emit("attempt", None, dict(facts))
    category = "write_error"
    try:
        count = write.write(outgoing)
        if type(count) is int and 0 <= count <= len(outgoing):
            facts["requestBytesWritten"] = str(count)
        if count != len(outgoing):
            raise OSError("incomplete request write; no action retry")
        response = None
        if not notification:
            deadline = time.monotonic() + timeout
            while not raw.endswith(b"\n"):
                remaining = deadline - time.monotonic()
                category = "timeout"
                if remaining <= 0 or not select.select([read], [], [], remaining)[0]:
                    raise TimeoutError("MCP response timeout; outcome unknown")
                category = "read_error"
                chunk = read.read(min(65536, limit + 1 - len(raw)))
                category = "eof"
                if not chunk:
                    raise EOFError("MCP response ended before newline; outcome unknown")
                raw.extend(chunk)
                category = "oversized_response"
                if len(raw) > limit:
                    raise ValueError("MCP response limit exceeded")
            category = "response_parse"
            response = json.loads(raw)
            category = "response_shape"
            if not isinstance(response, dict) or ("result" in response) == ("error" in response):
                raise ValueError("invalid MCP response shape")
            category = "response_id"
            if response.get("id") != request["id"]:
                raise ValueError("mismatched MCP response id")
        facts.update(responseBytesRead=str(len(raw)), responseWireHex=raw.hex(),
                     observedClientElapsedNs=str(time.monotonic_ns() - start),
                     category="notification" if notification else "protocol_error" if "error" in response else
                         "tool_error" if isinstance(response.get("result"), dict) and response["result"].get("isError") else "success")
        # The public reply is evidence, not an inferred canonical receipt/rollback.
        emit("outcome", response, dict(facts))
        return response
    except BaseException:
        facts.update(responseBytesRead=str(len(raw)), responseWireHex=raw.hex(),
                     observedClientElapsedNs=str(time.monotonic_ns() - start), category=category)
        emit("error", None, dict(facts))
        raise


def run(args):
    projection = json.load(sys.stdin)
    phase = projection["phase"]
    expected = {"phase", "startId", "deliveryOrdinal", "sources", "activeEmbedding"} if phase == "ingest" else {"phase", "startId", "deliveryOrdinal", "currentQuestion", "expectedMiss"}
    assert set(projection) == expected and phase in ("ingest", "query"), "unexpected actor projection fields"
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
        if args.accounting:
            def emit(event, response, accounting):
                print(json.dumps(dict(event=event, stepId=str(serial), deliveryOrdinal=projection["deliveryOrdinal"],
                    tag=tag, request=request, response=response, accounting=accounting)), flush=True)
            response = accounted_exchange(read, write, request, notification, emit)
        else:
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
        call("recall_memories", dict(query=query["text"], namespace="default", recordAccess=False, depth=0, limit=1),
             "replay-miss" if projection["expectedMiss"] else "query:" + query["queryId"], error=projection["expectedMiss"])

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--accounting", action="store_true")
    for flag in ("read-fd", "write-fd", "deny-fd"):
        parser.add_argument("--" + flag, type=int, required=True)
    run(parser.parse_args())
