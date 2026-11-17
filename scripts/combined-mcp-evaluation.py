"""Consumed trace2/group/context/barrier track using the existing public runner."""
import copy
import json
import subprocess

TRACK = "serialized-mcp-combined-control-synthetic-diagnostic-v1"
SCOPE = "synthetic-all-splits-no-training-no-holdout-claim"
VARIANTS = ("baseline", "gold", "future", "middle-cache-miss")
RPC_KINDS = {"attempt", "outcome", "error", "incomplete", "process_error"}


def registry(h):
    result = h["GROUPED"]["registry"](h)
    for episode in result["episodes"]:
        template = episode["sources"][0]
        token = template["summary"].split()[-1]
        episode["sources"] = []
        for i in range(25):
            text = f"amber {token} record {i} résumé 🌲" + (" cedar river" * 100 if i == 1 else "")
            episode["sources"].append(dict(template, sourceId=f"turn-{i}", turnId=f"turn-{i}", ordinal=i,
                namespace="foreign" if i == 24 else "default", summary=text, contentSha256=h["digest"](text.encode())))
    return result


def fixture(episode, h, variant):
    actor, labels, cache, schedule = h["GROUPED"]["episode_fixture"](episode, h)
    schedule["queries"] = [dict(queryId=f"q{i}", text="amber " + ("FUTURE_QUESTION_CANARY" + (" CHANGED" if variant == "future" else "") if i == 3 else f"question {i}")) for i in range(4)]
    cache["entries"] = {"a6-document:" + s["summary"]: [1., i / 100., 0., 0.] for i, s in enumerate(actor["sources"])}
    cache["entries"]["a6-document:health check"] = [0., 0., 0., 1.]
    cache["entries"].update({"a6-query:" + q["text"]: [1., 0., 0., 0.] for q in schedule["queries"]})
    if variant == "middle-cache-miss" and episode["episodeId"] == "episode-3":
        del cache["entries"]["a6-query:" + schedule["queries"][0]["text"]]
    if variant == "gold":
        labels["relevant"] = {"q0": ["turn-23"], "q1": [], "q3": ["turn-24"]}
        labels["labelCanary"] += "_CHANGED"
    return actor, labels, cache, schedule


def prepare(args, h):
    if args.context_profile not in ("default", "boundary") or args.combined_variant not in VARIANTS:
        raise ValueError("unsupported combined fixture")
    h["CONTEXT"]["TOKEN"]["load"](args.context_assets)
    sources = registry(h)
    groups, splits = h["GROUPED"]["maps"](sources, h)  # Before questions, labels or caches.
    manifest = h["build_manifest"](args, json.loads(args.build.read_text()))
    versions, capabilities = h["track_profile"](TRACK)
    manifest.update(track=TRACK, traceSchemaVersion=2, versions=versions, capabilities=capabilities,
        executables=dict(ownedEvaluator=h["digest"](args.binary.read_bytes()), scorer=h["digest"](args.scorer.read_bytes())),
        contextPolicy=h["CONTEXT"]["TOKEN"]["policy"](args.context_profile), contextFixture="standard",
        combinedVariant=args.combined_variant, accountingPolicy=h["ACCOUNTING_POLICY"],
        groupMapVersion=1, splitMapVersion=1, aggregateProfile=h["GROUPED"]["PROFILE"], executionScope=SCOPE)
    args.out.mkdir()
    ref = h["GROUPED"]["reference"]
    for key, name, value in (("sourceRegistry", "source-registry.json", sources), ("groupMap", "group-map.json", groups), ("splitMap", "split-map.json", splits)):
        manifest[key] = ref(args.out, name, value, h)
    manifest["episodes"] = []
    for episode in sources["episodes"]:
        identity = episode["episodeId"]
        (args.out / "episodes" / identity).mkdir(parents=True)
        artifacts = {key: ref(args.out, f"episodes/{identity}/{name}", value, h)
            for (key, name), value in zip(h["GROUPED"]["ARTIFACTS"], fixture(episode, h, args.combined_variant))}
        manifest["episodes"].append(dict(episodeId=identity, artifacts=artifacts))
    manifest["dataset"] = dict(kind="generated-synthetic-grouped", upstreamRevision=None, upstreamUrl=None,
        thirdPartyLicense=None, sourceRegistrySha256=manifest["sourceRegistry"]["sha256"], intendedUse="contract-diagnostic-only")
    h["save"](args.out / "manifest.json", manifest)


