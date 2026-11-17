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

COMBINED = runpy.run_path(str(Path(__file__).with_name("combined-mcp-evaluation.py")))
CONTEXT = runpy.run_path(str(Path(__file__).with_name("context-mcp-evaluation.py")))
PAIRED = runpy.run_path(str(Path(__file__).with_name("paired-mcp-evaluation.py")))
BEIR = runpy.run_path(str(Path(__file__).with_name("beir-mcp-evaluation.py")))
GROUPED = runpy.run_path(str(Path(__file__).with_name("grouped-mcp-evaluation.py")))
TRACK = "serialized-mcp-synthetic-diagnostic-v1"
MODEL = "a6-public-synthetic-v1"
CONFORMANCE = runpy.run_path(str(Path(__file__).with_name("public-mcp-conformance.py")))
OWNED = runpy.run_path(str(Path(__file__).with_name("owned-mcp-supervisor.py")))
RESET = runpy.run_path(str(Path(__file__).with_name("reset-mcp-supervisor.py")))
SWEEP = runpy.run_path(str(Path(__file__).with_name("sweep-mcp-supervisor.py")))
NATIVE = runpy.run_path(str(Path(__file__).with_name("native-prefix-mcp-supervisor.py")))
NATIVE_SWEEP = runpy.run_path(str(Path(__file__).with_name("native-sweep-mcp-supervisor.py")))
ROOT = Path(__file__).resolve().parents[1]
CAPABILITIES = dict(publicMcp=True, publicV2=False, fullA6=False, coherentReadView=False,
                    barrier=False, reset=False, snapshot=False, clock="system-uncontrolled",
                    clockAdvance=False, deterministicRng=False, deterministicIds=False,
                    canonicalSchemaAttested=False)
VERSIONS = dict(publicContractVersion=1, canonicalSchemaVersion=None,
                canonicalRepresentation="unattested-via-public-v1", contextVersion=1)
EXTRA = ("scripts/combined-mcp-evaluation.py", "scripts/test_combined_mcp_evaluation.py", "scripts/test_combined_mcp_episode.py",
         "tests/live_tool_schema_conformance.py", "tests/page_schema_conformance.py",
         # Exact retained format1/4 histories used by the admission/replay tests.
         'src/mcp/fixtures/legacy-compound-before.zip',
         'src/mcp/fixtures/legacy-compound-before.zip.json',
         'src/mcp/fixtures/legacy-semantic-before/completed.zip',
         'src/mcp/fixtures/legacy-semantic-before/fault-false.zip',
         'src/mcp/fixtures/legacy-semantic-before/fault-true.zip',
         'src/mcp/fixtures/legacy-semantic-before/faults.json',
         'src/mcp/fixtures/legacy-semantic-before/history.json',
         'src/mcp/fixtures/legacy-semantic-before/pending.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-after_batch_commit.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-after_canonical_ack.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-after_edge_commit.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-after_insert_payload_sync.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-after_metadata_commit.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-before_batch_commit.zip',
         'src/mcp/fixtures/legacy-semantic-before/process-before_metadata_commit.zip',
         'src/mcp/fixtures/legacy-semantic-before/process.json',
         "tests/query-schema-requirements.txt", "tests/query_schema_conformance.py",
         # Exact native replay inputs; no directory wildcard or ambient scratch.
         'src/storage/vector_certification/fixtures/pending-before-certification.zip',
         'src/storage/vector_certification/fixtures/pending-multi-provenance.json',
         'src/storage/vector_certification/fixtures/pending-v2-api2-generatedfalse.zip',
         'src/storage/vector_certification/fixtures/pending-v2-api2-generatedtrue.zip',
         'src/storage/vector_certification/fixtures/pending-v3-api1-generatedfalse.zip',
         'src/storage/vector_certification/fixtures/pending-v3-api2-generatedtrue.zip',
         'src/storage/metadata/native_records/fixtures/document-recipe-legacy-receipts.json',
         'src/storage/metadata/native_records/fixtures/document-recipe-legacy.jsonl',
         'tests/fixtures/native-api1-batch-before158/PROVENANCE.json',
         'tests/fixtures/native-api1-batch-before158/README.md',
         'tests/fixtures/native-api1-batch-before158/before-probe.rs.txt',
         'tests/fixtures/native-api1-batch-before158/manifest.json',
         'tests/fixtures/native-api1-batch-before158/store.zip',
         'tests/fixtures/native-batch-before106-oversize/completed.zip',
         'tests/fixtures/native-batch-before106-oversize/pending.zip',
         'tests/fixtures/native-batch-before106-oversize/provenance.json',
         'tests/fixtures/native-batch-before106-oversize/request.json',
         'tests/fixtures/native-batch-before106-oversize/reservation.json',
         'tests/fixtures/native-batch-before106/provenance.json',
         'tests/fixtures/native-batch-before106/v1-manifest.json',
         'tests/fixtures/native-batch-before106/v1-request.json',
         'tests/fixtures/native-batch-before106/v1-reservation.json',
         'tests/fixtures/native-batch-before106/v1-store.zip',
         'tests/fixtures/native-batch-before106/v2-manifest.json',
         'tests/fixtures/native-batch-before106/v2-request.json',
         'tests/fixtures/native-batch-before106/v2-reservation.json',
         'tests/fixtures/native-batch-before106/v2.jsonl',
         'tests/fixtures/native-batch-before106/v3-manifest.json',
         'tests/fixtures/native-batch-before106/v3-request.json',
         'tests/fixtures/native-batch-before106/v3-reservation.json',
         'tests/fixtures/native-batch-before106/v3.jsonl',
         'src/mcp/fixtures/legacy-lifecycle-old-api1.json', 'src/mcp/fixtures/legacy-lifecycle-old-api1.zip', 'src/storage/metadata/native_records/fixtures/original-api-old2.zip', 'src/storage/metadata/native_records/fixtures/request-context-api2.zip',
         "scripts/beir-mcp-evaluation.py", "scripts/beir-reference-lock.json", "scripts/test_beir_mcp_evaluation.py", "scripts/check_beir_mcp_evaluation.py", "scripts/paired-mcp-evaluation.py", "scripts/test_paired_mcp_evaluation.py", "scripts/check_paired_mcp_evaluation.py", "scripts/grouped-mcp-evaluation.py", "scripts/test_grouped_mcp_evaluation.py", "scripts/check_grouped_mcp_evaluation.py", "scripts/test_trace_accounting.py", "scripts/check_trace_accounting.py", "scripts/public-mcp-conformance.py", "scripts/public-mcp-evaluation.py", "scripts/public-mcp-actor.py", "scripts/test_public_mcp_evaluation.py",
         "src/storage/metadata/native_records/fixtures/phase-clock-legacy.jsonl", "scripts/owned-mcp-supervisor.py", "scripts/test_owned_mcp_episode.py", "scripts/reset-mcp-supervisor.py", "scripts/test_reset_mcp_episode.py", "scripts/sweep-mcp-supervisor.py", "scripts/test_sweep_mcp_episode.py", "scripts/native-prefix-mcp-supervisor.py", "scripts/test_native_prefix_episode.py", "scripts/native-sweep-mcp-supervisor.py", "scripts/test_native_sweep_episode.py", "src/storage/engine/fixtures/text-compaction-before.zip",
         *["docsbench/native-sweep-consumer/full-e1d4cea9/inputs/"+name for name in ("manifest.json","actor-inputs.json","evaluator-labels.json","evaluator-cache.json","evaluator-schedule.json")])
