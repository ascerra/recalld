import hashlib,json,pathlib,runpy,subprocess,time
root=pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue/api1-get-main192');base=root.parent;cache=base/'validation-pr22-a20ab593';head='c8514fe70bb9cdd50c3eb89a12b36124f9cf3972'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==head
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
assert subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=root,text=True).strip()=='1c1b7abeb190e2e2b28eaafd05f5fdce65385b28'
run=base/'validation-api1-main192-c8514fe7';run.mkdir();source=run/'source';source.mkdir();out=run/'output';out.mkdir()
h=runpy.run_path(str(root/'scripts/public-mcp-evaluation.py'));helper=runpy.run_path(str(root/'scripts/offline-evaluation.py'))
pins=helper['snapshot_source'](root,source,tuple(h['EXTRA'])+('tests/page_schema_conformance.py','tests/live_tool_schema_conformance.py','tests/live_read_schema_conformance.py'))
for name,sha in pins.items():assert hashlib.sha256(subprocess.check_output(['git','show',head+':'+name],cwd=root)).hexdigest()==sha,name
wheels=pathlib.Path('/tmp/recalld-huygens-active-validation/schema-wheels');wpins=json.loads((wheels/'manifest.json').read_text())
for name,sha in wpins.items():assert hashlib.sha256((wheels/name).read_bytes()).hexdigest()==sha
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef';container='recalld-queue-api1-main192'
meta=dict(source=head,tree='1c1b7abeb190e2e2b28eaafd05f5fdce65385b28',publishedHead195='df7b9358f2cd48980c93044ac0832982d9147047',main192='f32d1754de83e7c3ad5dc2a0c3f27ad42a5e4dde',sourceFiles=pins,wheels=wpins,image=image,status='running',scope='Affected ordinary API1 direct/daemon get receipt/unknown/cancel/no-retry and read/metadata schema join plus three final fixture controls only; no full suite/public9/unrelated schema matrix. Parent proofs remain source-specific.',memoryGiB=6,started=time.time())
(out/'build.json').write_text(json.dumps(meta,indent=2)+'\n')
execute=r'''import hashlib,json,pathlib,subprocess,re,os,resource,time,sys,zipfile,runpy
out=pathlib.Path('/output');meta=json.loads((out/'build.json').read_text());root=pathlib.Path('/source')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def verify():assert {n:sha(root/n) for n in meta['sourceFiles']}==meta['sourceFiles']
verify();(out/'source-before.json').write_text(json.dumps(meta['sourceFiles'],indent=2))
(out/'toolchain.json').write_text(json.dumps({'rustc':subprocess.check_output(['rustc','-vV'],text=True),'cargo':subprocess.check_output(['cargo','-vV'],text=True),'stack':resource.getrlimit(resource.RLIMIT_STACK),'env':{k:os.environ.get(k) for k in ('RUSTUP_TOOLCHAIN','RUST_MIN_STACK','RUSTFLAGS','CARGO_BUILD_JOBS')}},indent=2))
with (out/'clean.log').open('wb') as log:subprocess.run(['cargo','clean','-p','recalld'],stdout=log,stderr=subprocess.STDOUT,check=True)
cmd=['cargo','test','--offline','--locked','-j1','--all-features','--lib','--no-run','--message-format=json']
(out/'compile-command.json').write_text(json.dumps(cmd))
with (out/'compile.jsonl').open('wb') as log,(out/'compile.log').open('wb') as err:subprocess.run(cmd,stdout=log,stderr=err,check=True)
art=[json.loads(l) for l in (out/'compile.jsonl').read_text().splitlines() if l.startswith('{')];exes=[v['executable'] for v in art if v.get('reason')=='compiler-artifact' and v.get('executable') and v['target']['name']=='recalld' and v['profile']['test']]
assert len(exes)==1,exes;exe=pathlib.Path(exes[0]);before=sha(exe)
listed=subprocess.check_output([str(exe),'--list'],text=True);(out/'test-list.txt').write_text(listed);names=[l[:-6] for l in listed.splitlines() if l.endswith(': test')]
patterns=['ordinary_point_','live_read_schemas_','live_metadata_schemas_','coherent_by_id_actual_postcommit_failure_preserves_receipt_and_hides_payload','native_forget_failed_publication_stays_blocked_until_real_retry_without_resurrection','native_exposure_deferred_selection_never_writes_and_unsupported_recorder_stays_closed']
selected=sorted(n for n in names if any(p in n for p in patterns));assert len(selected)>=31 and all(any(p in n for n in selected) for p in patterns)
(out/'selected-tests.json').write_text(json.dumps(selected,indent=2));cmd=[str(exe),'--exact',*selected,'--nocapture','--test-threads=2']
(out/'test-command.json').write_text(json.dumps(cmd))
with (out/'tests.log').open('wb') as log:subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,check=True)
log=(out/'tests.log').read_text();assert re.search(r'test result: ok\. '+str(len(selected))+r' passed; 0 failed; 0 ignored;',log)
assert sha(exe)==before
bundle=pathlib.Path('/tmp/schema');bundle.mkdir()
for name,expected in meta['wheels'].items():
 p=pathlib.Path('/wheels')/name;assert sha(p)==expected
 with zipfile.ZipFile(p) as z:
  assert all(not pathlib.PurePosixPath(n).is_absolute() and '..' not in pathlib.PurePosixPath(n).parts for n in z.namelist());z.extractall(bundle)
env=dict(os.environ,PYTHONPATH=str(bundle),PYTHONDONTWRITEBYTECODE='1')
for runner,fixtures,minimum in [('live_read_schema_conformance.py','read-fixtures',47),('live_tool_schema_conformance.py','metadata-fixtures',13)]:
 ps=list((out/fixtures).glob('*.json'));assert len(ps)>=minimum,(fixtures,len(ps))
 with (out/(runner+'.log')).open('wb') as log:subprocess.run(['python3','tests/'+runner,str(out/fixtures)],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
verify();assert sha(exe)==before;(out/'source-after.json').write_text(json.dumps(meta['sourceFiles'],indent=2));(out/'result.json').write_text(json.dumps({'status':'passed','source':meta['source'],'selectedTests':selected,'passed':len(selected),'executable':str(exe),'beforeAfterSHA256':before,'formalRead':47,'formalMetadata':13,'noFullSuite':True,'noPublicMatrix':True},indent=2))
'''
(out/'execute.py').write_text(execute)
cmd=['docker','run','--name',container,'--network','none','--cpus','2','--memory','6g','--memory-swap','6g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=512m','--mount',f'type=bind,src={source},dst=/source,readonly','--mount',f'type=bind,src={out},dst=/output','--mount',f'type=bind,src={cache/"cargo"},dst=/cargo','--mount',f'type=bind,src={cache/"target"},dst=/target','--mount',f'type=bind,src={wheels},dst=/wheels,readonly','--workdir','/source']
for k,v in dict(HOME='/tmp',CARGO_HOME='/cargo',CARGO_TARGET_DIR='/target',CARGO_BUILD_JOBS='1',CARGO_NET_OFFLINE='true',RUSTUP_TOOLCHAIN='1.94.1',RECALLD_LIVE_READ_SCHEMA_FIXTURES='/output/read-fixtures',RECALLD_LIVE_SCHEMA_FIXTURES='/output/metadata-fixtures').items():cmd+=['-e',k+'='+v]
cmd +=[image,'bash','-c','python3 /output/execute.py; result=$?; cat /sys/fs/cgroup/memory.events > /output/memory.events; cat /sys/fs/cgroup/memory.peak > /output/memory.peak; exit "$result"']
(out/'command.json').write_text(json.dumps(cmd,indent=2));print(json.dumps({'source':head,'container':container,'run':str(run),'phase':'bounded affected validation started'}),flush=True)
with (out/'driver.log').open('wb') as log:
 try:code=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,timeout=1500).returncode
 except subprocess.TimeoutExpired:code=124;subprocess.run(['docker','stop','--time','10',container],check=False,stdout=log,stderr=subprocess.STDOUT)
inspect=subprocess.run(['docker','inspect',container],capture_output=True,text=True);(out/'container-inspect.json').write_text(inspect.stdout);subprocess.run(['docker','rm',container],check=True,stdout=subprocess.DEVNULL)
assert {n:helper['sha'](source/n) for n in pins}==pins
meta.update(status='passed' if code==0 else 'failed',exitCode=code,finished=time.time());(out/'build.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({'source':head,'exitCode':code,'run':str(run)}),flush=True);raise SystemExit(code)
