"""Actual fresh-owner checkpoint consumer. Core owns cloning/hash/cache restoration."""
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading

TRACK = "serialized-owned-reset-synthetic-diagnostic-v1"
CHECKPOINT_TRACK = "isolated-legacy-checkpoint-v1"
CONTROL_CAPABILITIES = dict(controlVersion=1, methods=["capabilities", "inspect", "barrier", "advance", "snapshot"], publicContract="legacy-v1",
    guarantees=dict(barrier="owned-work-and-required-derived-generations", logicalHash=False, snapshot=True, reset=False,
        freshOwnerCheckpointRestore=True, controlledClock=False, lifecycleClockControlled=True,
        controlledIds=False, canonicalAllocationControlled=True, controlledRng=False))


def fixtures(base):
    actor, labels, cache, schedule = base()
    replacement = dict(actor["sources"][0], sourceId="turn-4", turnId="turn-4", ordinal=4, summary="replacement")
    import hashlib
    replacement["contentSha256"] = hashlib.sha256(b"replacement").hexdigest()
    actor["sources"].append(replacement)
    cache["entries"]["a6-document:replacement"] = [.8, .6, 0., 0.]
    cache["entries"]["a6-document:blocked-worker-canary"] = [0., 0., 0., 1.]
    cache["entries"]["a6-query:replacement"] = [.8, .6, 0., 0.]
    cache["entries"]["a6-query:replacement FUTURE_RESET_QUERY"] = [.8, .6, 0., 0.]
    # Public-legal asymmetric corpus derived from the baseline FTS regression:
    # nonempty summaries, distinct vectors and explicit namespace/limit; no tie oracle.
    import math
    corpus = [("document A", "alpha " + " ".join(["filler"]*20), [0.,0.,1.,0.]),
              ("beta beta beta", None, [1.,0.,0.,0.]), ("beta", None, [0.,1.,0.,0.])]
    for ordinal,(summary,body,vector) in enumerate(corpus,5):
        source=dict(actor["sources"][3],sourceId=f"turn-{ordinal}",turnId=f"turn-{ordinal}",ordinal=ordinal,summary=summary,
                    contentSha256=hashlib.sha256(summary.encode()).hexdigest())
        if body is not None:
            source.update(fullText=body,fullTextSha256=hashlib.sha256(body.encode()).hexdigest())
        actor["sources"].append(source)
        cache["entries"]["a6-document:"+summary+("\n\n"+body if body is not None else "")]=vector
    cache["entries"]["a6-document:foreign"]=[0.,0.,0.,1.]
    norm=math.sqrt(.9**2+.8**2+.7**2)
    cache["entries"]["a6-query:alpha beta"]=[.9/norm,.8/norm,.7/norm,0.]
    schedule = dict(scheduleVersion=1, futureQueryCanary="FUTURE_RESET_QUERY", queries=[
        dict(queryId=f"q{i}", text="replacement" if i < 3 else "replacement FUTURE_RESET_QUERY") for i in range(4)],
        warmQuestion=dict(queryId="warm", text="replacement"), advanceMillis=86400000)
    steps = [dict(tag="store:turn-4", tool="store_memory", arguments=dict(summary="replacement", namespace="default", checkDuplicates=False, supersedes="$turn-0")),
        dict(tag="observed-get", tool="get_memory", arguments=dict(id="$turn-4", namespace="default", recordAccess=True)),
        dict(tag="feedback", tool="reinforce_memory", arguments=dict(id="$turn-4", namespace="default", quality=3))]
    for question in schedule["queries"]:
        steps.append(dict(tag="query:" + question["queryId"], tool="recall_memories", arguments=dict(query=question["text"], namespace="default", recordAccess=False, depth=0, limit=1, compact=False)))
    schedule["queries"].append(dict(queryId="fts",text="alpha beta"))
    steps.append(dict(tag="query:fts",tool="recall_memories",arguments=dict(query="alpha beta",namespace="foreign",recordAccess=False,depth=0,limit=3,compact=False)))
    steps.extend([
        dict(tag="forget", tool="forget_memory", arguments=dict(id="$turn-0", namespace="default")),
        dict(tag="tombstone", tool="get_memory", arguments=dict(id="$turn-0", namespace="default", recordAccess=False)),
        dict(tag="invalid-quality", tool="reinforce_memory", arguments=dict(id="$turn-4", namespace="default", quality=0)),
        dict(tag="replay-miss", tool="recall_memories", arguments=dict(query="unrecorded-query", namespace="default", recordAccess=False, depth=0, limit=1))])
    schedule["steps"] = steps
    labels["relevant"] = {"q0": ["turn-4"], "q1": [], "q3": ["turn-4", "turn-3"], "fts":["turn-5","turn-6","turn-7"]}
    return actor, labels, cache, schedule