EXTRA += ("scripts/context-mcp-evaluation.py", "scripts/context_tokenizer.py", "scripts/context-tokenizer-lock.json", "scripts/context-encoding.json", "scripts/TIKTOKEN-LICENSE.txt", "scripts/prepare-context-assets.py", "scripts/test_context_mcp_evaluation.py", "scripts/check_context_mcp_evaluation.py",)


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


ACCOUNTING_TRACK = "serialized-mcp-accounting-synthetic-diagnostic-v1"
ACCOUNTING_POLICY = dict(accountingVersion=1, transport="mcp-stdio-client-buffers-v1",
    provider="synthetic-replay-http-bodies-v1", traceDelivery="append-flush-before-outcome-consumption",
    tokenizer=None, answerContextCapTokens=None, productionPricing=None,
    unknownOutcome="preserve-no-rollback-or-retry", sampled=False)


class TraceRecorder:
    """Supervisor-owned context; actor only supplies current public RPC facts."""
    def __init__(self, path, environment, episode, embedding=None, *, trace_version=1, group_id="synthetic-regression"):
        self.file = path.open("x")
        self.trace_version, self.group_id = trace_version, group_id
        self.environment, self.episode, self.embedding = environment, episode, embedding
        self.pending, self.seen, self.sequence = {}, set(), 0

    def append(self, event, row, category=None):
        self.sequence += 1
        observed = None
        if event == "outcome" and row.get("tag", "").startswith("query:"):
            try:
                payload = json.loads(row["response"]["result"]["content"][0]["text"])
                observed = [dict(publicResponseRank=str(i + 1), memoryId=v["id"], publicScore=v.get("score")) for i,v in enumerate(payload["memories"])]
            except (KeyError, TypeError, ValueError):
                pass  # Preserve the raw public response; do not invent candidate fields.
        value = dict(traceSchemaVersion=self.trace_version, accountingVersion=1, sequence=str(self.sequence), eventKind=event,
            runId=self.environment, episodeId=self.episode, groupId=self.group_id, environmentHash=self.environment,
            stepId=row.get("stepId"), deliveryOrdinal=row.get("deliveryOrdinal"), action=row.get("request"),
            outcome=row.get("response"), accounting=row.get("accounting"), errorCategory=category,
            sourceId=row.get("tag", "")[6:] if row.get("tag", "").startswith("store:") else None,
            sourceMappingsFile="source-mappings.json", publicRankObservations=observed,
            providerEvidence=dict(file="actor-provider.jsonl", deliveryOrdinal=row.get("deliveryOrdinal"),
                scope="delivery-window-includes-shared-startup-not-per-rpc", configuredModel=MODEL, activeEmbedding=self.embedding),
            readGeneration=None, readiness=dict(available=False), simulatedTimeMs=None, snapshot=None, reward=None)
        self.file.write(json.dumps(value) + "\n"); self.file.flush()

    def consume(self, row):
        kind = row["event"]
        if kind == "isolation": return
        identity = row["stepId"]
        if kind == "attempt":
            if identity in self.seen: raise ValueError("duplicate RPC attempt")
            self.seen.add(identity); self.pending[identity] = row
        elif kind in ("outcome", "error"):
            prior = self.pending.pop(identity, None)
            if prior is None or prior["request"] != row["request"]: raise ValueError("unmatched RPC outcome")
        else: raise ValueError("unknown actor event")
        self.append(kind, row, row["accounting"]["category"] if kind == "error" else None)

    def interrupted(self, category):
        for row in self.pending.values(): self.append("incomplete", row, category)
        self.pending.clear()

    def process_error(self, category): self.append("process_error", {}, category)

    def close(self):
        self.interrupted("supervisor_finalization"); self.file.close()


def run_accounted_actor(command, projection, wire, actor_log, root, env, pass_fds, trace, timeout=90):
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=actor_log,
                             cwd=root, env=env, pass_fds=pass_fds, bufsize=0)
    pending = bytearray(); deadline = time.monotonic() + timeout
    try:
        child.stdin.write(encoded(projection)); child.stdin.close()
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([child.stdout], [], [], remaining)[0]:
                raise TimeoutError("actor event stream timeout")
            chunk = os.read(child.stdout.fileno(), 65536)
            if not chunk: break
            pending.extend(chunk)
            if len(pending) > 16 * 1024 * 1024: raise ValueError("actor event limit exceeded")
            while b"\n" in pending:
                line, _, rest = pending.partition(b"\n"); pending = bytearray(rest)
                wire.write(line + b"\n"); wire.flush()
                # The prior attempt's append/flush completes before the next event is consumed.
                trace.consume(json.loads(line))
        if pending:
            wire.write(pending); wire.flush(); raise ValueError("partial actor event")
        code = child.wait(timeout=max(.001, deadline - time.monotonic()))
        if code or trace.pending:
            trace.process_error("actor_exit_or_unfinished_rpc"); trace.interrupted("actor_exit_unknown")
            code = code or 1
        return subprocess.CompletedProcess(command, code)
    except BaseException:
        trace.process_error("actor_stream_failure"); trace.interrupted("actor_stream_unknown")
        raise
    finally:
        if child.poll() is None:
            child.terminate()
            try: child.wait(timeout=5)
            except subprocess.TimeoutExpired: child.kill(); child.wait()
        child.stdout.close()
        if not child.stdin.closed: child.stdin.close()