def validate(args, h):
    raw = args.manifest.read_bytes(); m = json.loads(raw); root = args.manifest.parent
    h["exact_keys"](m, (*h["COMMON_FIELDS"], "contextPolicy", "contextFixture", "combinedVariant", "accountingPolicy",
        "groupMapVersion", "splitMapVersion", "aggregateProfile", "executionScope", "sourceRegistry", "groupMap", "splitMap", "episodes", "dataset"))
    if (m["track"] != TRACK or m["combinedVariant"] not in VARIANTS or m["contextFixture"] != "standard" or
        m["executionScope"] != SCOPE or m["aggregateProfile"] != h["GROUPED"]["PROFILE"] or
        any(type(m[k]) is not int or m[k] != 1 for k in ("groupMapVersion", "splitMapVersion")) or
        h["encoded"](m["accountingPolicy"]) != h["encoded"](h["ACCOUNTING_POLICY"])):
        raise ValueError("unsupported combined policy")
    h["validate_build"](m, args.binary, args.scorer, args.source_root)
    load = h["GROUPED"]["load"]
    sources = load(root, m["sourceRegistry"], "source-registry.json", h)
    if h["encoded"](sources) != h["encoded"](registry(h)):
        raise ValueError("unsupported combined source registry")
    groups, splits = h["GROUPED"]["maps"](sources, h)
    for key, name, expected in (("groupMap", "group-map.json", groups), ("splitMap", "split-map.json", splits)):
        if h["encoded"](load(root, m[key], name, h)) != h["encoded"](expected):
            raise ValueError("source-only grouping/split mismatch")
    if m["dataset"] != dict(kind="generated-synthetic-grouped", upstreamRevision=None, upstreamUrl=None,
        thirdPartyLicense=None, sourceRegistrySha256=m["sourceRegistry"]["sha256"], intendedUse="contract-diagnostic-only"):
        raise ValueError("unsupported dataset claim")
    expected = {e["episodeId"]: e for e in sources["episodes"]}
    if not isinstance(m["episodes"], list) or len(m["episodes"]) != len(expected):
        raise ValueError("incomplete combined roster")
    seen, bundles, roster = set(), [], []
    environment = h["digest"](raw)
    for member in m["episodes"]:
        h["exact_keys"](member, ("episodeId", "artifacts"))
        identity = member["episodeId"]
        if identity not in expected or identity in seen:
            raise ValueError("unknown or duplicate episode")
        seen.add(identity)
        h["exact_keys"](member["artifacts"], [key for key, _ in h["GROUPED"]["ARTIFACTS"]])
        episode = dict(m, **member["artifacts"])
        episode["dataset"] = dict(kind="generated-synthetic", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
            actorArtifactSha256=episode["actorInputs"]["sha256"], intendedUse="contract-diagnostic-only")
        artifacts = h["validate_artifacts"](root, episode, fixture(expected[identity], h, m["combinedVariant"]), prefix=f"episodes/{identity}/")
        h["CONTEXT"]["preflight"](m, artifacts[3], args.context_assets)
        group = groups["episodes"][identity]
        context = dict(episodeId=identity, groupId=group, split=splits["groups"][group],
            queryIds=sorted(q["queryId"] for q in artifacts[3]["queries"]), groupMapSha256=m["groupMap"]["sha256"], splitMapSha256=m["splitMap"]["sha256"])
        episode["groupContext"] = context
        bundles.append((episode, *artifacts, h["digest"](h["encoded"](dict(cohortEnvironmentHash=environment, **context)))))
        roster.append({k: context[k] for k in ("episodeId", "groupId", "split", "queryIds")})
    if [r["episodeId"] for r in roster] != sorted(expected):
        raise ValueError("noncanonical cohort order")
    return m, bundles, roster, environment


def recorder(h, path, environment, context, embedding):
    class Trace(h["TraceRecorder"]):
        def __init__(self):
            super().__init__(path, environment, context["episodeId"], embedding, trace_version=2, group_id=context["groupId"])
            self.completed = {}; self.contexts = set(); self.barriers = []

        def extra(self, kind, step, ordinal, value):
            self.sequence += 1
            self.file.write(json.dumps(dict(traceSchemaVersion=2, sequence=str(self.sequence), eventKind=kind,
                runId=environment, environmentHash=environment, episodeId=context["episodeId"], groupId=context["groupId"],
                stepId=step, deliveryOrdinal=ordinal, observation=value)) + "\n")
            self.file.flush()

        def consume(self, row):
            if row["event"] == "context":
                step = row["stepId"]; prior = self.completed.get(step)
                h["exact_keys"](row, ("event", "tag", "stepId", "deliveryOrdinal", "context"))
                if (prior is None or step in self.contexts or not prior["tag"].startswith("query:") or
                    row["tag"] != "context:" + prior["tag"][6:] or row["deliveryOrdinal"] != prior["deliveryOrdinal"] or
                    prior["response"].get("error") or prior["response"].get("result", {}).get("isError")):
                    raise ValueError("unmatched/duplicate context event")
                self.contexts.add(step)
                self.extra("context", step, row["deliveryOrdinal"], row["context"])
            else:
                super().consume(row)
                if row["event"] == "outcome": self.completed[row["stepId"]] = row

        def barrier(self, receipt):
            h["OWNED"]["checked_barrier"](receipt["barrier"], receipt["target"])
            if self.barriers and receipt["deliveryOrdinal"] <= self.barriers[-1]["deliveryOrdinal"]:
                raise ValueError("duplicate or reordered barrier")
            self.barriers.append(receipt)
            self.extra("barrier", receipt["acknowledgedThroughStepId"], receipt["deliveryOrdinal"],
                dict(scope="post-delivery-barrier-not-response-readview", receipt=receipt))
    return Trace()


