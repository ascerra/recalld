"""Actual synthetic native-prefix checkpoint/MCP consumer; no native write/reset emulation."""
import copy
import hashlib
import http.server
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading

TRACK = "serialized-native-prefix-synthetic-diagnostic-v1"
CHECKPOINT_TRACK = "isolated-native-prefix-checkpoint-v1"
HASH_CONTRACT = "native-logical-export-v1"


def fixtures(base, future_variant=False):
    actor, labels, cache, _ = base()
    corpus = [("alpha document", "alpha " + "filler " * 20, [0., 0., 1., 0.]),
              ("beta beta beta", None, [1., 0., 0., 0.]),
              ("beta", None, [0., 1., 0., 0.]), ("foreign beta", "foreign payload", [1., 0., 0., 0.])]
    for source, (summary, body, vector) in zip(actor["sources"], corpus):
        source.update(summary=summary, contentSha256=hashlib.sha256(summary.encode()).hexdigest(), embedding=vector)
        if body is not None:
            source.update(fullText=body, fullTextSha256=hashlib.sha256(body.encode()).hexdigest())
    future = "beta FUTURE_NATIVE_QUERY" + (" CHANGED_FUTURE_ONLY" if future_variant else "")
    questions = [dict(queryId="q0", text="alpha beta"), dict(queryId="q1", text="beta"),
                 dict(queryId="q2", text="alpha beta"), dict(queryId="q3", text=future)]
    vector = [0.9, 0.8, 0.7, 0.]
    norm = sum(x*x for x in vector) ** 0.5
    cache["entries"] = {"a6-query:alpha beta": [v/norm for v in vector], "a6-query:beta": [1., 0., 0., 0.],
                        "a6-query:"+future: [1., 0., 0., 0.], "a6-query:blocked-native-query": [1., 0., 0., 0.]}
    steps = [dict(tag="get", tool="get_memory", arguments=dict(id="$turn-0", namespace="default", recordAccess=False)),
             dict(tag="foreign-assertion", tool="get_memory", arguments=dict(id="$turn-0", namespace="foreign", recordAccess=False))]
    steps += [dict(tag="query:"+q["queryId"], tool="recall_memories", arguments=dict(query=q["text"], namespace="default", recordAccess=False, depth=0, limit=3, compact=False)) for q in questions]
    steps += [dict(tag="foreign-query", tool="recall_memories", arguments=dict(query="beta", namespace="foreign", recordAccess=False, depth=0, limit=3, compact=False)),
              dict(tag="omitted-access", tool="get_memory", arguments=dict(id="$turn-0", namespace="default")),
              dict(tag="true-access", tool="get_memory", arguments=dict(id="$turn-0", namespace="default", recordAccess=True)),
              dict(tag="omitted-search-access", tool="recall_memories", arguments=dict(query="beta", namespace="default")),
              dict(tag="true-search-access", tool="recall_memories", arguments=dict(query="beta", namespace="default", recordAccess=True)),
              dict(tag="blocked-store", tool="store_memory", arguments=dict(summary="unrecorded-write", namespace="default")),
              dict(tag="blocked-feedback", tool="reinforce_memory", arguments=dict(id="$turn-0", namespace="default", quality=3)),
              dict(tag="blocked-forget", tool="forget_memory", arguments=dict(id="$turn-0", namespace="default")),
              dict(tag="replay-miss", tool="recall_memories", arguments=dict(query="unrecorded-native-query", namespace="default", recordAccess=False, depth=0, limit=1, compact=False))]
    labels["relevant"] = {"q0":["turn-0","turn-1"], "q1":[], "q3":["turn-1"]}
    return actor, labels, cache, dict(scheduleVersion=1, futureVariant="changed" if future_variant else "baseline",
        queries=questions, futureQueryCanary="FUTURE_NATIVE_QUERY", steps=steps)