def reconcile_accounting(trace_rows, provider_rows):
    attempts, terminal, process_errors = {}, {}, 0
    for index, row in enumerate(trace_rows, 1):
        if type(row.get("traceSchemaVersion")) is not int or row["traceSchemaVersion"] != 1: raise ValueError("unsupported trace1 version")
        if row["sequence"] != str(index): raise ValueError("trace sequence gap")
        kind = row["eventKind"]; identity = row["stepId"]
        if kind == "process_error": process_errors += 1; continue
        if kind == "attempt":
            if identity in attempts: raise ValueError("duplicate trace attempt")
            attempts[identity] = row
        elif kind in ("outcome", "error", "incomplete"):
            if identity not in attempts or identity in terminal or row["action"] != attempts[identity]["action"]:
                raise ValueError("unmatched/duplicate trace completion")
            terminal[identity] = row
        else: raise ValueError("unsupported trace event")
        facts = row["accounting"]
        outgoing = bytes.fromhex(facts["requestWireHex"]); incoming = bytes.fromhex(facts["responseWireHex"])
        if str(len(outgoing)) != facts["requestBytesIntended"] or str(len(incoming)) != facts["responseBytesRead"] or json.loads(outgoing) != row["action"]:
            raise ValueError("buffer byte/accounting mismatch")
        if type(facts["accountingVersion"]) is not int or facts["accountingVersion"] != 1:
            raise ValueError("unsupported buffer accounting version")
        elapsed = facts["observedClientElapsedNs"]
        if elapsed is not None and (not isinstance(elapsed, str) or not elapsed.isascii() or not elapsed.isdecimal()):
            raise ValueError("invalid observed duration")
        if kind == "outcome":
            if facts["category"] == "notification":
                if incoming or row["outcome"] is not None: raise ValueError("notification has response")
            elif json.loads(incoming) != row["outcome"]: raise ValueError("response buffer/outcome mismatch")
        written = facts["requestBytesWritten"]
        if written is not None and (not written.isdecimal() or int(written) > len(outgoing)):
            raise ValueError("invalid completed request bytes")
    if attempts.keys() != terminal.keys(): raise ValueError("missing terminal or explicit incomplete action")
    values = list(terminal.values())
    categories = sorted({v["accounting"]["category"] for v in values if v["eventKind"] != "incomplete"})
    known = [v["accounting"] for v in values if v["eventKind"] != "incomplete"]
    for row in provider_rows:
        fact = row["accounting"]
        if str(len(bytes.fromhex(fact["requestBodyHex"]))) != fact["requestBodyBytes"] or str(len(bytes.fromhex(fact["responseBodyHex"]))) != fact["responseBodyBytesIntended"]:
            raise ValueError("provider body accounting mismatch")
        if fact["responseBodyBytesWritten"] is not None and int(fact["responseBodyBytesWritten"]) > int(fact["responseBodyBytesIntended"]):
            raise ValueError("invalid provider write bytes")
    provider_known = [r["accounting"] for r in provider_rows if r["accounting"]["responseBodyBytesWritten"] is not None]
    return dict(accountingVersion=1, denominatorUnit="complete-external-synthetic-episode", sampled=False,
        attemptedActions=str(len(attempts)), terminalRpcObservations=str(sum(v["eventKind"] == "outcome" for v in values)),
        failedRpcObservations=str(sum(v["eventKind"] == "error" for v in values)), incompleteActions=str(sum(v["eventKind"] == "incomplete" for v in values)),
        actorProcessErrors=str(process_errors), categories={c:str(sum(v["eventKind"] != "incomplete" and v["accounting"]["category"] == c for v in values)) for c in categories},
        requestBytesIntended=str(sum(int(v["accounting"]["requestBytesIntended"]) for v in attempts.values())),
        requestBytesWrittenKnown=str(sum(int(v["requestBytesWritten"]) for v in known if v["requestBytesWritten"] is not None)),
        requestWriteUnknownActions=str(sum(v["eventKind"] == "incomplete" or v["accounting"]["requestBytesWritten"] is None for v in values)),
        responseBytesReadKnown=str(sum(int(v["responseBytesRead"]) for v in known)),
        observedClientElapsedNsByStep={k:v["accounting"]["observedClientElapsedNs"] if v["eventKind"] != "incomplete" else None for k,v in terminal.items()},
        providerRequestsObserved=str(len(provider_rows)), providerStatuses={str(s):str(sum(r["status"] == s for r in provider_rows)) for s in sorted({r["status"] for r in provider_rows})},
        providerRequestBodyBytes=str(sum(int(r["accounting"]["requestBodyBytes"]) for r in provider_rows)),
        providerResponseBodyBytesWrittenKnown=str(sum(int(v["responseBodyBytesWritten"]) for v in provider_known)),
        providerResponseWriteUnknown=str(len(provider_rows)-len(provider_known)),
        providerInternalRetries=None, httpFramingBytes=None, backendDurability=None,
        tokenizer=None, measuredTokens=None, answerContextCapTokens=None, productionCost=None,
        paidExternalModelCalls="0", paidCallBasis="declared-offline-synthetic-replay-only",
        tokenUsageStatus="unavailable-protocol-zero-is-synthetic-fixture", latencyScope="observed-client-rpc-no-nested-sum-no-performance-claim")


def track_profile(track):
    if track == COMBINED["TRACK"]:
        return dict(VERSIONS, privateControlVersion=1, combinedConsumerVersion=1), dict(CAPABILITIES, barrier=True, completeExternalAccounting=True, groupedContext=True)
    if track == NATIVE_SWEEP["TRACK"]:
        versions, capabilities = track_profile(NATIVE["TRACK"])
        return dict(versions, sweepControlVersion=1, sweepOriginalContractVersion=1, physicalCompactionContractVersion=1, nativePrefixFixtureVersion=3, inheritedPolicyAdoptionVersion=1), dict(capabilities,
            nativeSweep=True, nativePhysicalCompaction=True, sweepDecayRateMultiplier=1.0, namespaceDecayMultipliers={"foreign":0.0})
    if track == NATIVE["TRACK"]:
        versions, capabilities = track_profile(RESET["TRACK"])
        return dict(versions, canonicalSchemaVersion=2, canonicalRepresentation="strict-native-prefix", logicalHashContract=NATIVE["HASH_CONTRACT"], lifecycleVersion="legacy_access_v1"), dict(capabilities,
            canonicalSchemaAttested=True, nativeOwnedTextQueryAdmission=True, nativeWrites=False, nativeSweep=False,
            ordinaryNativeStartup=False, independentNativeGenesis=False, automaticSweep=False, autolink=False, rif="off",
            cache="fresh-per-owner-with-checkpoint-owned-restoration", provider="offline-replay-cache-fail-closed")
    if track == SWEEP["TRACK"]:
        versions, capabilities = track_profile(RESET["TRACK"])
        return dict(versions, sweepControlVersion=1), dict(capabilities, explicitLegacySweep=True, nativeSweep=False)
    if track == RESET["TRACK"]:
        return dict(VERSIONS, privateControlVersion=1, checkpointVersion=1), dict(CAPABILITIES, barrier=True, snapshot=True,
            freshOwnerCheckpointRestore=True, checkpointLogicalHash=True, clock="lifecycle-owned", clockAdvance=True,
            lifecycleClockControlled=True, canonicalAllocationControlled=True)
    if track == OWNED["TRACK"]:
        return dict(VERSIONS, privateControlVersion=1), dict(CAPABILITIES, barrier=True)
    return VERSIONS, CAPABILITIES


