"""Actor-safe recorded ANSWER parser. No fixtures, schedules, labels or I/O."""
import hashlib
import json

CONTEXT_FIELDS = ("renderedText", "renderedSha256", "policy", "tokenIdsSha256", "fullTokenCount",
                  "evidenceTokenCount", "includedMemoryIds", "emptyEvidenceOverheadTokenCount")
CONFIG_FIELDS = ("role", "origin", "modelRevision", "roleInputVersion", "roleOutputVersion", "parser", "keyEncoding",
                 "contextPolicy", "runtimeSourcesSha256", "ownedEvaluatorSha256", "ownedConstructorProfile", "clock",
                 "canonicalAllocationControlled", "controlledRng", "embeddingModel", "dimensions", "documentPrefix",
                 "queryPrefix", "retrievalLimit", "graphDepth", "rif", "autolink", "automaticSweep", "decayRateMultiplier",
                 "cache", "activeEmbedding")


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + "\n").encode("utf-8", "strict")


def digest(value):
    return hashlib.sha256(value).hexdigest()


def exact(value, fields):
    if not isinstance(value, dict) or set(value) != set(fields):
        raise ValueError("unexpected fields")


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result: raise ValueError("duplicate JSON key")
            result[key] = value
        return result
    def constant(value):
        raise ValueError("nonfinite JSON value")
    value = json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=pairs, parse_constant=constant)
    pending = [value]
    while pending:
        item = pending.pop()
        if isinstance(item, str): item.encode("utf-8", "strict")
        elif isinstance(item, dict): pending.extend(item.keys()); pending.extend(item.values())
        elif isinstance(item, list): pending.extend(item)
    return value


def role_input(question, context):
    exact(question, ("queryId", "text"))
    if any(not isinstance(v, str) or not v for v in question.values()):
        raise ValueError("invalid current question")
    value = dict(currentQuestion=question, context={k: context[k] for k in CONTEXT_FIELDS})
    if digest(context["renderedText"].encode("utf-8", "strict")) != context["renderedSha256"]:
        raise ValueError("render hash mismatch")
    encoded(value)
    return value


def key(configuration, value):
    parts = [encoded(configuration), encoded(value)]
    return digest(b"a6-recorded-answer-key-v1\0" + b"".join(len(p).to_bytes(8, "big") + p for p in parts))


def consume(envelope):
    exact(envelope, ("role", "roleConfig", "input", "inputSha256", "key", "outputHex", "outputSha256"))
    exact(envelope["roleConfig"], CONFIG_FIELDS)
    if envelope["role"] != "ANSWER" or envelope["roleConfig"].get("role") != "ANSWER":
        raise ValueError("unsupported actor role")
    if (envelope["roleConfig"]["modelRevision"] is not None or envelope["roleConfig"]["origin"] != "synthetic-recorded-output" or
        any(type(envelope["roleConfig"][k]) is not int or envelope["roleConfig"][k] != 1 for k in ("roleInputVersion", "roleOutputVersion"))):
        raise ValueError("unsupported role provenance/version")
    value = envelope["input"]
    exact(value, ("currentQuestion", "context")); exact(value["context"], CONTEXT_FIELDS)
    role_input(value["currentQuestion"], value["context"])
    if envelope["roleConfig"].get("contextPolicy") != value["context"]["policy"]:
        raise ValueError("role/context policy mismatch")
    if digest(encoded(value)) != envelope["inputSha256"] or key(envelope["roleConfig"], value) != envelope["key"]:
        raise ValueError("recorded role input/key mismatch")
    raw = bytes.fromhex(envelope["outputHex"])
    if raw.hex() != envelope["outputHex"] or digest(raw) != envelope["outputSha256"]:
        raise ValueError("recorded output bytes/hash mismatch")
    try:
        output = decode(raw)
        exact(output, ("roleOutputVersion", "status", "text", "citations"))
        if type(output["roleOutputVersion"]) is not int or output["roleOutputVersion"] != 1:
            raise ValueError("unsupported output version")
        if output["status"] not in ("answered", "refused") or not isinstance(output["text"], str):
            raise ValueError("invalid recorded status/text")
        if not isinstance(output["citations"], list): raise ValueError("invalid citations")
        for citation in output["citations"]:
            exact(citation, ("memoryId", "quote"))
            if any(not isinstance(v, str) or not v for v in citation.values()):
                raise ValueError("invalid citation")
        if output["status"] == "answered" and (not output["text"] or not output["citations"]):
            raise ValueError("empty answer/citations")
        if output["status"] == "refused" and output["citations"]:
            raise ValueError("refusal with citations")
    except (ValueError, TypeError, UnicodeError) as error:
        return dict(status="rejected", reason="output-parse", detail=str(error), outputSha256=envelope["outputSha256"])
    return dict(status="accepted" if output["status"] == "answered" else "unavailable",
                reason="recorded-answer" if output["status"] == "answered" else "recorded-refusal",
                outputSha256=envelope["outputSha256"], output=output)


def verify_citations(output, public, context):
    allowed = {m["id"]: m["summary"] for m in public if m["id"] in context["includedMemoryIds"]}
    seen = set()
    for citation in output["citations"]:
        identity, quote = citation["memoryId"], citation["quote"]
        if identity not in allowed or not quote or quote not in allowed[identity] or identity in seen:
            raise ValueError("citation outside actual packed public payload")
        seen.add(identity)