def checked_state(h, value):
    h["exact_keys"](value, ("committed", "logicalHash", "runtimeState", "snapshot"))
    h["OWNED"]["checked_inspect"](dict(committed=value["committed"], logicalHash=value["logicalHash"]))
    if value["snapshot"] is not False:
        raise ValueError("inspect is not a checkpoint")
    runtime = value["runtimeState"]
    h["exact_keys"](runtime, ("nowMillis", "allocation"))
    allocation = runtime["allocation"]
    h["exact_keys"](allocation, ("version", "seed", "counter", "lastMemoryMillis"))
    if (type(runtime["nowMillis"]) is not int or not 0 <= runtime["nowMillis"] < 2**48 or
        type(allocation["version"]) is not int or allocation["version"] != 1 or
        not isinstance(allocation["seed"], list) or len(allocation["seed"]) != 32 or
        any(type(x) is not int or not 0 <= x <= 255 for x in allocation["seed"]) or
        type(allocation["counter"]) is not int or not 0 <= allocation["counter"] < 2**64-1 or
        (allocation["lastMemoryMillis"] is not None and (type(allocation["lastMemoryMillis"]) is not int or not 0 <= allocation["lastMemoryMillis"] <= runtime["nowMillis"]))):
        raise ValueError("unsupported owned runtime state")
    return value


def checked_snapshot(h, value, parent, track=CHECKPOINT_TRACK):
    h["exact_keys"](value, ("manifest", "sha256", "committed", "logicalHash", "track"))
    h["OWNED"]["checked_inspect"](dict(committed=value["committed"], logicalHash=None))
    if value["track"] != track:
        raise ValueError("unsupported checkpoint track")
    for key in ("sha256", "logicalHash"):
        if not isinstance(value[key], str) or len(value[key]) != 64 or any(c not in "0123456789abcdef" for c in value[key]):
            raise ValueError("invalid checkpoint digest")
    path = Path(value["manifest"])
    if not path.is_absolute() or path.name != "manifest.json" or path.is_symlink() or not path.is_file() or path.parent.is_symlink() or path.resolve().parent.parent != parent.resolve():
        raise ValueError("checkpoint outside private fixture parent")
    if path.stat().st_size > 1024*1024 or h["digest"](path.read_bytes()) != value["sha256"]:
        raise ValueError("checkpoint receipt digest mismatch")
    return value