def fixtures(track=TRACK):
    if track == BEIR["TRACK"]:
        return BEIR["fixtures"](globals())
    if track == NATIVE_SWEEP["TRACK"]:
        return NATIVE_SWEEP["fixtures"](fixtures, NATIVE)
    if track == NATIVE["TRACK"]:
        return NATIVE["fixtures"](fixtures)
    if track == SWEEP["TRACK"]:
        return SWEEP["fixtures"](fixtures)
    if track == RESET["TRACK"]:
        return RESET["fixtures"](fixtures)
    if track == OWNED["TRACK"]:
        return OWNED["fixtures"](fixtures)
    if track == CONFORMANCE["TRACK"]:
        return CONFORMANCE["fixtures"](fixtures)
    if track not in (TRACK, ACCOUNTING_TRACK):
        raise ValueError("unsupported diagnostic track")
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


COMMON_FIELDS = ("manifestVersion", "sourceMappingVersion", "traceSchemaVersion", "track", "versions", "capabilities",
                 "sourceCommit", "sourceFilesSha256", "containerImageId", "limits", "executables", "activeEmbedding", "scoringProfile")


def validate_build(m, binary, scorer, source_root):
    if any(type(m[k]) is not int or m[k] != (2 if k == "traceSchemaVersion" and m["track"] == COMBINED["TRACK"] else 1) for k in ("manifestVersion", "sourceMappingVersion", "traceSchemaVersion")):
        raise ValueError("unsupported independent harness version")
    versions, capabilities = track_profile(m["track"])
    if m["track"] not in (TRACK, COMBINED["TRACK"], CONTEXT["TRACK"], BEIR["TRACK"], GROUPED["TRACK"], PAIRED["TRACK"], PAIRED["ARM_TRACK"], ACCOUNTING_TRACK, CONFORMANCE["TRACK"], OWNED["TRACK"], RESET["TRACK"], SWEEP["TRACK"], NATIVE["TRACK"], NATIVE_SWEEP["TRACK"]) or encoded(m["versions"]) != encoded(versions) or encoded(m["capabilities"]) != encoded(capabilities):
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
    expected_executables = {"ownedEvaluator" if m["track"] in (COMBINED["TRACK"], OWNED["TRACK"], RESET["TRACK"], SWEEP["TRACK"], NATIVE["TRACK"], NATIVE_SWEEP["TRACK"]) else "recalld": digest(binary.read_bytes()), "scorer": digest(scorer.read_bytes())}
    if m["track"] in (NATIVE["TRACK"], NATIVE_SWEEP["TRACK"]):
        expected_executables["nativePrefixFixture"] = digest(binary.with_name("native_prefix_fixture").read_bytes())
    if m["executables"] != expected_executables:
        raise ValueError("executable hash mismatch")
    profile = json.loads(subprocess.check_output([str(scorer), "--profile"], env={"PATH": "/usr/bin:/bin"}))
    if (profile["sourceCommit"] != m["sourceCommit"] or profile["containerImageId"] != m["containerImageId"] or
            profile["sourceTreeSha256"] != digest(encoded(m["sourceFilesSha256"]))):
        raise ValueError("compiled provenance mismatch")
    if encoded(m["activeEmbedding"]) != encoded(profile["activeEmbedding"]):
        raise ValueError("configuration profile mismatch")


def validate_artifacts(root, m, expected=None, prefix=""):
    artifacts = []
    for key, name in (("actorInputs", "actor-inputs.json"), ("evaluatorLabels", "evaluator-labels.json"), ("replayCache", "evaluator-cache.json"), ("evaluatorSchedule", "evaluator-schedule.json")):
        exact_keys(m[key], ("file", "sha256"))
        name = prefix + name
        p = root / name
        if m[key]["file"] != name or not p.is_file() or p.is_symlink() or not p.resolve().is_relative_to(root.resolve()) or digest(p.read_bytes()) != m[key]["sha256"]:
            raise ValueError("artifact hash/reference mismatch")
        artifacts.append(json.loads(p.read_text()))
    actor, labels, cache, schedule = artifacts
    if expected is not None:
        expected_actor, _, expected_cache, expected_schedule = expected
    elif m["track"] == CONTEXT["TRACK"]:
        expected_actor, _, expected_cache, expected_schedule = CONTEXT["fixtures"](globals(), m["contextFixture"])
    elif m["track"] == BEIR["TRACK"]:
        expected_actor, _, expected_cache, expected_schedule = BEIR["fixtures"](globals(), schedule.get("futureVariant") == "changed")
    elif m["track"] == NATIVE_SWEEP["TRACK"]:
        expected_actor, _, expected_cache, expected_schedule = NATIVE_SWEEP["fixtures"](fixtures, NATIVE, schedule.get("futureVariant") == "changed")
    elif m["track"] == NATIVE["TRACK"]:
        expected_actor, _, expected_cache, expected_schedule = NATIVE["fixtures"](fixtures, schedule.get("futureVariant") == "changed")
    else:
        expected_actor, _, expected_cache, expected_schedule = fixtures(m["track"])
    if encoded(actor) != encoded(expected_actor) or encoded(cache) != encoded(expected_cache) or encoded(schedule) != encoded(expected_schedule):
        raise ValueError("unsupported synthetic actor/cache fixture")
    exact_keys(labels, ("adapterVersion", "annotationUnit", "relevant", "labelCanary", *(("metricQrels",) if m["track"] == BEIR["TRACK"] else ())))
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
    return actor, labels, cache, schedule