def capabilities(h):
    value = copy.deepcopy(h["RESET"]["CONTROL_CAPABILITIES"])
    value.update(sweepControlVersion=None, track=CHECKPOINT_TRACK, canonicalSchemaVersion=2,
                 logicalHashContract=HASH_CONTRACT, actorActions=["get_memory:recordAccess=false", "recall_memories:recordAccess=false"], writes=False)
    value["guarantees"].update(explicitLegacySweep=False, nativeSweep=False)
    return value


def run(args, h, sweep=None):
    checkpoint_track = sweep["CHECKPOINT_TRACK"] if sweep else CHECKPOINT_TRACK
    fixture = (lambda base, future=False: sweep["fixtures"](base, h["NATIVE"], future)) if sweep else fixtures
    expected_capabilities = sweep["capabilities"](h) if sweep else capabilities(h)
    manifest, actor, labels, cache, schedule, environment = h["validate"](args.manifest, args.binary, args.scorer, args.source_root)
    # Only validated artifacts can create output/provider/store side effects.
    args.out.mkdir()
    evaluator=args.out/"evaluator"; evaluator.mkdir()
    if sweep: sweep["check_old_manifest"](args, h, manifest, evaluator)
    runtime=args.out/"runtime"; runtime.mkdir(mode=0o700)
    (runtime/"home").mkdir(mode=0o700)
    shutil.copyfile(args.source_root/"scripts/public-mcp-actor.py", runtime/"actor.py")
    assert h["digest"]((runtime/"actor.py").read_bytes()) == manifest["sourceFilesSha256"]["scripts/public-mcp-actor.py"]
    entered,release=threading.Event(),threading.Event()
    class Replay(h["Replay"]):
        def do_POST(self):
            raw=self.rfile.read(int(self.headers.get("Content-Length","0")))
            if "a6-query:blocked-native-query" in json.loads(raw).get("input",[]):
                entered.set()
                if not release.wait(30):
                    self.send_error(500,"fixture query latch timed out");return
            self.rfile=io.BytesIO(raw)
            super().do_POST()
    server=http.server.ThreadingHTTPServer(("127.0.0.1",0), Replay)
    server.owned=True; server.log=args.out/"actor-provider.jsonl"; server.environment=environment; server.delivery=0
    server.key=lambda text: h["digest"](h["encoded"](dict(environmentHash=server.environment, model=h["MODEL"], dimension=4, prefixedInput=text)))
    server.entries={}
    thread=threading.Thread(target=server.serve_forever); thread.start()
    config=runtime/"config.toml"
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
    profile=dict(identityVersion=1,providerKind="openai",modelId=h["MODEL"],modelRevision=None,dimension=4,
        documentPreprocessing=dict(version="recalld-document-v1",prefix="a6-document:"),
        queryPreprocessing=dict(version="recalld-query-v1",prefix="a6-query:"),normalization="l2_unit_v1")
    seed=dict(fixtureVersion=1,profile=profile,attest=True,sources=[dict(sourceId=s["sourceId"],namespace=s["namespace"],summary=s["summary"],fullText=s.get("fullText"),embedding=s["embedding"]) for s in actor["sources"]])
    if sweep: seed.update(fixtureVersion=3, namespaceDecayMultipliers={"foreign":0.0}, inheritedDecayAdoptions={"default":1.0})
    children=[]; negatives=[]; comparisons=[]
    result=dict(status="running",track=sweep["TRACK"] if sweep else TRACK,environmentHash=environment,independentNativeGenesis=False)
    save=h["save"]
    def build_prefix(name, data):
        source=evaluator/(name+"-input.json"); report=evaluator/(name+"-provenance.json"); root=runtime/name
        save(source,data)
        with (evaluator/(name+"-builder.log")).open("wb") as log:
            subprocess.run([str(args.binary.with_name("native_prefix_fixture")),"--input",str(source),"--root",str(root),"--report",str(report)],check=True,stdout=log,stderr=subprocess.STDOUT,env=dict(PATH="/usr/bin:/bin"))
        return root,json.loads(report.read_text())
    def child(name, root=None, checkpoint=None, prior=None, negative=False, native=False, cfg=None):
        value=h["RESET"]["Child"](h,args,root or runtime/name,cfg or config,evaluator/name,checkpoint,prior,negative,native,native and sweep is not None)
        children.append(value);return value
    def snapshot(owner):
        return h["RESET"]["checked_snapshot"](h,owner.control.call("snapshot",dict(deadlineMs=30000)),runtime,checkpoint_track)
    def provider_rows(): return h["rows"](server.log) if server.log.exists() else []
    def reject(name, root=None, checkpoint=None, native=False, cfg=None, error_contains=None):
        count=len(provider_rows()); owner=child("negative-"+name,root,checkpoint,negative=True,native=native,cfg=cfg)
        try:
            assert owner.process.wait(timeout=30) != 0,name
            if error_contains: assert error_contains in (owner.output/"runtime.log").read_text(),name
            assert len(provider_rows())==count,"rejected constructor called provider"
            if not native and root is None: assert not owner.root.exists(),"rejected restore created destination"
            negatives.append(dict(case=name,exitCode=owner.process.returncode,providerCalls=0))
            save(evaluator/"negatives.json",negatives)
        finally: owner.close(rejected=True)
    def inventory(folder):
        # Byte canaries for this small private fixture, not a logical hash format.
        return {str(p.relative_to(folder)):h["digest"](p.read_bytes()) for p in folder.rglob("*") if p.is_file() and not p.is_symlink()}
    def unchanged(folder,before):
        assert all((folder/name).is_file() and h["digest"]((folder/name).read_bytes())==sha for name,sha in before.items()),"source/checkpoint bytes changed"
    def admit(owner, expected=None, expected_state=None):
        assert owner.control.call("capabilities",{})==expected_capabilities
        if not sweep:
            owner.control.call("sweep",dict(deadlineMs=30000),error="UNSUPPORTED_CONTROL")
            owner.control.call("sweepStatus",{},error="UNSUPPORTED_CONTROL")
        state,ready=owner.ready(); snap=snapshot(owner)
        if expected:
            assert snap["logicalHash"]==expected["logicalHash"] and snap["committed"]==expected["committed"]
            assert state==expected_state
        save(owner.output/"before-actor.json",dict(state=state,readiness=ready,snapshot=snap))
        owner.step(dict(phase="connect_native_prefix",startId=0,deliveryOrdinal=0))
        return state,snap
    def suffix(owner, mappings, current_labels, current_schedule, current_cache):
        if sweep:
            return sweep["suffix"](owner, mappings, h, args, manifest, actor, current_labels,
                current_schedule, current_cache, server, runtime, snapshot, provider_rows)
        server.entries={}; first=len(provider_rows()); state,_=owner.ready(); start_id=100
        outputs=[]; receipts=[]; acks=[]; queries=[]
        for index,step in enumerate(current_schedule["steps"]):
            ordinal=100+index;server.delivery=ordinal
            projection=h["CONFORMANCE"]["projection"](step,mappings,start_id,ordinal)
            arguments=projection["currentAction"]["arguments"]
            if "query" in arguments:
                key="a6-query:"+arguments["query"]
                if key in current_cache["entries"]: server.entries[server.key(key)]=current_cache["entries"][key]
            before_calls=len(provider_rows())
            ack,_=owner.step(projection);start_id=int(ack["stepId"])
            tool=ack["response"]["result"]
            expected_error=step["tag"] in {"foreign-assertion","omitted-access","true-access","omitted-search-access","true-search-access","blocked-store","blocked-feedback","blocked-forget","replay-miss"}
            outputs.append(dict(tag=step["tag"],result=tool));acks.append(ack)
            save(owner.output/"outputs.json",outputs)
            assert tool.get("isError",False)==expected_error,step["tag"]
            if expected_error and step["tag"]!="replay-miss": assert len(provider_rows())==before_calls
            after,ready=owner.ready();assert after==state,"false or rejected action changed canonical/runtime state"
            receipts.append(dict(deliveryOrdinal=ordinal,state=after,readiness=ready));save(owner.output/"step-receipts.json",receipts)
            if step["tag"].startswith("query:"):
                memories=json.loads(tool["content"][0]["text"])["memories"]
                assert memories and mappings["turn-3"] not in {v["id"] for v in memories}
                queries.append(dict(queryId=step["tag"][6:],memoryIds=[v["id"] for v in memories]))
            if step["tag"]=="foreign-query":
                memories=json.loads(tool["content"][0]["text"])["memories"]
                assert [v["id"] for v in memories]==[mappings["turn-3"]]
            if step["tag"]=="get":
                payload=json.loads(tool["content"][0]["text"])
                assert payload["id"]==mappings["turn-0"] and payload["summary"]==actor["sources"][0]["summary"] and payload["fullText"]==actor["sources"][0]["fullText"]
        snap=snapshot(owner);assert snap["logicalHash"]==checkpoint["logicalHash"]
        provider=provider_rows()[first:];assert sum(v["status"]==400 for v in provider)==1
        assert sum(v["status"]==200 for v in provider)==5
        events="".join(json.dumps(dict(sourceId=s,status=status,reason="acknowledged-migrated-prefix",**(dict(memoryId=m) if status=="accepted" else {})))+"\n" for s,m in mappings.items() for status in ("seen","attempted","accepted"))
        (owner.output/"source-events.jsonl").write_text(events)
        score=dict(profile=manifest["scoringProfile"],environmentHash=server.environment,sourceIds=[s["sourceId"] for s in actor["sources"]],mappings={m:[s] for s,m in mappings.items()},relevant=current_labels["relevant"],queries=queries,sourceEvents=events,sourceEventsSha256=h["digest"](events.encode()))
        save(owner.output/"score-input.json",score)
        subprocess.run([str(args.scorer),"--input",str(owner.output/"score-input.json"),"--out",str(owner.output/"scores.json")],check=True,env=dict(PATH="/usr/bin:/bin"))
        save(owner.output/"source-mappings.json",dict(sourceMappingVersion=1,mappings=score["mappings"],prefixProvenanceSha256=h["digest"]((evaluator/"original-provenance.json").read_bytes())))
        with (owner.output/"trace.jsonl").open("w") as trace:
            for ack,receipt in zip(acks,receipts):
                trace.write(json.dumps(dict(traceSchemaVersion=1,runId=server.environment,episodeId=owner.output.name,groupId="synthetic-regression",stepId=ack["stepId"],environmentHash=server.environment,action=ack["request"],outcome=ack["response"],readGeneration=None,readiness=dict(available=True,scope="post-step-barrier-not-response-readview",deliveryOrdinal=receipt["deliveryOrdinal"]),snapshot=None,reward=None))+"\n")
        canary=current_schedule["futureQueryCanary"]; first_delivery=next(100+i for i,s in enumerate(current_schedule["steps"]) if canary in json.dumps(s))
        wire=h["rows"](owner.output/"actor-wire.jsonl")
        assert canary not in (runtime/"actor.py").read_text() and current_labels["labelCanary"] not in (runtime/"actor.py").read_text()
        assert all(canary not in json.dumps(v) for v in wire if v["deliveryOrdinal"]<first_delivery)
        assert all(canary not in json.dumps(v) for v in provider if v["deliveryOrdinal"]<first_delivery)
        for path in (owner.output/"actor-inputs").glob("*.json"): assert current_labels["labelCanary"] not in path.read_text()
        assert current_labels["labelCanary"] not in json.dumps(wire)+json.dumps(provider)
        comparison=dict(outputs=outputs,state=state,logicalHash=snap["logicalHash"],provider=[dict(request=json.loads(v["requestBody"]),status=v["status"],vectors=v["responseVectors"]) for v in provider])
        save(owner.output/"suffix.json",comparison);return comparison
    try:
        if sweep:
            unknown=copy.deepcopy(seed);unknown.update(fixtureVersion=2);unknown.pop("inheritedDecayAdoptions")
            unknown_root,_=build_prefix("unknown-inherited-prefix",unknown)
            before_unknown=inventory(unknown_root)
            def unknown_files(phase):
                values={str(p.relative_to(unknown_root)):dict(sha256=h["digest"](p.read_bytes()),size=p.stat().st_size)
                    for p in unknown_root.rglob("*") if p.is_file() and not p.is_symlink()}
                save(evaluator/("unknown-files-"+phase+".json"),values)
                return values
            unknown_files("before-open")
            unknown_owner=child("unknown-inherited-prefix",root=unknown_root,native=True)
            assert unknown_owner.control.call("capabilities",{}) == expected_capabilities
            before_state=unknown_owner.control.call("inspect",{})
            unknown_files("before-sweep")
            provider_count=len(provider_rows())
            try:
                unknown_owner.control.call("sweep",dict(deadlineMs=30000))
            except ValueError:
                # Preserve the full typed failed-sweep wire rather than claiming
                # that an unknown policy forbids every nonprojecting read/open.
                wire=h["rows"](unknown_owner.output/"control-wire.jsonl")
                failure=wire[-1]["response"]["error"]
                assert failure["code"] == "SWEEP_FAILED" and "inherited" in json.dumps(failure)
                assert failure["retryable"] is False
            else:
                raise AssertionError("unknown inherited policy sweep succeeded")
            unknown_files("after-sweep")
            unavailable=unknown_owner.control.call("inspect",{},error="CONTROL_UNAVAILABLE")
            assert "sweep episode invalid" in unavailable["message"]
            status=unknown_owner.control.call("sweepStatus",{})
            assert status["episodeInvalidated"] is True
            assert status["lastSweep"]["phase"] == "failed"
            assert status["lastSweep"]["result"] == failure["sweep"]["result"]
            assert status["lastSweep"]["error"]["code"] == "SWEEP_FAILED"
            assert len(provider_rows()) == provider_count
            unknown_files("before-close")
            unknown_owner.close()
            unknown_files("after-close")
            unchanged(unknown_root,before_unknown)
            reopened=child("unknown-inherited-reopened",root=unknown_root,native=True)
            assert reopened.control.call("capabilities",{}) == expected_capabilities
            after_state=reopened.control.call("inspect",{})
            assert after_state == before_state
            assert len(provider_rows()) == provider_count
            reopened.close()
            unchanged(unknown_root,before_unknown)
            save(evaluator/"unknown-inherited-sweep.json",dict(failure=failure,
                invalidatedOwnerInspect=unavailable,sweepStatus=status,
                beforeState=before_state,freshOwnerState=after_state,
                sourceFilesSha256=before_unknown,stateUnchanged=True,providerCalls=0))
            negatives.append(dict(case="unknown-inherited-prefix-sweep",code="SWEEP_FAILED",providerCalls=0))
        root,provenance=build_prefix("original",seed); mappings=provenance["mappings"]
        if sweep:
            adoption=provenance["inheritedPolicyAdoptions"]
            assert len(adoption)==2 and adoption[1]["historicalReceiptBytesPreserved"] and adoption[1]["foreignNamespaceUnchanged"]
            assert adoption[0]["request"]["preview"]["prior"] is None
            save(evaluator/"explicit-policy-adoption.json",adoption)
        original=child("original",root=root,native=True); initial_state,checkpoint=admit(original)
        # This is the actual source-owner lock gate, before any clone is installed.
        reject("old-owner",checkpoint=checkpoint)
        checkpoint_root=Path(checkpoint["manifest"]).parent;checkpoint_files=inventory(checkpoint_root)
        save(evaluator/"prefix-checkpoint.json",checkpoint)
        baseline=suffix(original,mappings,labels,schedule,cache);comparisons.append(baseline);original.close()
        original_files=inventory(original.root)
        for index in range(3):
            owner=child(f"restore-{index}",checkpoint=checkpoint,prior=original)
            admit(owner,checkpoint,initial_state)
            value=suffix(owner,mappings,labels,schedule,cache);comparisons.append(value)
            save(evaluator/"replay-comparisons.json",comparisons)
            assert value==baseline,"same-prefix replay differs; values retained"
            owner.close()
            unchanged(original.root,original_files);unchanged(checkpoint_root,checkpoint_files)
        # Perturbed evaluator artifacts pass the same real manifest preflight. Their
        # environment/cache identity changes; compare actor behavior separately.
        variants=[]
        for variant in ("labels","future"):
            folder=evaluator/("variant-"+variant);folder.mkdir()
            candidate=copy.deepcopy(manifest)
            a,l,c,sc=fixture(h["fixtures"],variant=="future")
            if variant=="labels":
                l["relevant"]={"q0":["turn-2"],"q1":["turn-0"],"q3":[]}
                l["labelCanary"]="CHANGED_NATIVE_EVALUATOR_LABEL_ONLY"
            for field,name,value in (("actorInputs","actor-inputs.json",a),("evaluatorLabels","evaluator-labels.json",l),("replayCache","evaluator-cache.json",c),("evaluatorSchedule","evaluator-schedule.json",sc)):
                save(folder/name,value);candidate[field]=dict(file=name,sha256=h["digest"]((folder/name).read_bytes()))
            save(folder/"manifest.json",candidate)
            _,a,l,c,sc,variant_environment=h["validate"](folder/"manifest.json",args.binary,args.scorer,args.source_root)
            assert variant_environment!=environment
            server.environment=variant_environment
            owner=child("canary-"+variant,checkpoint=checkpoint,prior=original)
            admit(owner,checkpoint,initial_state)
            value=suffix(owner,mappings,l,sc,c);owner.close()
            if variant=="labels":
                assert value==baseline,"evaluator label perturbation changed actor behavior"
                for filename in (owner.output/"actor-inputs").glob("*.json"):
                    assert filename.read_bytes()==(original.output/"actor-inputs"/filename.name).read_bytes()
            else:
                first=next(i for i,step in enumerate(schedule["steps"]) if schedule["futureQueryCanary"] in json.dumps(step))
                action_first=sum("control" not in step for step in schedule["steps"][:first])
                assert value["outputs"][:action_first]==baseline["outputs"][:action_first],"future question changed prior actor outputs"
                for ordinal in [0,*[100+i for i,step in enumerate(schedule["steps"][:first]) if "control" not in step]]:
                    assert (owner.output/"actor-inputs"/f"step-{ordinal}.json").read_bytes()==(original.output/"actor-inputs"/f"step-{ordinal}.json").read_bytes()
                prior_queries=sum(step.get("tool")=="recall_memories" and step["arguments"].get("recordAccess") is False for step in schedule["steps"][:first])
                assert value["provider"][:prior_queries]==baseline["provider"][:prior_queries],"future question changed prior vectors"
            base_keys={key for row in provider_rows() if row["environmentHash"]==environment for key in row["cacheKeys"]}
            variant_keys={key for row in provider_rows() if row["environmentHash"]==variant_environment for key in row["cacheKeys"]}
            assert base_keys and variant_keys and not base_keys & variant_keys,"environment identities did not separate replay keys"
            variants.append(dict(variant=variant,environmentHash=variant_environment,logicalHash=value["logicalHash"],actorBehaviorVerified=True))
            save(evaluator/"canaries.json",variants)
        server.environment=environment
        # An actual blocked text-query provider keeps the old owner alive. Restore
        # must reject until its actor and runtime have both completed and joined.
        pending=child("pending-query",checkpoint=checkpoint,prior=original)
        admit(pending,checkpoint,initial_state)
        server.entries={server.key("a6-query:blocked-native-query"):cache["entries"]["a6-query:blocked-native-query"]}
        server.delivery=301
        pending.start_step(dict(phase="action",startId=100,deliveryOrdinal=301,currentAction=dict(tag="pending-query",tool="recall_memories",arguments=dict(query="blocked-native-query",namespace="default",recordAccess=False,depth=0,limit=1,compact=False))))
        assert entered.wait(15),"actual provider query was not entered"
        try:reject("old-query-worker-active",checkpoint=checkpoint)
        finally:release.set()
        ack,_=pending.finish_step();assert not ack["response"]["result"].get("isError",False)
        state,_=pending.ready();assert state==initial_state;pending.close()
        unsupported=child("unsupported-clock",checkpoint=checkpoint,prior=pending)
        admit(unsupported,checkpoint,initial_state)
        now=initial_state["runtimeState"]["nowMillis"]
        assert unsupported.control.call("advance",dict(deltaMs=2**48-now,deadlineMs=30000))==dict(nowMillis=2**48)
        published_before={str(p) for p in runtime.glob("*/manifest.json")}
        unsupported.control.call("snapshot",dict(deadlineMs=30000),error="CONTROL_UNAVAILABLE")
        assert {str(p) for p in runtime.glob("*/manifest.json")}==published_before
        unsupported.close();negatives.append(dict(case="unsupported-checkpoint-clock",error="CONTROL_UNAVAILABLE"))
        destination=runtime/"existing-destination";destination.mkdir();(destination/"sentinel").write_text("unchanged\n")
        before=inventory(destination);reject("existing-destination",root=destination,checkpoint=checkpoint);assert inventory(destination)==before
        link=runtime/"symlink-destination";link.symlink_to(destination,target_is_directory=True)
        reject("symlink-destination",root=link,checkpoint=checkpoint);assert link.is_symlink() and inventory(destination)==before
        reject("wrong-manifest-digest",checkpoint=dict(checkpoint,sha256="0"*64))
        checkpoint_manifest=json.loads(Path(checkpoint["manifest"]).read_text())
        if sweep:
            old_track=dict(checkpoint_manifest,track="isolated-native-sweep-checkpoint-v1")
            path=checkpoint_root/"negative-old-track.json";save(path,old_track)
            assert old_track["executableSha256"] == checkpoint_manifest["executableSha256"]
            reject("old-sweep-track-valid-digest",checkpoint=dict(checkpoint,manifest=str(path),sha256=h["digest"](path.read_bytes())),error_contains="unsupported checkpoint track")
        bad_source=dict(checkpoint_manifest,sourceRoot=str(runtime/"absent-source"))
        path=checkpoint_root/"negative-source.json";save(path,bad_source)
        reject("wrong-source",checkpoint=dict(checkpoint,manifest=str(path),sha256=h["digest"](path.read_bytes())))
        unchanged(original.root,original_files);unchanged(checkpoint_root,checkpoint_files)
        # Wrong provider identity, unknown provenance and revision mismatches reject before providers.
        for name,change in [("unbound",lambda v:v.update(attest=False)),("wrong-model",lambda v:v["profile"].update(modelId="synthetic-wrong-profile")),("wrong-revision",lambda v:v["profile"].update(modelRevision="synthetic-negative-only"))]:
            negative_seed=copy.deepcopy(seed);change(negative_seed)
            negative_root,_=build_prefix(name,negative_seed);reject(name,root=negative_root,native=True)
        result.update(status="passed",restores=3,suffixes=4,actionsPerSuffix=sum("control" not in step for step in schedule["steps"]),hashes=[v["logicalHash"] for v in comparisons],negativeCases=negatives,actorCurrentStepOnly=True,canaries=variants,sourceCheckpointBytesUnchanged=True,originalOwnerBytesUnchanged=True)
    except BaseException as error:
        result.update(status="failed",error=str(error));raise
    finally:
        release.set()
        for owner in reversed(children):
            if not owner.joined:
                try:owner.close(rejected=True)
                except Exception as error: result.setdefault("cleanupErrors",[]).append(str(error))
        server.shutdown();server.server_close();thread.join()
        result["artifactSha256"]={str(p.relative_to(args.out)):h["digest"](p.read_bytes()) for p in args.out.rglob("*") if p.is_file() and "runtime" not in p.relative_to(args.out).parts and p.name!="result.json"}
        save(args.out/"result.json",result)
