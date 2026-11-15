"""Source-only grouping and cohort dispatch for the existing synthetic MCP runner."""
import copy
import json
from pathlib import Path
import subprocess
from types import SimpleNamespace

TRACK = "serialized-mcp-grouped-synthetic-diagnostic-v1"
PROFILE = "synthetic-grouped-unique-source-v1"
DEDUP = "exact-role-namespace-content-v1"
ARTIFACTS = (("actorInputs", "actor-inputs.json"), ("evaluatorLabels", "evaluator-labels.json"),
             ("replayCache", "evaluator-cache.json"), ("evaluatorSchedule", "evaluator-schedule.json"))


def registry(h):
    episodes = []
    for i, token in enumerate(("oak", "oak", "pine", "cedar", "elm", "ash", "fir")):
        sources = []
        for ordinal, (word, namespace) in enumerate((("amber", "default"), ("birch", "default"), ("olive", "default"), ("foreign", "foreign"))):
            text = word + " " + token
            sources.append(dict(sourceId=f"turn-{ordinal}", conversationId=f"conversation-{i}", sessionId=f"session-{i}",
                turnId=f"turn-{ordinal}", role="user", sourceTimestamp=1767225600000, ordinal=ordinal,
                summary=text, namespace=namespace, contentSha256=h["digest"](text.encode())))
        episodes.append(dict(episodeId=f"episode-{i}", userIds=[f"user-{i}"], conversationIds=[f"conversation-{i}"],
            evidenceHistoryIds=[f"history-{i}"], aliases=["declared-shared-history"] if i in (2, 3) else [], sources=sources))
    return dict(registryVersion=1, dedupMethod=DEDUP, episodes=episodes)


def maps(value, h):
    h["exact_keys"](value, ("registryVersion", "dedupMethod", "episodes"))
    if type(value["registryVersion"]) is not int or value["registryVersion"] != 1 or value["dedupMethod"] != DEDUP:
        raise ValueError("unsupported source grouping method")
    if not isinstance(value["episodes"], list) or not value["episodes"]:
        raise ValueError("empty episode registry")
    members, keys, fingerprints = {}, {}, {}
    for episode in value["episodes"]:
        h["exact_keys"](episode, ("episodeId", "userIds", "conversationIds", "evidenceHistoryIds", "aliases", "sources"))
        identity = episode["episodeId"]
        if not isinstance(identity, str) or not identity or not all(c.isascii() and (c.isalnum() or c in "-_") for c in identity) or identity in members:
            raise ValueError("duplicate or invalid episode identity")
        members[identity] = {identity}
        for kind in ("userIds", "conversationIds", "evidenceHistoryIds", "aliases"):
            values = episode[kind]
            if not isinstance(values, list) or any(not isinstance(v, str) or not v for v in values) or len(values) != len(set(values)):
                raise ValueError("invalid grouping relation")
            for token in values:
                keys.setdefault((kind, token), set()).add(identity)
        sources = episode["sources"]
        if not isinstance(sources, list) or not sources:
            raise ValueError("empty source history")
        ids, ordinals, history = set(), set(), []
        for source in sources:
            h["exact_keys"](source, ("sourceId", "conversationId", "sessionId", "turnId", "role", "sourceTimestamp", "ordinal", "summary", "namespace", "contentSha256"))
            if any(not isinstance(source[k], str) or not source[k] for k in ("sourceId", "conversationId", "sessionId", "turnId", "role", "summary", "namespace")):
                raise ValueError("invalid source identity/content")
            if type(source["ordinal"]) is not int or type(source["sourceTimestamp"]) is not int or source["ordinal"] < 0:
                raise ValueError("invalid source ordinal/time")
            if source["sourceId"] in ids or source["ordinal"] in ordinals or h["digest"](source["summary"].encode()) != source["contentSha256"]:
                raise ValueError("duplicate source or content hash mismatch")
            ids.add(source["sourceId"]); ordinals.add(source["ordinal"])
            history.append((source["ordinal"], [source["role"], source["namespace"], source["summary"]]))
        if ordinals != set(range(len(sources))):
            raise ValueError("source ordinals must be contiguous")
        fingerprint = h["digest"](h["encoded"]([row for _, row in sorted(history)]))
        fingerprints[identity] = fingerprint
        keys.setdefault(("exactHistory", fingerprint), set()).add(identity)
    # Small fixed contract cohort: merge sets to a fixed point, with no mutable
    # representative/iteration-order dependency in the emitted component IDs.
    for related in keys.values():
        component = set().union(*(members[identity] for identity in related))
        for identity in component:
            members[identity] = component
    components = sorted({tuple(sorted(v)) for v in members.values()})
    groups = {h["digest"](b"a6-group-v1\0" + h["encoded"](list(v))): list(v) for v in components}
    by_episode = {episode: group for group, values in groups.items() for episode in values}
    group_map = dict(groupMapVersion=1, dedupMethod=DEDUP, groups=dict(sorted(groups.items())),
                     episodes=dict(sorted(by_episode.items())), historySha256ByEpisode=dict(sorted(fingerprints.items())))
    ordered = sorted(groups, key=lambda group: (h["digest"](("a6-split-v1" + group).encode()), group))
    train, development = len(ordered) * 60 // 100, len(ordered) * 20 // 100
    assignments = {group: "train" if i < train else "development" if i < train + development else "test" for i, group in enumerate(ordered)}
    split_map = dict(splitMapVersion=1, method="a6-split-v1", rounding="floor-train-dev-remainder-test", ratios=[60, 20, 20],
        groups=dict(sorted(assignments.items())), counts={split: sum(v == split for v in assignments.values()) for split in ("train", "development", "test")})
    return group_map, split_map