def reconcile(h, trace, providers):
    rpc = []; contexts = barriers = 0
    completed = {}; seen_context = set(); last_barrier = -2
    for i, row in enumerate(trace, 1):
        if type(row["traceSchemaVersion"]) is not int or row["traceSchemaVersion"] != 2 or row["sequence"] != str(i):
            raise ValueError("unsupported trace version or sequence")
        kind = row["eventKind"]
        if kind in RPC_KINDS:
            h["exact_keys"](row, ("traceSchemaVersion", "accountingVersion", "sequence", "eventKind", "runId", "episodeId", "groupId", "environmentHash", "stepId", "deliveryOrdinal", "action", "outcome", "accounting", "errorCategory", "sourceId", "sourceMappingsFile", "publicRankObservations", "providerEvidence", "readGeneration", "readiness", "simulatedTimeMs", "snapshot", "reward"))
            # Explicit RPC-only accounting projection, preserving original trace sequence.
            rpc.append(dict(row, traceSchemaVersion=1, parentTraceSequence=row["sequence"], sequence=str(len(rpc) + 1)))
            if kind == "outcome": completed[row["stepId"]] = row
        elif kind in ("context", "barrier"):
            h["exact_keys"](row, ("traceSchemaVersion", "sequence", "eventKind", "runId", "environmentHash", "episodeId", "groupId", "stepId", "deliveryOrdinal", "observation"))
            if kind == "context":
                prior = completed.get(row["stepId"])
                if prior is None or row["stepId"] in seen_context or prior["deliveryOrdinal"] != row["deliveryOrdinal"]:
                    raise ValueError("context lacks unique completed RPC")
                seen_context.add(row["stepId"]); contexts += 1
            else:
                value = row["observation"]
                h["exact_keys"](value, ("scope", "receipt"))
                if value["scope"] != "post-delivery-barrier-not-response-readview": raise ValueError("unsupported readiness scope")
                receipt = value["receipt"]
                h["OWNED"]["checked_barrier"](receipt["barrier"], receipt["target"])
                if receipt["deliveryOrdinal"] != row["deliveryOrdinal"] or row["deliveryOrdinal"] <= last_barrier or receipt["acknowledgedThroughStepId"] != row["stepId"]:
                    raise ValueError("barrier correlation/order mismatch")
                last_barrier = row["deliveryOrdinal"]; barriers += 1
        else:
            raise ValueError("unknown trace2 event kind")
    result = h["reconcile_accounting"](rpc, providers)
    result.update(traceSchemaVersion=2, contextObservations=str(contexts), barrierObservations=str(barriers),
        accountingProjection="rpc-only-parentTraceSequence-preserved", contextTokenEvidence="context-validation.json; encoding-specific; no model usage")
    return result


def control(h, read_fd, write_fd, log):
    class Controlled(h["OWNED"]["Control"]):
        def handshake(self):
            expected = h["SWEEP"]["control_capabilities"](h["RESET"]["CONTROL_CAPABILITIES"])
            if h["encoded"](self.call("capabilities", {})) != h["encoded"](expected):
                raise ValueError("unsupported combined owned-constructor capabilities")
            self.call("reset", {}, error="UNSUPPORTED_CONTROL")
            self.call("logicalHash", {}, error="UNSUPPORTED_CONTROL")

        def checkpoint(self, ordinal, acknowledged):
            before = h["RESET"]["checked_state"](h, self.call("inspect", {}))
            at = before["committed"]
            report = h["OWNED"]["checked_barrier"](self.call("barrier", dict(target=at, deadlineMs=30000)), at)
            after = h["RESET"]["checked_state"](h, self.call("inspect", {}))
            if before != after or after["committed"] != dict(storeId=report["storeId"], generation=report["committedGeneration"]):
                raise ValueError("serialized owned state changed during barrier")
            if self.receipts and (at["storeId"] != self.receipts[-1]["after"]["storeId"] or h["OWNED"]["generation"](at["generation"]) < h["OWNED"]["generation"](self.receipts[-1]["after"]["generation"])):
                raise ValueError("lineage changed or generation regressed")
            receipt = dict(deliveryOrdinal=ordinal, acknowledgedThroughStepId=str(acknowledged), target=at,
                barrier=report, after=after["committed"], runtimeState=after["runtimeState"])
            self.receipts.append(receipt)
            return receipt
    return Controlled(read_fd, write_fd, log)


