import ast,hashlib,json,pathlib,subprocess,time
base=pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue');run=base/'validation-o1-9988af67';out=run/'output';source=run/'source';tail=out/'completion-tail';tail.mkdir()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
build=json.loads((out/'build.json').read_text());steps=json.loads((out/'steps.json').read_text());assert build['sourceCommit']=='9988af67439a19813decb8c01ddddb97b91fc09b'
assert len(steps)==9 and all(e['exitCode']==0 for e in steps)
assert all(sha(source/n)==v for n,v in build['sourceFilesSha256'].items())
exe=base/'validation-pr22-a20ab593/target/debug/deps/recalld-8b31964f64b2cfc4';expected='46479a60c3a3bda06ad6301593daee667fc47de716916a4e8784f3190b5829fa';assert sha(exe)==expected
(tail/'prior-status.json').write_text(json.dumps({'reason':'Initial affected filter set emitted only6page fixtures; initial driver stopped before formal schema phase. Original logs/build status retained. Add actual canonical list/catalog tests on same executable; no source change/rebuild/full repeat.','source':build['sourceCommit'],'executableSHA256':expected,'started':time.time()},indent=2))
script='''import pathlib,subprocess,json,hashlib,time
out=pathlib.Path('/work/output');tail=out/'completion-tail';events=[]
for name,minimum in [('canonical_list::tests',4),('catalog_page::tests',5)]:
 command=['/work/target/debug/deps/recalld-8b31964f64b2cfc4',name,'--nocapture','--test-threads=2'];log=tail/(name.replace('::','-')+'.log')
 with log.open('wb') as f: code=subprocess.run(command,stdout=f,stderr=subprocess.STDOUT).returncode
 import re
 matches=re.findall(r'test result: ok[.] (\\d+) passed; (\\d+) failed',log.read_text());assert code==0 and matches and int(matches[-1][0])>=minimum and int(matches[-1][1])==0
 events.append({'command':command,'exitCode':code,'finished':time.time(),'passed':int(matches[-1][0])});(tail/'tests.json').write_text(json.dumps(events,indent=2))
(tail/'cgroup.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}))
'''
(tail/'execute.py').write_text(script)
image=build['containerImageId'];name='recalld-merge-queue-o1-page-tail'
cmd=['docker','run','--name',name,'--network','none','--cpus','2','--memory','6g','--memory-swap','6g','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g','--mount',f'type=bind,src={source},dst=/work/source,readonly','--mount',f'type=bind,src={base/"validation-pr22-a20ab593/target"},dst=/work/target,readonly','--mount',f'type=bind,src={out},dst=/work/output','--workdir','/work/source','-e','HOME=/tmp','-e','RECALLD_PAGE_SCHEMA_FIXTURES=/work/output/page-fixtures',image,'python3','/work/output/completion-tail/execute.py']
(tail/'command.json').write_text(json.dumps(cmd,indent=2))
with (tail/'container.log').open('wb') as f:code=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT).returncode
(tail/'container.json').write_text(subprocess.check_output(['docker','inspect',name],text=True));subprocess.run(['docker','rm',name],check=True,stdout=subprocess.DEVNULL)
(tail/'exit.json').write_text(json.dumps({'exitCode':code}));assert code==0
assert sha(exe)==expected
schema=out/'schema-validation';schema.mkdir();wheelroot=pathlib.Path('/tmp/recalld-huygens-active-validation/schema-wheels');wheels=json.loads((wheelroot/'manifest.json').read_text());assert len(wheels)==6 and all(sha(wheelroot/n)==v for n,v in wheels.items());(schema/'wheels.json').write_text(json.dumps(wheels))
for directory,filename,minimum in [('schema-fixtures','fixture-before.json',1),('page-fixtures','page-before.json',22),('live-fixtures','live-before.json',13)]:
 pins={p.name:sha(p) for p in (out/directory).glob('*.json')};assert len(pins)>=minimum;(schema/filename).write_text(json.dumps(pins))
tree=ast.parse((base/'validate-o1-9988af67.py').read_text());schema_script=next(ast.literal_eval(n.value) for n in ast.walk(tree) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='schema_script' for t in n.targets));compile(schema_script,'<schema>','exec')
name='recalld-merge-queue-o1-d1-schema';cmd=['docker','run','--name',name,'--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,size=64m','-e','HOME=/tmp']
for host,guest,ro in [(out/'schema-fixtures','/fixtures',True),(out/'page-fixtures','/page-fixtures',True),(out/'live-fixtures','/live-fixtures',True),(source/'tests/query_schema_conformance.py','/runner.py',True),(source/'tests/page_schema_conformance.py','/page-runner.py',True),(source/'tests/live_tool_schema_conformance.py','/live-runner.py',True),(wheelroot,'/wheels',True),(schema,'/evidence',False)]:
 cmd+=['--mount',f'type=bind,src={host},dst={guest}'+(',readonly' if ro else '')]
cmd +=[image,'python3','-c',schema_script];(schema/'command.json').write_text(json.dumps(cmd,indent=2))
with (schema/'schema.log').open('wb') as f:code=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT).returncode
(schema/'container.json').write_text(subprocess.check_output(['docker','inspect',name],text=True));subprocess.run(['docker','rm',name],check=True,stdout=subprocess.DEVNULL);(schema/'exit.json').write_text(json.dumps({'exitCode':code}));assert code==0
assert sha(exe)==expected
post={n:sha(source/n) for n in build['sourceFilesSha256']};assert post==build['sourceFilesSha256'];(tail/'source-after.json').write_text(json.dumps(post,indent=2))
(tail/'completed.json').write_text(json.dumps({'status':'passed','source':build['sourceCommit'],'executableBeforeAfterSHA256':expected,'finished':time.time()},indent=2));print('PASS supplemental same-executable page tests and formal schemas',flush=True)