def episode_fixture(episode, h):
    sources = sorted(copy.deepcopy(episode["sources"]), key=lambda s: s["ordinal"])
    actor = dict(adapterVersion=1, sources=sources)
    question = sources[0]["summary"]
    schedule = dict(scheduleVersion=1, queries=[dict(queryId=f"q{i}", text=question if i < 3 else question + " FUTURE_QUESTION_CANARY") for i in range(4)],
                    futureQueryCanary="FUTURE_QUESTION_CANARY")
    index = int(episode["episodeId"].rsplit("-", 1)[1])
    relevant = ({"q0": ["turn-0"], "q1": [], "q3": ["turn-0", "turn-3"]},
                {"q0": ["turn-3"], "q1": ["turn-3"], "q2": []},
                {"q0": ["turn-0"], "q3": ["turn-0"]},
                {"q0": ["turn-3"], "q3": ["turn-0", "turn-3"]},
                {"q0": ["turn-0"]}, {}, {f"q{i}": [] for i in range(4)})[index]
    labels = dict(adapterVersion=1, annotationUnit="synthetic-unit", relevant=relevant, labelCanary="EVALUATOR_LABEL_CANARY_ONLY")
    vectors = ([1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [1., 0., 0., 0.])
    cache = dict(model=h["MODEL"], dimension=4, entries={"a6-document:" + s["summary"]: v for s, v in zip(sources, vectors)})
    cache["entries"]["a6-document:health check"] = [0., 0., 0., 1.]
    cache["entries"].update({"a6-query:" + q["text"]: vectors[0] for q in schedule["queries"]})
    return actor, labels, cache, schedule


def reference(root, name, value, h):
    h["save"](root / name, value)
    return dict(file=name, sha256=h["digest"]((root / name).read_bytes()))


def load(root, ref, name, h):
    h["exact_keys"](ref, ("file", "sha256"))
    path = root / name
    if ref["file"] != name or path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(root.resolve()) or h["digest"](path.read_bytes()) != ref["sha256"]:
        raise ValueError("grouped artifact hash/reference mismatch")
    return json.loads(path.read_bytes())


def prepare(args, h):
    sources = registry(h)
    group_map, split_map = maps(sources, h)  # Must precede cache/label construction.
    build = json.loads(args.build.read_text())
    manifest = h["build_manifest"](args, build)
    manifest.update(track=TRACK, groupMapVersion=1, splitMapVersion=1, aggregateProfile=PROFILE,
                    executionScope="synthetic-all-splits-no-training-no-holdout-claim")
    args.out.mkdir()
    for key, name, value in (("sourceRegistry", "source-registry.json", sources), ("groupMap", "group-map.json", group_map), ("splitMap", "split-map.json", split_map)):
        manifest[key] = reference(args.out, name, value, h)
    manifest["episodes"] = []
    for episode in sorted(sources["episodes"], key=lambda v: v["episodeId"]):
        identity = episode["episodeId"]
        (args.out / "episodes" / identity).mkdir(parents=True)
        artifacts = {key: reference(args.out, f"episodes/{identity}/{name}", value, h)
                     for (key, name), value in zip(ARTIFACTS, episode_fixture(episode, h))}
        manifest["episodes"].append(dict(episodeId=identity, artifacts=artifacts))
    manifest["dataset"] = dict(kind="generated-synthetic-grouped", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
        sourceRegistrySha256=manifest["sourceRegistry"]["sha256"], intendedUse="contract-diagnostic-only")
    h["save"](args.out / "manifest.json", manifest)


def validate(args, h):
    raw = args.manifest.read_bytes(); manifest = json.loads(raw); root = args.manifest.parent
    h["exact_keys"](manifest, (*h["COMMON_FIELDS"], "groupMapVersion", "splitMapVersion", "aggregateProfile", "executionScope", "sourceRegistry", "groupMap", "splitMap", "episodes", "dataset"))
    if manifest["track"] != TRACK or any(type(manifest[k]) is not int or manifest[k] != 1 for k in ("groupMapVersion", "splitMapVersion")) or manifest["aggregateProfile"] != PROFILE or manifest["executionScope"] != "synthetic-all-splits-no-training-no-holdout-claim":
        raise ValueError("unsupported grouped profile/version")
    h["validate_build"](manifest, args.binary, args.scorer, args.source_root)
    sources = load(root, manifest["sourceRegistry"], "source-registry.json", h)
    expected_groups, expected_splits = maps(sources, h)
    expected_registry = registry(h)
    normalized = lambda value: sorted([dict(e, sources=sorted(e["sources"], key=lambda s: s["ordinal"])) for e in value["episodes"]], key=lambda e: e["episodeId"])
    if h["encoded"](normalized(sources)) != h["encoded"](normalized(expected_registry)):
        raise ValueError("unsupported synthetic source registry")
    group_map = load(root, manifest["groupMap"], "group-map.json", h)
    split_map = load(root, manifest["splitMap"], "split-map.json", h)
    if h["encoded"](group_map) != h["encoded"](expected_groups):
        raise ValueError("group map differs from source histories")
    if h["encoded"](split_map) != h["encoded"](expected_splits):
        raise ValueError("split map differs from whole-group assignments")
    if manifest["dataset"] != dict(kind="generated-synthetic-grouped", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
            sourceRegistrySha256=manifest["sourceRegistry"]["sha256"], intendedUse="contract-diagnostic-only"):
        raise ValueError("unsupported grouped dataset claim")
    by_id = {e["episodeId"]: e for e in sources["episodes"]}
    if not isinstance(manifest["episodes"], list) or len(manifest["episodes"]) != len(by_id):
        raise ValueError("incomplete episode roster")
    seen, bundles, roster = set(), [], []
    environment = h["digest"](raw)
    for entry in sorted(manifest["episodes"], key=lambda v: v["episodeId"]):
        h["exact_keys"](entry, ("episodeId", "artifacts"))
        identity = entry["episodeId"]
        if identity not in by_id or identity in seen:
            raise ValueError("unknown or duplicate episode roster member")
        seen.add(identity); h["exact_keys"](entry["artifacts"], [key for key, _ in ARTIFACTS])
        episode = dict(manifest, **entry["artifacts"])
        episode["dataset"] = dict(kind="generated-synthetic", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
            actorArtifactSha256=episode["actorInputs"]["sha256"], intendedUse="contract-diagnostic-only")
        artifacts = h["validate_artifacts"](root, episode, episode_fixture(by_id[identity], h), prefix=f"episodes/{identity}/")
        group = group_map["episodes"][identity]
        context = dict(episodeId=identity, groupId=group, split=split_map["groups"][group], queryIds=sorted(q["queryId"] for q in artifacts[3]["queries"]), groupMapSha256=manifest["groupMap"]["sha256"], splitMapSha256=manifest["splitMap"]["sha256"])
        episode["groupContext"] = context
        episode_environment = h["digest"](h["encoded"](dict(cohortEnvironmentHash=environment, **context)))
        bundles.append((episode, *artifacts, episode_environment)); roster.append({k: context[k] for k in ("episodeId", "groupId", "split", "queryIds")})
    return manifest, bundles, roster, environment


def run(args, h):
    manifest, bundles, roster, environment = validate(args, h)  # All episodes before any output/provider/store.
    args.out.mkdir(); (args.out / "episodes").mkdir()
    status = dict(status="running", environmentHash=environment, planned=len(roster), attempted=0, succeeded=0, failed=0, episodes=[dict(member, status="unattempted") for member in roster])
    scores = []
    try:
        for index, bundle in enumerate(bundles):
            identity = bundle[0]["groupContext"]["episodeId"]
            child = SimpleNamespace(**vars(args)); child.out = args.out / "episodes" / identity
            status["attempted"] += 1; status["episodes"][index]["status"] = "attempted"
            h["save"](args.out / "episode-status.json", status)
            try:
                h["run_episode"](child, prepared=bundle)
            except BaseException:
                status["failed"] += 1; status["episodes"][index]["status"] = "failed"
                raise
            status["succeeded"] += 1; status["episodes"][index]["status"] = "passed"
            raw = (child.out / "evaluator/score-input.json").read_text()
            scores.append(dict(episodeId=identity, inputJson=raw, inputSha256=h["digest"](raw.encode())))
        grouped = dict(profile=PROFILE, groupMapVersion=1, splitMapVersion=1, environmentHash=environment, episodeRoster=roster, episodes=scores)
        for key in ("groupMap", "splitMap"):
            raw = (args.manifest.parent / manifest[key]["file"]).read_text()
            grouped[key + "Json"] = raw; grouped[key + "Sha256"] = h["digest"](raw.encode())
        h["save"](args.out / "grouped-score-input.json", grouped)
        subprocess.run([str(args.scorer), "--grouped-input", str(args.out / "grouped-score-input.json"), "--out", str(args.out / "grouped-scores.json")], check=True, env=dict(PATH="/usr/bin:/bin"))
        status["status"] = "passed"
    except BaseException as error:
        status.update(status="failed", error=str(error))
        raise
    finally:
        status["unattempted"] = status["planned"] - status["attempted"]
        h["save"](args.out / "episode-status.json", status)