class Child:
    """Owns the actual child and its actor subprocesses. No clone implementation."""
    def __init__(self, h, args, root, config, output, checkpoint=None, prior=None, negative=False, native_prefix=False):
        if checkpoint is not None and prior is None and not negative:
            raise RuntimeError("restore requires a terminated and joined prior owner")
        if prior is not None and (not prior.joined or prior.process.poll() is None):
            raise RuntimeError("old owner must be terminated and joined before restore")
        self.h, self.args, self.root, self.output = h, args, root, output
        self.joined, self.actor = False, None
        self.output.mkdir()
        rr, rw = os.pipe(); sr, sw = os.pipe()
        self.control_fds = [sr, rw]
        self.stderr = (output / "runtime.log").open("wb")
        command = [str(args.binary), "--config", str(config), "--data-root", str(root), "--control-read-fd", str(rr), "--control-write-fd", str(sw)]
        if native_prefix:
            command += ["--native-prefix"]
        if checkpoint:
            command += ["--restore-checkpoint", checkpoint["manifest"], "--checkpoint-sha256", checkpoint["sha256"]]
        self.process = subprocess.Popen(command, cwd=root.parent, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr, bufsize=0,
            pass_fds=(rr,sw), env=dict(PATH="/usr/local/bin:/usr/bin:/bin", HOME=str(root.parent / "home"), TMPDIR=str(root.parent / "home"), RECALLD_EVAL_OFFLINE_KEY="offline-fixture"))
        os.close(rr); os.close(sw)
        self.control = h["OWNED"]["Control"](sr, rw, output / "control-wire.jsonl")
        self.record = dict(command=command, pid=self.process.pid, priorJoined=None if prior is None else prior.joined, negativeConstructorProbe=negative, status="started")
        h["save"](output / "process.json", self.record)

    def ready(self):
        state = checked_state(self.h, self.control.call("inspect", {}))
        ready = self.h["OWNED"]["checked_barrier"](self.control.call("barrier", dict(target=state["committed"], deadlineMs=30000)), state["committed"])
        if ready["committedGeneration"] != state["committed"]["generation"]:
            raise ValueError("serialized owner changed during readiness check")
        return state, ready

    def before_actor(self, checkpoint=None, expected_state=None, capabilities=None):
        if self.h["encoded"](self.control.call("capabilities", {})) != self.h["encoded"](CONTROL_CAPABILITIES if capabilities is None else capabilities):
            raise ValueError("unsupported reset constructor capabilities")
        self.control.call("reset", {}, error="UNSUPPORTED_CONTROL")
        state, ready = self.ready()
        snapshot = checked_snapshot(self.h, self.control.call("snapshot", dict(deadlineMs=30000)), self.root.parent)
        if snapshot["committed"] != state["committed"]:
            raise ValueError("snapshot generation changed before actor admission")
        if checkpoint and (snapshot["logicalHash"] != checkpoint["logicalHash"] or snapshot["committed"] != checkpoint["committed"] or state != expected_state):
            raise ValueError("fresh owner differs before actor admission")
        self.h["save"](self.output / "before-actor.json", dict(state=state, readiness=ready, snapshot=snapshot))

    def start_step(self, projection):
        if self.joined or self.process.poll() is not None or self.actor is not None:
            raise RuntimeError("actor admission requires one live serialized owner")
        folder=self.output / "actor-inputs"; folder.mkdir(exist_ok=True)
        self.h["save"](folder / f"step-{projection['deliveryOrdinal']}.json", projection)
        self.wire = (self.output / "actor-wire.jsonl").open("ab")
        self.actor_log = (self.output / "actor.log").open("ab")
        # Descriptor numbers are canaries only; pass_fds contains MCP ends alone.
        command = [sys.executable, str(self.root.parent / "actor.py"), "--write-fd", str(self.process.stdin.fileno()), "--read-fd", str(self.process.stdout.fileno()), "--deny-fd", str(self.control_fds[0]),
                   *[arg for fd in self.control_fds for arg in ("--deny-control-fd",str(fd))]]
        self.actor = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=self.wire, stderr=self.actor_log, cwd=self.root.parent / "home",
            env=dict(PATH="/usr/bin:/bin", HOME=str(self.root.parent / "home"), TZ="UTC"), pass_fds=(self.process.stdin.fileno(), self.process.stdout.fileno()))
        self.actor.stdin.write(self.h["encoded"](projection)); self.actor.stdin.close()

    def finish_step(self):
        code = self.actor.wait(timeout=90)
        self.wire.close(); self.actor_log.close(); self.actor = None
        if code != 0:
            raise RuntimeError(f"actor failed with {code}; no retry")
        outcomes = [r for r in self.h["rows"](self.output / "actor-wire.jsonl") if r["event"] == "outcome"]
        if not outcomes:
            raise RuntimeError("actor has no ACK")
        return outcomes[-1], outcomes

    def step(self, projection):
        self.start_step(projection)
        return self.finish_step()

    def close(self, rejected=False):
        if self.joined:
            return
        valid = True
        if self.actor is not None:
            self.actor.kill(); self.actor.wait(timeout=10); self.wire.close(); self.actor_log.close(); self.actor=None; valid=False
        os.close(self.control_fds[1])
        try:
            self.process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.process.kill(); self.process.wait(timeout=10); valid=False
        self.process.stdin.close(); self.process.stdout.close(); os.close(self.control_fds[0]); self.stderr.close()
        self.joined = True
        self.record.update(status="joined", exitCode=self.process.returncode, forced=not valid)
        self.h["save"](self.output / "process.json", self.record)
        if not valid or (self.process.returncode != 0 and not rejected):
            raise RuntimeError("owner did not shut down cleanly; episode invalid")