def control_negatives(control, initial):
    # Actual rejected controls, not a claim of an actual blocked-worker timeout.
    for deadline in (0, 30001, True):
        control.call("barrier", dict(target=initial["target"], deadlineMs=deadline), error="INVALID_PARAMS")
    foreign = dict(initial["target"], storeId="00000000-0000-4000-8000-000000000001")
    if foreign["storeId"] == initial["target"]["storeId"]:
        foreign["storeId"] = "00000000-0000-4000-8000-000000000002"
    control.call("barrier", dict(target=foreign, deadlineMs=1), error="BARRIER_UNAVAILABLE")


def verify_episode(args, h, manifest, schedule, wire, receipts):
    if len(receipts) != len(schedule["queries"]) + 3:
        raise ValueError("missing initial/ingest/query/miss barrier")
    initial = receipts[1]["after"]  # Ingestion completed.
    if any(r["after"] != initial or r["runtimeState"] != receipts[1]["runtimeState"] for r in receipts[2:]):
        raise ValueError("false query or cache miss changed canonical generation")
    contexts = json.loads((args.out / "evaluator/context-validation.json").read_text())["queries"]
    mapping = json.loads((args.out / "source-mappings.json").read_text())["mappings"]
    for value in contexts.values():
        if any(mapping[mid] == ["turn-24"] for mid in value["consideredMemoryIds"]):
            raise ValueError("foreign namespace decoy returned")
        if len(value["consideredMemoryIds"]) != 20:
            raise ValueError("actual public20 cutoff not saturated")
    if manifest["contextPolicy"]["profile"] == "boundary" and not all(v["omittedMemoryIds"] for v in contexts.values()):
        raise ValueError("boundary context did not omit actual evidence")
    isolations = [r for r in wire if r["event"] == "isolation" and r["tag"] == "control-fds-closed"]
    if len(isolations) != 6 or any(r["fdClosed"] is not True or r["count"] != 2 for r in isolations):
        raise ValueError("missing actual private control FD denial")
    for row in wire:
        if row["event"] == "attempt" and row["request"].get("method") == "tools/call" and row["request"]["params"]["name"] == "recall_memories":
            if row["request"]["params"]["arguments"]["recordAccess"] is not False:
                raise ValueError("query is not explicitly false")
    h["save"](args.out / "evaluator/combined-proof.json", dict(status="passed", actualRequiredDerivedViews=4,
        barriers=len(receipts), queryCount=len(contexts), eligibleFixtureDefaultRecords=24, actualReturnedPerQuery=20,
        responseReadGeneration=None, readinessScope="post-delivery-barrier-not-response-readview", nativeWrites=False))


def run(args, h):
    prepared = validate(args, h)  # Entire cohort before output/provider/store.
    h["GROUPED"]["run"](args, h, prepared=prepared)
    m, _, roster, environment = prepared
    status_path = args.out / "episode-status.json"
    try:
        grouped = json.loads((args.out / "grouped-score-input.json").read_text())
        packed = copy.deepcopy(grouped)
        for member in packed["episodes"]:
            raw = (args.out / "episodes" / member["episodeId"] / "evaluator/packed-context-score-input.json").read_text()
            before = json.loads(member["inputJson"]); after = json.loads(raw)
            if [(q["queryId"]) for q in before["queries"]] != [(q["queryId"]) for q in after["queries"]]:
                raise ValueError("raw/packed query roster mismatch")
            if {k:v for k,v in before.items() if k != "queries"} != {k:v for k,v in after.items() if k != "queries"}:
                raise ValueError("raw/packed population or labels changed")
            member.update(inputJson=raw, inputSha256=h["digest"](raw.encode()))
        h["save"](args.out / "packed-grouped-score-input.json", packed)
        subprocess.run([str(args.scorer), "--grouped-input", str(args.out / "packed-grouped-score-input.json"), "--out", str(args.out / "packed-grouped-scores.json")], check=True, env=dict(PATH="/usr/bin:/bin"))
        h["save"](args.out / "combined-roster.json", dict(environmentHash=environment, episodes=roster,
            plannedQueries=sum(len(r["queryIds"]) for r in roster), rawPackedSamePopulation=True))
    except BaseException as error:
        status = json.loads(status_path.read_text()); status.update(status="failed", aggregateFailure=str(error))
        h["save"](status_path, status)
        raise