def validate(manifest_path, binary, scorer, source_root, beir_reference=None, *, context_assets=None):
    # Complete input validation before creating episode output, provider, or store.
    raw = manifest_path.read_bytes()
    m = json.loads(raw)
    accounted = m.get("track") == ACCOUNTING_TRACK
    context = m.get("track") == CONTEXT["TRACK"]
    exact_keys(m, (*(("accountingPolicy",) if accounted else ()), *(("metricPolicy", "referenceBundleSha256") if m.get("track") == BEIR["TRACK"] else ()), "manifestVersion", "sourceMappingVersion", "traceSchemaVersion", "track", "versions", "capabilities",
                   "sourceCommit", "sourceFilesSha256", "containerImageId", "limits", "executables", "actorInputs",
                   "evaluatorLabels", "evaluatorSchedule", "replayCache", "activeEmbedding", "scoringProfile", "dataset", *(("contextPolicy", "contextFixture") if context else ())))
    if accounted and encoded(m["accountingPolicy"]) != encoded(ACCOUNTING_POLICY):
        raise ValueError("unsupported accounting/token/context/cost policy")
    validate_build(m, binary, scorer, source_root)
    artifacts = validate_artifacts(manifest_path.parent, m)
    if m["track"] == BEIR["TRACK"]:
        actor, labels, cache, schedule = artifacts
        BEIR["preflight"](m, labels, actor, schedule, beir_reference)
    if context:
        CONTEXT["preflight"](m, artifacts[3], context_assets)
    return m, *artifacts, digest(raw)


def build_manifest(args, build):
    return dict(manifestVersion=1, sourceMappingVersion=1, traceSchemaVersion=1, track=TRACK, versions=VERSIONS,
        capabilities=CAPABILITIES, sourceCommit=build["sourceCommit"], sourceFilesSha256=build["sourceFilesSha256"],
        containerImageId=build["containerImageId"], limits=dict(cpus=2, memoryBytes=4294967296),
        executables=dict(recalld=digest(args.binary.read_bytes()), scorer=digest(args.scorer.read_bytes())),
        activeEmbedding=json.loads(subprocess.check_output([str(args.scorer), "--profile"]))["activeEmbedding"], scoringProfile="synthetic-unique-source-v1")


def prepare(args):
    if getattr(args, "track", TRACK) == COMBINED["TRACK"]:
        return COMBINED["prepare"](args, globals())
    if getattr(args, "track", TRACK) == PAIRED["TRACK"]:
        return PAIRED["prepare"](args, globals())
    if getattr(args, "track", TRACK) == GROUPED["TRACK"]:
        return GROUPED["prepare"](args, globals())
    reference_sha = BEIR["verify_reference"](getattr(args, "beir_reference", None)) if args.track == BEIR["TRACK"] else None
    context = getattr(args, "track", TRACK) == CONTEXT["TRACK"]
    if context:
        profile, kind = CONTEXT["PROFILES"][args.context_profile]
        CONTEXT["TOKEN"]["load"](args.context_assets)
    args.out.mkdir()
    build = json.loads(args.build.read_text())
    actor, labels, cache, schedule = CONTEXT["fixtures"](globals(),kind) if context else fixtures(args.track)
    m = dict(manifestVersion=1, sourceMappingVersion=1, traceSchemaVersion=1, track=args.track, versions=track_profile(args.track)[0],
             capabilities=track_profile(args.track)[1], sourceCommit=build["sourceCommit"], sourceFilesSha256=build["sourceFilesSha256"],
             containerImageId=build["containerImageId"], limits=dict(cpus=2, memoryBytes=4294967296),
             executables={"ownedEvaluator" if args.track in (COMBINED["TRACK"], OWNED["TRACK"], RESET["TRACK"], SWEEP["TRACK"], NATIVE["TRACK"], NATIVE_SWEEP["TRACK"]) else "recalld": digest(args.binary.read_bytes()), "scorer": digest(args.scorer.read_bytes())},
             activeEmbedding=json.loads(subprocess.check_output([str(args.scorer), "--profile"]))["activeEmbedding"], scoringProfile="synthetic-unique-source-v1")
    if context: m.update(track=CONTEXT["TRACK"],contextPolicy=CONTEXT["TOKEN"]["policy"](profile),contextFixture=kind)
    if args.track == BEIR["TRACK"]:
        m.update(metricPolicy=BEIR["POLICY"], referenceBundleSha256=reference_sha)
    if getattr(args, "track", TRACK) == ACCOUNTING_TRACK:
        m.update(track=ACCOUNTING_TRACK, accountingPolicy=ACCOUNTING_POLICY)
    if args.track in (NATIVE["TRACK"], NATIVE_SWEEP["TRACK"]):
        m["executables"]["nativePrefixFixture"] = digest(args.binary.with_name("native_prefix_fixture").read_bytes())
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
        if getattr(self.server, "accounted", False):
            return self.accounted_post()
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
            log.write(json.dumps(dict(requestBody=raw.decode(), status=status, environmentHash=self.server.environment, deliveryOrdinal=self.server.delivery, cacheKeys=[self.server.key(t) for t in json.loads(raw).get("input", [])], **({"responseVectors": vectors if status == 200 else None} if self.server.owned or getattr(self.server, "grouped", False) else {}))) + "\n")
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def accounted_post(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        status, request, vectors, category = 200, {}, [], "cache_hit"
        try:
            request = json.loads(raw)
            if self.path != "/v1/embeddings" or request["model"] != MODEL or request["dimensions"] != 4 or request["encoding_format"] != "float":
                raise ValueError("unsupported profile")
            vectors = [self.server.entries[self.server.key(text)] for text in request["input"]]
            body = dict(data=[dict(index=i, embedding=v) for i, v in enumerate(vectors)], usage=dict(prompt_tokens=0, total_tokens=0))
        except (ValueError, KeyError, TypeError):
            status, category, body = 400, "replay_miss_or_invalid_request", dict(error=dict(message="offline replay miss; no fallback"))
        data = json.dumps(body).encode(); written = None
        try:
            self.send_response(status); self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data))); self.end_headers()
            count = self.wfile.write(data)
            if type(count) is int and 0 <= count <= len(data): written = str(count)
        finally:
            inputs = request.get("input", []) if isinstance(request, dict) else []
            with self.server.log.open("a") as log:
                log.write(json.dumps(dict(requestBody=raw.decode("utf-8", errors="replace"), status=status,
                    environmentHash=self.server.environment, deliveryOrdinal=self.server.delivery,
                    cacheKeys=[self.server.key(t) for t in inputs if isinstance(t, str)],
                    responseVectors=vectors if status == 200 else None,
                    accounting=dict(accountingVersion=1, requestBodyHex=raw.hex(), requestBodyBytes=str(len(raw)),
                        responseBodyHex=data.hex(), responseBodyBytesIntended=str(len(data)), responseBodyBytesWritten=written,
                        returnedVectors=str(len(vectors)) if status == 200 else "0", category=category,
                        tokenUsageStatus="synthetic-fixture-not-measured", framingBytes=None))) + "\n")
                log.flush()