def run(args, h):
    # Reuse the actual manifest consumer before creating any output/provider/store.
    manifest, actor, labels, cache, schedule, environment = h["validate"](args.manifest, args.binary, args.scorer, args.source_root)
    sweep = manifest["track"] == h["SWEEP"]["TRACK"]
    capabilities = h["SWEEP"]["control_capabilities"](CONTROL_CAPABILITIES) if sweep else CONTROL_CAPABILITIES
    args.out.mkdir()
    evaluator = args.out / "evaluator"; evaluator.mkdir()
    runtime = args.out / "runtime"; runtime.mkdir(mode=0o700)
    (runtime / "home").mkdir(mode=0o700)
    actor_code = args.source_root / "scripts/public-mcp-actor.py"
    shutil.copyfile(actor_code, runtime / "actor.py")
    assert h["digest"]((runtime / "actor.py").read_bytes()) == manifest["sourceFilesSha256"]["scripts/public-mcp-actor.py"]
    entered, release = threading.Event(), threading.Event()
    import http.server
    import io
    class Replay(h["Replay"]):
        def do_POST(self):
            raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            request = json.loads(raw)
            if "a6-document:blocked-worker-canary" in request.get("input", []):
                entered.set()
                if not release.wait(30):
                    self.send_error(500, "fixture worker latch timed out")
                    return
            self.rfile = io.BytesIO(raw)
            super().do_POST()
    server = http.server.ThreadingHTTPServer(("127.0.0.1",0), Replay)
    server.owned, server.log, server.environment, server.delivery = True, args.out / "actor-provider.jsonl", environment, 0
    server.key = lambda text:h["digest"](h["encoded"](dict(environmentHash=environment, model=h["MODEL"], dimension=4, prefixedInput=text)))
    def reset_cache():
        server.entries = {server.key(text):vector for text,vector in cache["entries"].items() if text.startswith("a6-document:")}
    def allow(text):
        key = "a6-query:" + text
        if key in cache["entries"]:
            server.entries[server.key(key)] = cache["entries"][key]
    reset_cache()
    thread=threading.Thread(target=server.serve_forever); thread.start()
    config=runtime / "config.toml"
    config.write_text(f'''timezone = "UTC"
[embedding]
provider = "openai"
model_name = "{h['MODEL']}"
api_key_env = "RECALLD_EVAL_OFFLINE_KEY"
base_url = "http://127.0.0.1:{server.server_address[1]}"
dimensions = 4
document_prefix = "a6-document:"
query_prefix = "a6-query:"
[decay]
disable_sweep = true
decay_rate_multiplier = {1.0 if sweep else 0.0}
[rif]
enabled = false
[graph]
autolink_enabled = false
[cache]
warm_file_enabled = false
''')
    children, negatives, replays = [], [], []
    result=dict(status="running", environmentHash=environment, track=manifest["track"])
    def child(name, checkpoint=None, prior=None, root=None, negative=False):
        value=Child(h,args,root or runtime/name,config,evaluator/name,checkpoint,prior,negative)
        children.append(value)
        return value
    def values(outcomes):
        mappings={}
        for row in outcomes:
            if row["tag"].startswith("store:") and not row["response"]["result"].get("isError"):
                mappings[row["tag"][6:]]=json.loads(row["response"]["result"]["content"][0]["text"])["id"]
        return mappings
    def inventory(folder):
        # Verify immutable fixture bytes; never call this a logical hash/snapshot.
        return {str(p.relative_to(folder)):h["digest"](p.read_bytes()) for p in folder.rglob("*") if p.is_file() and not p.is_symlink()}
    def unchanged(folder, before):
        assert all((folder/name).is_file() and h["digest"]((folder/name).read_bytes()) == sha for name,sha in before.items()), "source canary changed"
    def reject(name, checkpoint, destination=None):
        candidate=child("negative-"+name,checkpoint,root=destination,negative=True)
        try:
            assert candidate.process.wait(timeout=15) != 0, name
            if destination is None:
                assert not candidate.root.exists(), name
            negatives.append(dict(case=name, exitCode=candidate.process.returncode, destination=str(candidate.root)))
        finally:
            candidate.close(rejected=True)
        h["save"](evaluator / "negatives.json", negatives)
    def suffix(owner, prefix_ids, ordinal_base=100):
        if sweep:
            return h["SWEEP"]["suffix"](owner, prefix_ids, h, args, manifest, actor, labels, schedule, environment, runtime, evaluator, server, reset_cache, allow)
        reset_cache()
        initial_requests=len(h["rows"](server.log)) if server.log.exists() else 0
        state,_=owner.ready()
        advance=owner.control.call("advance",dict(deltaMs=schedule["advanceMillis"],deadlineMs=30000))
        assert advance == dict(nowMillis=state["runtimeState"]["nowMillis"]+schedule["advanceMillis"])
        mappings=dict(prefix_ids); outputs=[]; receipts=[]; actor_acks=[]; start_id=100
        for index, step in enumerate(schedule["steps"]):
            ordinal=ordinal_base+index;server.delivery=ordinal
            projection=h["CONFORMANCE"]["projection"](step,mappings,start_id,ordinal)
            arguments=projection["currentAction"]["arguments"]
            if "query" in arguments: allow(arguments["query"])
            ack,outcomes=owner.step(projection)
            assert int(ack["stepId"]) > start_id
            start_id=int(ack["stepId"]);mappings.update(values(outcomes))
            tool=ack["response"]["result"]
            expected_error=step["tag"] in ("invalid-quality","replay-miss")
            assert tool.get("isError",False) == expected_error, step["tag"]
            outputs.append(dict(tag=step["tag"],result=tool));actor_acks.append(ack)
            state,ready=owner.ready();receipts.append(dict(deliveryOrdinal=ordinal,state=state,readiness=ready))
            h["save"](owner.output / "step-receipts.json",receipts)
        assert mappings["turn-4"] != prefix_ids["turn-0"]
        tombstone=json.loads(next(r for r in outputs if r["tag"]=="tombstone")["result"]["content"][0]["text"])
        assert tombstone["id"] == prefix_ids["turn-0"] and tombstone["phase"]=="Tombstone" and tombstone["summary"]=="" and tombstone["fullText"] is None and tombstone["tags"]==[]
        queries=[]
        for row in outputs:
            if row["tag"].startswith("query:"):
                memories=json.loads(row["result"]["content"][0]["text"])["memories"]
                if row["tag"]=="query:fts":
                    assert len(memories)==3 and {v["id"] for v in memories}=={mappings[f"turn-{i}"] for i in (5,6,7)}
                    assert all(v["namespace"]=="foreign" for v in memories)
                    assert all(isinstance(v["score"],(int,float)) for v in memories)
                else:
                    assert [v["id"] for v in memories] == [mappings["turn-4"]]
                returned=memories+json.loads(row["result"]["content"][0]["text"]).get("graphContext",{}).get("neighbors",[])
                queries.append(dict(queryId=row["tag"][6:],memoryIds=[v["id"] for v in returned]))
        state,ready=owner.ready()
        checkpoint=checked_snapshot(h,owner.control.call("snapshot",dict(deadlineMs=30000)),runtime)
        assert checkpoint["committed"] == state["committed"]
        provider=h["rows"](server.log)[initial_requests:]
        assert sum(r["status"]==400 for r in provider)==1
        repeated=[r for r in provider if json.loads(r["requestBody"])["input"] == ["a6-query:replacement"]]
        assert len(repeated)==3 and all(r["status"]==200 for r in repeated), "provider cache changed real repeated-query path"
        event_rows=[dict(sourceId=source,status=status,reason="acknowledged-checkpoint-prefix" if source in prefix_ids else "actual-suffix-store",**(dict(memoryId=memory) if status=="accepted" else {}))
                    for source,memory in mappings.items() for status in ("seen","attempted","accepted")]
        events="".join(json.dumps(v)+"\n" for v in event_rows)
        score=dict(profile=manifest["scoringProfile"],environmentHash=environment,sourceIds=[s["sourceId"] for s in actor["sources"]],mappings={memory:[source] for source,memory in mappings.items()},
            relevant=labels["relevant"],queries=queries,sourceEvents=events,sourceEventsSha256=h["digest"](events.encode()))
        (owner.output / "source-events.jsonl").write_text(events)
        h["save"](owner.output / "score-input.json",score)
        subprocess.run([str(args.scorer),"--input",str(owner.output / "score-input.json"),"--out",str(owner.output / "scores.json")],check=True,env=dict(PATH="/usr/bin:/bin"))
        h["save"](owner.output / "source-mappings.json",dict(sourceMappingVersion=1,actorArtifactSha256=manifest["actorInputs"]["sha256"],mappings=score["mappings"],sources=actor["sources"]))
        h["save"](owner.output / "prefix-provenance.json",dict(prefixArtifact="../prefix-mappings.json",prefixArtifactSha256=h["digest"]((evaluator / "prefix-mappings.json").read_bytes())))
        with (owner.output / "trace.jsonl").open("w") as trace:
            for ack,receipt in zip(actor_acks,receipts):
                trace.write(json.dumps(dict(traceSchemaVersion=1,runId=environment,episodeId=owner.output.name,groupId="synthetic-regression",
                    stepId=ack["stepId"],environmentHash=environment,action=ack["request"],outcome=ack["response"],readGeneration=None,
                    readiness=dict(available=True,scope="post-step-barrier-not-response-readview",receiptFile="step-receipts.json",deliveryOrdinal=receipt["deliveryOrdinal"]),
                    simulatedTimeMs=None,snapshot=None,reward=None))+"\n")
        wire=h["rows"](owner.output / "actor-wire.jsonl")
        canary=schedule["futureQueryCanary"]
        first=next(ordinal_base+i for i,s in enumerate(schedule["steps"]) if canary in json.dumps(s))
        assert all(canary not in json.dumps(r) for r in wire if r["deliveryOrdinal"]<first)
        assert all(canary not in json.dumps(r) for r in provider if r["deliveryOrdinal"]<first)
        assert canary not in (runtime / "actor.py").read_text()
        for p in (owner.output / "actor-inputs").glob("*.json"):
            assert labels["labelCanary"] not in p.read_text()
        assert labels["labelCanary"] not in json.dumps(wire)+json.dumps(provider)
        assert len([r for r in wire if r["tag"]=="control-fds-closed"]) == len(list((owner.output / "actor-inputs").glob("*.json")))
        comparison=dict(outputs=outputs,mappings=mappings,state=state,readiness=ready,logicalHash=checkpoint["logicalHash"],
            provider=[dict(request=json.loads(r["requestBody"]),status=r["status"],vectors=r["responseVectors"]) for r in provider])
        h["save"](owner.output / "suffix.json",comparison)
        return comparison
    try:
        original=child("original")
        original.before_actor(capabilities=capabilities)
        server.delivery=0
        ack,outcomes=original.step(dict(phase="ingest",sources=[s for s in actor["sources"] if s["sourceId"]!="turn-4"],activeEmbedding=manifest["activeEmbedding"],startId=0,deliveryOrdinal=0))
        prefix_ids=values(outcomes);assert len(prefix_ids)==(1 if sweep else 7)
        h["save"](evaluator / "prefix-mappings.json",prefix_ids)
        start_id=int(ack["stepId"]);warm_start=len(h["rows"](server.log))
        for ordinal in (1,2):
            server.delivery=ordinal;allow(schedule["warmQuestion"]["text"])
            ack,_=original.step(dict(phase="query",startId=start_id,deliveryOrdinal=ordinal,currentQuestion=schedule["warmQuestion"],expectedMiss=False));start_id=int(ack["stepId"])
        assert len(h["rows"](server.log))-warm_start==2
        prefix_state,_=original.ready()
        checkpoint=checked_snapshot(h,original.control.call("snapshot",dict(deadlineMs=30000)),runtime)
        assert checkpoint["committed"] == prefix_state["committed"]
        h["save"](evaluator / "prefix-checkpoint.json",checkpoint)
        checkpoint_root=Path(checkpoint["manifest"]).parent;checkpoint_files=inventory(checkpoint_root)
        reject("old-owner-active",checkpoint)
        expected=suffix(original,prefix_ids)
        original.close()
        original_files=inventory(original.root)
        # The consumer must refuse its own successful-restore path until joined.
        # Runtime rejection above separately exercises the actual owner's locks.
        for index in range(3):
            restored=child(f"restored-{index}",checkpoint,prior=original if index==0 else restored)
            restored.before_actor(checkpoint,prefix_state,capabilities)
            server.delivery=99
            restored.step(dict(phase="connect",activeEmbedding=manifest["activeEmbedding"],startId=0,deliveryOrdinal=99))
            observed=suffix(restored,prefix_ids)
            assert observed==expected, f"raw suffix replay {index} differs"
            restored.close();replays.append(dict(index=index,output=str(restored.output),joined=restored.joined))
            unchanged(original.root,original_files);unchanged(checkpoint_root,checkpoint_files)
        # Actual pending provider work in a restored owner must retain ancestry.
        pending=child("pending-owner",checkpoint,prior=restored)
        pending.before_actor(checkpoint,prefix_state,capabilities)
        pending.step(dict(phase="connect",activeEmbedding=manifest["activeEmbedding"],startId=0,deliveryOrdinal=200))
        server.delivery=201
        pending.start_step(dict(phase="action",startId=100,deliveryOrdinal=201,currentAction=dict(tag="pending-store",tool="store_memory",arguments=dict(summary="blocked-worker-canary",namespace="default",checkDuplicates=False))))
        assert entered.wait(15), "actual provider worker was not entered"
        try:
            reject("old-worker-active",checkpoint)
        finally:
            release.set()
        ack,_=pending.finish_step();assert not ack["response"]["result"].get("isError",False)
        pending.ready();pending.close()
        unchanged(original.root,original_files);unchanged(checkpoint_root,checkpoint_files)
        unsupported=child("unsupported-clock",checkpoint,prior=pending)
        unsupported.before_actor(checkpoint,prefix_state,capabilities)
        now=prefix_state["runtimeState"]["nowMillis"]
        assert unsupported.control.call("advance",dict(deltaMs=2**48-now,deadlineMs=30000))==dict(nowMillis=2**48)
        published_before={str(p) for p in runtime.glob("*/manifest.json")}
        unsupported.control.call("snapshot",dict(deadlineMs=30000),error="CONTROL_UNAVAILABLE")
        assert {str(p) for p in runtime.glob("*/manifest.json")}==published_before, "unsupported clock published a checkpoint"
        unsupported.close()
        negatives.append(dict(case="unsupported-checkpoint-clock",error="CONTROL_UNAVAILABLE",newCheckpointPublished=False))
        destination=runtime / "existing-destination";destination.mkdir();(destination / "sentinel").write_text("must remain unchanged\n")
        before=inventory(destination);reject("existing-destination",checkpoint,destination);assert inventory(destination)==before
        link=runtime / "symlink-destination";link.symlink_to(destination,target_is_directory=True)
        reject("symlink-destination",checkpoint,link);assert link.is_symlink() and inventory(destination)==before
        reject("wrong-manifest-digest",dict(checkpoint,sha256="0"*64))
        source_manifest=json.loads(Path(checkpoint["manifest"]).read_text())
        bad_source=dict(source_manifest,sourceRoot=str(runtime / "absent-source"))
        path=checkpoint_root / "negative-source.json";h["save"](path,bad_source)
        reject("wrong-source",dict(checkpoint,manifest=str(path),sha256=h["digest"](path.read_bytes())))
        unchanged(original.root,original_files);unchanged(checkpoint_root,checkpoint_files)
        result.update(status="passed",freshOwnerSuffixReplays=3,negativeCases=negatives,rawSuffixEquality=True,
            actorCurrentStepOnly=True,sourceCheckpointBytesUnchanged=True,originalOwnerBytesUnchanged=True,
            lifecycleRuntimeAndCanonicalIdsRestored=True,allOwnersJoined=True,logicalSuffixHash=expected["logicalHash"],
            fullA6=False,publicV2=False,resetRpc=False,allRngControlled=False)
    except BaseException as error:
        result.update(status="failed",error=str(error))
        raise
    finally:
        release.set()
        cleanup_errors=[]
        for owner in reversed(children):
            if not owner.joined:
                try:owner.close()
                except Exception as error:cleanup_errors.append(str(error))
        server.shutdown();server.server_close();thread.join(timeout=10)
        if cleanup_errors or thread.is_alive():
            result.update(status="failed",cleanupErrors=cleanup_errors,providerThreadJoined=not thread.is_alive())
        result["replays"]=replays
        result["artifactsSha256"]={str(p.relative_to(args.out)):h["digest"](p.read_bytes()) for p in args.out.rglob("*") if p.is_file() and not p.is_symlink() and p.name!="result.json"}
        h["save"](args.out / "result.json",result)
    if result["status"] != "passed":
        raise RuntimeError("reset episode did not finish")