def run_episode(args, prepared=None):
    if prepared is None and json.loads(args.manifest.read_bytes()).get("track") == COMBINED["TRACK"]:
        return COMBINED["run"](args, globals())
    if prepared is None and json.loads(args.manifest.read_bytes()).get("track") == PAIRED["TRACK"]:
        return PAIRED["run"](args, globals())
    if prepared is None and json.loads(args.manifest.read_bytes()).get("track") == GROUPED["TRACK"]:
        return GROUPED["run"](args, globals())
    if prepared is None and json.loads(args.manifest.read_text()).get("track") == NATIVE_SWEEP["TRACK"]:
        return NATIVE["run"](args, globals(), NATIVE_SWEEP)
    if prepared is None and json.loads(args.manifest.read_text()).get("track") == NATIVE["TRACK"]:
        return NATIVE["run"](args, globals())
    if prepared is None and json.loads(args.manifest.read_text()).get("track") in (RESET["TRACK"], SWEEP["TRACK"]):
        return RESET["run"](args, globals())
    m, actor, labels, cache, schedule, environment = prepared or validate(args.manifest, args.binary, args.scorer, args.source_root, beir_reference=getattr(args, "beir_reference", None), context_assets=getattr(args, "context_assets", None))
    combined = m["track"] == COMBINED["TRACK"]
    owned = combined or m["track"] == OWNED["TRACK"]
    beir = m["track"] == BEIR["TRACK"]
    scripted = (owned and not combined) or beir or m["track"] == CONFORMANCE["TRACK"]
    conformance = scripted and not owned and not beir
    adapter = OWNED if owned else CONFORMANCE
    args.out.mkdir()
    evaluator = args.out / "evaluator"
    evaluator.mkdir()
    ingest_projection = dict(phase="ingest", sources=actor["sources"][:4] if owned and not combined else actor["sources"][:3] if conformance else actor["sources"], activeEmbedding=m["activeEmbedding"], startId=0, deliveryOrdinal=0)
    save(args.out / "actor-projection.json", ingest_projection)
    (args.out / "actor-inputs").mkdir()
    # Cache server receives only the actor cache projection, never evaluator labels.
    stub = http.server.HTTPServer(("127.0.0.1", 0), Replay)
    accounted = combined or m["track"] == ACCOUNTING_TRACK
    trace = COMBINED["recorder"](globals(), args.out / "trace.jsonl", environment, m["groupContext"], m["activeEmbedding"]) if combined else TraceRecorder(args.out / "trace.jsonl", environment, args.out.name, m["activeEmbedding"]) if accounted else None
    stub.accounted = accounted
    stub.owned = owned
    stub.grouped = "groupContext" in m
    stub.log, stub.environment, stub.delivery = args.out / "actor-provider.jsonl", environment, 0
    stub.key = lambda text: digest(encoded(dict(environmentHash=environment, model=MODEL, dimension=4, prefixedInput=text)))
    stub.entries = {stub.key(text): vector for text, vector in cache["entries"].items() if text.startswith("a6-document:") and (not conformance or text.removeprefix("a6-document:") in {CONFORMANCE["document_text"](s) for s in actor["sources"][:3]} or text == "a6-document:health check")}
    threading.Thread(target=stub.serve_forever, daemon=True).start()
    result = dict(status="running", environmentHash=environment, capabilities=m["capabilities"])
    control = None
    control_fds = []
    process = None
    runtime_failure = None
    process_events = []
    public_ids = {}
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
api_key_env = "{'RECALLD_EVAL_OFFLINE_KEY' if owned else 'A6_SYNTHETIC_KEY'}"
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
            if owned:
                env.pop("A6_SYNTHETIC_KEY")
                env.pop("RECALLD_DAEMON_SOCKET")
                env["RECALLD_EVAL_OFFLINE_KEY"] = "offline-fixture"
            with (args.out / "runtime.log").open("wb") as runtime_log, (args.out / "actor-wire.jsonl").open("wb") as wire, (args.out / "actor.log").open("wb") as actor_log:
                def launch():
                    nonlocal control, process
                    if owned:
                        request_read, request_write = os.pipe()
                        response_read, response_write = os.pipe()
                        control_fds.extend((request_read, request_write, response_read, response_write))
                        command = [str(args.binary), "--config", str(root / "config.toml"), "--data-root", str(root / "data"),
                                   "--control-read-fd", str(request_read), "--control-write-fd", str(response_write)]
                        child = subprocess.Popen(command, cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=runtime_log, bufsize=0, pass_fds=(request_read, response_write))
                        process = child  # cleanup still owns child if handshake fails
                        os.close(request_read); control_fds.remove(request_read)
                        os.close(response_write); control_fds.remove(response_write)
                        control = OWNED["Control"](response_read, request_write, evaluator / "control-wire.jsonl")
                        control.handshake()
                        initial_receipt = control.checkpoint(-1, 0)
                        if combined:
                            trace.barrier(initial_receipt)
                            COMBINED["control_negatives"](control, initial_receipt)
                        process_events.append(dict(pid=child.pid, dataRoot=str(root / "data"), exitCode=None))
                        save(evaluator / "process-events.json", process_events)
                        return child
                    child = subprocess.Popen([str(args.binary), "--config", str(root / "config.toml"), "--data-dir", str(root / "data"), "mcp"],
                                             cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=runtime_log, bufsize=0)
                    process_events.append(dict(pid=child.pid, dataRoot=str(root / "data"), exitCode=None))
                    save(evaluator / "process-events.json", process_events)
                    return child
                def stop():
                    process.stdin.close()
                    process.wait(timeout=20)
                    process_events[-1]["exitCode"] = process.returncode
                    save(evaluator / "process-events.json", process_events)
                    if process.returncode != 0:
                        raise RuntimeError("production MCP child failed during EOF shutdown")
                process = launch()
                actor_path = root / "actor.py"
                shutil.copyfile(Path(__file__).with_name("public-mcp-actor.py"), actor_path)
                shutil.copyfile(actor_path, args.out / "actor-code.py")
                result["actorCodeSha256"] = digest(actor_path.read_bytes())
                assert digest(actor_path.read_bytes()) == m["sourceFilesSha256"]["scripts/public-mcp-actor.py"]
                if "contextPolicy" in m:
                    for name in ("context_tokenizer.py","context-tokenizer-lock.json"):
                        shutil.copyfile(Path(__file__).with_name(name),root/name)
                        assert digest((root/name).read_bytes())==m["sourceFilesSha256"]["scripts/"+name]
                start_id = 0
                # The supervisor owns this schedule. Each actor process sees only its current question.
                for ordinal in range(len(schedule["steps"]) + 1 if scripted else len(schedule["queries"]) + 2):
                    stub.delivery = ordinal
                    if ordinal == 0:
                        projection = ingest_projection
                    elif scripted:
                        step = schedule["steps"][ordinal - 1]
                        if step.get("restart"):
                            stop()
                            process = launch()
                            projection = dict(phase="connect", startId=start_id, deliveryOrdinal=ordinal, activeEmbedding=m["activeEmbedding"])
                        else:
                            if "waitSeconds" in step:
                                time.sleep(step["waitSeconds"])  # Real elapsed time; no frozen clock/control.
                            projection = adapter["projection"](step, public_ids, start_id, ordinal)
                            arguments = projection["currentAction"]["arguments"]
                            text = ("a6-query:" + arguments["query"] if "query" in arguments else
                                    "a6-document:" + CONFORMANCE["document_text"](arguments) if "summary" in arguments else None)
                            if text in cache["entries"]:
                                stub.entries[stub.key(text)] = cache["entries"][text]
                    else:
                        miss = ordinal == len(schedule["queries"]) + 1
                        question = dict(queryId="replay-miss", text="unrecorded-query") if miss else schedule["queries"][ordinal - 1]
                        if not miss:
                            text = "a6-query:" + question["text"]
                            # Load only the current frozen cache entry, never generate on a miss.
                            if not combined or text in cache["entries"]: stub.entries[stub.key(text)] = cache["entries"][text]
                        projection = dict(phase="query", startId=start_id, deliveryOrdinal=ordinal,
                                          currentQuestion=question, expectedMiss=miss)
                    if "contextPolicy" in m and ordinal>0: projection["contextPolicy"]=m["contextPolicy"]
                    if combined and ordinal>0: projection["retrievalLimit"] = 20
                    save(args.out / "actor-inputs" / f"step-{ordinal}.json", projection)
                    if accounted:
                        child = run_accounted_actor([sys.executable, str(actor_path), *(["--context-bundle", str(args.context_assets), "--combined-context", *[arg for fd in control_fds for arg in ("--deny-control-fd", str(fd))]] if combined else []), *(["--retrieval-limit", str(m["pairedArm"]["retrievalLimit"])] if "pairedArm" in m else []), "--accounting", "--write-fd", str(process.stdin.fileno()),
                            "--read-fd", str(process.stdout.fileno()), "--deny-fd", str(guard_read)], projection, wire, actor_log, root,
                            dict(PATH="/usr/bin:/bin", HOME=str(root / "home"), TZ="UTC"), (process.stdin.fileno(), process.stdout.fileno()), trace)
                    else:
                        child = subprocess.run([sys.executable, str(actor_path), *(["--context-bundle", str(args.context_assets)] if "contextPolicy" in m else []), *(["--retrieval-limit", str(m["pairedArm"]["retrievalLimit"])] if "pairedArm" in m else []), "--write-fd", str(process.stdin.fileno()), "--read-fd", str(process.stdout.fileno()), "--deny-fd", str(guard_read), *[arg for fd in control_fds for arg in ("--deny-control-fd", str(fd))]],
                            input=encoded(projection), stdout=wire, stderr=actor_log, cwd=root,
                            env=dict(PATH="/usr/bin:/bin", HOME=str(root / "home"), TZ="UTC", PYTHONDONTWRITEBYTECODE="1", PYTHONNOUSERSITE="1"),
                            pass_fds=(process.stdin.fileno(), process.stdout.fileno()), timeout=90)
                    wire.flush()
                    delivered = rows(args.out / "actor-wire.jsonl")
                    acknowledgments = [r for r in delivered if r["event"] == "outcome"]
                    if child.returncode != 0:
                        runtime_failure = f"actor exited {child.returncode} at delivery {ordinal}"
                        break
                    assert acknowledgments and int(acknowledgments[-1]["stepId"]) > start_id
                    start_id = int(acknowledgments[-1]["stepId"])
                    for ack in acknowledgments:
                        if ack["tag"].startswith("store:") and not ack["response"]["result"].get("isError"):
                            public_ids[ack["tag"][6:]] = json.loads(ack["response"]["result"]["content"][0]["text"])["id"]
                    if control is not None:
                        receipt = control.checkpoint(ordinal, start_id)
                        if combined: trace.barrier(receipt)
                        save(evaluator / "readiness-receipts.json", control.receipts)
                    with (evaluator / "schedule-ledger.jsonl").open("a") as log:
                        log.write(json.dumps(dict(deliveryOrdinal=ordinal, acknowledgedThroughStepId=str(start_id))) + "\n")
                stop()
                if runtime_failure is None and process.returncode != 0:
                    runtime_failure = f"actor/runtime exited {child.returncode}/{process.returncode}"
            if not owned and "falling back to direct mode" not in (args.out / "runtime.log").read_text():
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
                elif row["event"] == "outcome":
                    tool = row["response"]["result"]
                    if tool.get("isError"):
                        events.append(dict(sourceId=source_id, status="rejected", reason="public-tool-error"))
                    else:
                        memory = json.loads(tool["content"][0]["text"])["id"]
                        assert memory not in mappings
                        mappings[memory] = [source_id]
                        events.append(dict(sourceId=source_id, status="accepted", reason="public-success-no-readiness-claim", memoryId=memory))
            elif row["tag"].startswith("query:") and row["event"] == "outcome":
                response = row["response"]
                if response.get("error") is not None or response.get("result", {}).get("isError"):
                    # Retain the primary public error and finish writing raw evidence.
                    # The final failure below prevents this page from being scored.
                    runtime_failure = f"public {row['tag']} failed: {json.dumps(response)}"
                    continue
                value = json.loads(response["result"]["content"][0]["text"])
                returned = value["memories"] + (value.get("graphContext", {}).get("neighbors", []) if owned else [])
                queries.append(dict(queryId=row["tag"][6:], memoryIds=[r["id"] for r in returned]))
        event_text = "".join(json.dumps(e) + "\n" for e in events)
        (evaluator / "source-events.jsonl").write_text(event_text)
        save(args.out / "source-mappings.json", dict(sourceMappingVersion=1, actorArtifactSha256=m["actorInputs"]["sha256"], mappings=mappings, sources=actor["sources"]))
        if "groupContext" in m:
            save(evaluator / "group-context.json", m["groupContext"])
        if not accounted:
            with (args.out / "trace.jsonl").open("w") as legacy_trace:
                for row in outcomes:
                    legacy_trace.write(json.dumps(dict(traceSchemaVersion=1, runId=environment, episodeId=args.out.name, groupId=m.get("groupContext", {}).get("groupId", "synthetic-regression"),
                        stepId=row["stepId"], environmentHash=environment, action=row["request"], outcome=row["response"],
                        readGeneration=None, readiness=(dict(available=True, scope="post-delivery-barrier-not-response-readview", receiptFile="evaluator/readiness-receipts.json", deliveryOrdinal=row["deliveryOrdinal"]) if owned else dict(available=False)), simulatedTimeMs=None, snapshot=None, reward=None)) + "\n")
        if runtime_failure:
            result.update(actorActions=len(outcomes), storeMappings=len(mappings))
            raise RuntimeError(runtime_failure)
        score_input = dict(profile=m["scoringProfile"], environmentHash=environment, sourceIds=[s["sourceId"] for s in actor["sources"]], mappings=mappings,
                           relevant=labels["relevant"], queries=queries, sourceEvents=event_text, sourceEventsSha256=digest(event_text.encode()))
        if beir:
            BEIR["consume"](args, globals(), m, actor, labels, schedule, wire, score_input)
        save(evaluator / "score-input.json", score_input)
        subprocess.run([str(args.scorer), "--input", str(evaluator / "score-input.json"), "--out", str(evaluator / "scores.json")], check=True)
        if "contextPolicy" in m: CONTEXT["consume"](args,globals(),m,schedule,wire,score_input)
        for name in ("actor-projection.json", "actor-wire.jsonl", "actor-provider.jsonl", "source-mappings.json", "trace.jsonl"):
            if labels["labelCanary"].encode() in (args.out / name).read_bytes():
                raise RuntimeError("evaluator canary leaked to actor channel")
        providers = rows(args.out / "actor-provider.jsonl")
        assert sum(r["status"] == 400 for r in providers) == 1
        assert any(r["tag"] == "replay-miss" and r["response"]["result"]["isError"] for r in outcomes)
        canary = schedule["futureQueryCanary"]
        assert canary not in (args.out / "actor-code.py").read_text()
        assert labels["labelCanary"] not in (args.out / "actor-code.py").read_text()
        canary_ordinal = next(i for i in range(len(schedule["steps"]) + 1) if canary in (args.out / "actor-inputs" / f"step-{i}.json").read_text()) if scripted else 4
        for ordinal in range(canary_ordinal):
            assert canary not in (args.out / "actor-inputs" / f"step-{ordinal}.json").read_text()
        assert canary in (args.out / "actor-inputs" / f"step-{canary_ordinal}.json").read_text()
        assert all(canary not in json.dumps(row) for row in wire if row["deliveryOrdinal"] < canary_ordinal)
        assert all(canary not in json.dumps(row) for row in providers if row["deliveryOrdinal"] < canary_ordinal)
        assert len(rows(evaluator / "schedule-ledger.jsonl")) == (len(schedule["steps"]) + 1 if scripted else 6)
        for projection_path in (args.out / "actor-inputs").glob("*.json"):
            assert labels["labelCanary"] not in projection_path.read_text()
        if combined:
            COMBINED["verify_episode"](args, globals(), m, schedule, wire, control.receipts)
        if owned and not combined:
            report = OWNED["verify"](wire, schedule, mappings, control.receipts)
            save(evaluator / "owned-conformance.json", report)
        if conformance:
            report = CONFORMANCE["verify"](wire, schedule, mappings, actor["sources"], process_events)
            save(evaluator / "conformance.json", report)
        result.update(futureQueryCanaryWithheldUntilDelivery=True, status="passed", actorActions=len(outcomes), expectedProviderMisses=1, storeMappings=len(mappings))
    except BaseException as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        for fd in control_fds:
            os.close(fd)
        os.close(guard_read)
        os.close(guard_write)
        stub.shutdown()
        stub.server_close()
        accounting_error = None
        if trace is not None:
            trace.close()
            try:
                save(evaluator / "accounting.json", COMBINED["reconcile"](globals(), rows(args.out / "trace.jsonl"), rows(stub.log) if stub.log.exists() else []) if combined else reconcile_accounting(rows(args.out / "trace.jsonl"), rows(stub.log) if stub.log.exists() else []))
            except BaseException as error:
                accounting_error = error; result.update(status="failed", accountingError=str(error))
        result["artifactsSha256"] = {str(p.relative_to(args.out)): digest(p.read_bytes()) for p in sorted(args.out.rglob("*")) if p.is_file() and p.name != "result.json"}
        save(args.out / "result.json", result)
        if accounting_error is not None: raise RuntimeError("accounting reconciliation failed") from accounting_error


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
    if args.track == CONFORMANCE["TRACK"]:
        script = script.replace("cargo test --offline --locked --examples", "cargo test --offline --locked --example score_mcp_episode")
        script = script.replace(" prepare --binary", " prepare --track " + args.track + " --binary")
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
    p.add_argument("--track", choices=(TRACK, COMBINED["TRACK"], CONTEXT["TRACK"], BEIR["TRACK"], GROUPED["TRACK"], PAIRED["TRACK"], ACCOUNTING_TRACK, CONFORMANCE["TRACK"], OWNED["TRACK"], RESET["TRACK"], SWEEP["TRACK"], NATIVE["TRACK"], NATIVE_SWEEP["TRACK"]), default=TRACK)
    for flag in ("binary", "scorer", "build", "out"): p.add_argument("--" + flag, type=Path, required=True)
    p.add_argument("--beir-reference", type=Path)
    p.add_argument("--combined-variant", choices=COMBINED["VARIANTS"], default="baseline")
    p.add_argument("--context-profile", choices=tuple(CONTEXT["PROFILES"]), default="default")
    p.add_argument("--context-assets", type=Path)
    p = sub.add_parser("run")
    for flag in ("binary", "scorer", "source-root", "manifest", "out"): p.add_argument("--" + flag, type=Path, required=True)
    p.add_argument("--beir-reference", type=Path)
    p.add_argument("--context-assets", type=Path)
    p = sub.add_parser("host")
    p.add_argument("--track", choices=(TRACK, CONFORMANCE["TRACK"]), default=TRACK)
    p.add_argument("--cache", type=Path, required=True); p.add_argument("--build-cache", type=Path, required=True); p.add_argument("--library-evidence", type=Path)
    args = parser.parse_args()
    {"prepare": prepare, "run": run_episode, "host": host}[args.mode](args)


if __name__ == "__main__":
    main()
