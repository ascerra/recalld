import hashlib,json,pathlib,runpy,subprocess,time
root=pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue/o1-main164');base=pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue');cache=base/'validation-pr22-a20ab593'
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert head=='2925a7289055b27ffda2583a81b656442f347632'
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
# Skip an obsolete planned composition if the independent queue has already advanced main.
actual_main=json.loads(subprocess.check_output(['gh','api','repos/calebevans/recalld/git/ref/heads/main'],text=True))['object']['sha']
assert actual_main=='6f064789473eea310be28e004de38fd52d2aa3d5','main advanced before run; source review required, no snapshot/cache/Docker started'
run=base/('validation-o1-'+head[:8]);run.mkdir();source=run/'source';source.mkdir();output=run/'output';output.mkdir()
h=runpy.run_path(str(root/'scripts/public-mcp-evaluation.py'));helper=runpy.run_path(str(root/'scripts/offline-evaluation.py'))
public=runpy.run_path(str(root/'scripts/public-conformance.py'))
pins=helper['snapshot_source'](root,source,tuple(h['EXTRA'])+tuple(public['HARNESS_INPUTS'])+('scripts/check-public-similarity.py','tests/query_schema_conformance.py','tests/page_schema_conformance.py'))
for name,sha in pins.items():
 assert hashlib.sha256(subprocess.check_output(['git','show',head+':'+name],cwd=root)).hexdigest()==sha,name
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
metadata=dict(sourceCommit=head,sourceFilesSha256=pins,containerImageId=image,started=time.time(),status='running',cache='one serial queue cache; cargo clean -p recalld before switched-source compilation', validationScope='O1 sealed18d plus actualmain1646f normal2925 composition; independent get/page/query capability union. Fresh allfeature/alltarget full, prepared RPC/strict schema, public9+true exposure controls. Owner1764 full remains0e3338 only until this run completes. No ordinaryAPI1get routing/nativeordinary/public2 activation.', resourceChange='6GiB authorized validation profile following retained native closure linkerOOM; no resource-conformance/performance claim')
(output/'build.json').write_bytes(h['encoded'](metadata))
commands=[['cargo', 'fmt', '--all', '--check'], ['cargo', 'test', '--offline', '--locked', '-j1', '--all-features', '--lib', 'prepared_', '--', '--nocapture', '--test-threads=2'], ['cargo', 'test', '--offline', '--locked', '-j1', '--all-features', '--all-targets', '--', '--test-threads=2'], ['cargo', 'test', '--offline', '--locked', '-j1', '--all-features', '--doc'], ['cargo', 'clippy', '--offline', '--locked', '-j1', '--all-features', '--all-targets'], ['cargo', 'build', '--offline', '--locked', '-j1', '--features', 'bench', '--bin', 'recalld'], ['python3', 'scripts/public-conformance.py', '--inside', '--fixture-version', '3'], ['python3', '/work/output/check-public.py']]
script='''import subprocess,json,pathlib,hashlib,time
out=pathlib.Path('/work/output');
import os,resource
(out/'toolchain.json').write_text(json.dumps({'commands':{str(c):subprocess.check_output(c,text=True) for c in (['rustc','-vV'],['cargo','-vV'],['rustup','show','active-toolchain'],['uname','-m'])},'stackRlimit':resource.getrlimit(resource.RLIMIT_STACK),'environment':{k:os.environ.get(k) for k in ('RUSTUP_TOOLCHAIN','RUST_MIN_STACK','RUSTFLAGS','CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_TARGET_DIR')}},indent=2))
with (out/'own-package-clean.log').open('wb') as log: subprocess.run(['cargo','clean','-p','recalld'],stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'cgroup-before.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}));commands=json.loads((out/'commands.json').read_text());events=[]
expected=json.loads((out/'build.json').read_text())['sourceFilesSha256']
before={name:hashlib.sha256(pathlib.Path('/work/source',name).read_bytes()).hexdigest() for name in expected}
(out/'inside-source-before.json').write_text(json.dumps(before,indent=2));assert before==expected
for index,command in enumerate(commands):
 binaries={}
 if command[0]=='python3' and command[1] in ('scripts/public-conformance.py','/work/output/check-public.py'):
  binaries={'../recalld':hashlib.sha256(pathlib.Path('/work/target/debug/recalld').read_bytes()).hexdigest()}
 with (out/f'step-{index}.log').open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 test_binaries={}\n if command[:2]==['cargo','test']:\n  import re\n  for name in re.findall(r'^\\s+Running [^\\n]+ \\((/work/target/[^\\n)]+)\\)$',(out/f'step-{index}.log').read_text(),re.M):\n   p=pathlib.Path(name);test_binaries[str(p.relative_to('/work/target'))]=hashlib.sha256(p.read_bytes()).hexdigest()\n cli_binaries={}\n if command[:2]==['cargo','test'] and '--all-targets' in command:\n  for name in ('recalld','recalld-cli','recalld-schema','recalld-text-migrate'):\n   p=pathlib.Path('/work/target/debug')/name;assert p.is_file(),name;cli_binaries[name]=hashlib.sha256(p.read_bytes()).hexdigest()\n events.append(dict(command=command,exitCode=code,binariesBeforeSha256=binaries,testExecutablesSha256=test_binaries,executedCliBinariesSha256=cli_binaries,finished=time.time()))
 (out/'steps.json').write_text(json.dumps(events,indent=2)+'\\n')
 (out/f'cgroup-step-{index}.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}))
 if code:raise SystemExit(code)
 if command[:2]==['cargo','test'] and '--doc' not in command:
  import re
  text=(out/f'step-{index}.log').read_text();assert re.search(r'test result: ok[.] [1-9][0-9]* passed',text),'zero-match test filter'
 if binaries:assert all(hashlib.sha256(pathlib.Path('/work/target/debug/examples',name).read_bytes()).hexdigest()==sha for name,sha in binaries.items())
'''
script += "\nafter={name:hashlib.sha256(pathlib.Path('/work/source',name).read_bytes()).hexdigest() for name in expected}\n(out/'inside-source-after.json').write_text(json.dumps(after,indent=2));assert after==expected\n"
(output/'commands.json').write_text(json.dumps(commands,indent=2)+'\n');(output/'execute.py').write_text(script)
(output/'check-public.py').write_text('''import pathlib,json,hashlib,runpy
out=pathlib.Path('/work/output')
manifest=json.loads((out/'build.json').read_text())
manifest['outputsSha256']={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in out.rglob('*') if p.is_file() and p.suffix in ('.json','.jsonl') and p.name not in ('manifest.json','build.json','steps.json')}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2))
runpy.run_path('/work/source/scripts/check-public-similarity.py')['check'](out)
''')
args=['docker','run','--name','recalld-merge-queue-o1','--network','none','--cpus','2','--memory','6g','--memory-swap','6g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g','--mount',f'type=bind,src={source},dst=/work/source,readonly','--mount',f'type=bind,src={cache/"cargo"},dst=/cargo','--mount',f'type=bind,src={cache/"target"},dst=/work/target','--mount',f'type=bind,src={output},dst=/work/output','--workdir','/work/source','-e','CARGO_HOME=/cargo','-e','CARGO_TARGET_DIR=/work/target','-e','RECALLD_QUERY_SCHEMA_FIXTURES=/work/output/schema-fixtures','-e','RECALLD_PAGE_SCHEMA_FIXTURES=/work/output/page-fixtures','-e','HOME=/tmp','-e','CARGO_NET_OFFLINE=true','-e','CARGO_BUILD_JOBS=1','-e','RUSTUP_TOOLCHAIN=1.94.1','-e',f'RECALLD_A6_SOURCE_COMMIT={head}','-e',f'RECALLD_A6_SOURCE_TREE_SHA256={h["digest"](h["encoded"](pins))}','-e',f'RECALLD_A6_IMAGE_ID={image}',image,'python3','/work/output/execute.py']
metadata['command']=args;(output/'build.json').write_bytes(h['encoded'](metadata))
with (output/'container.log').open('wb') as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
inspect=subprocess.run(['docker','inspect','recalld-merge-queue-o1'],capture_output=True,text=True)
(output/'container-inspect.json').write_text(inspect.stdout)
subprocess.run(['docker','rm','recalld-merge-queue-o1'],check=True,stdout=subprocess.DEVNULL)
if code==0:
 wheelroot=pathlib.Path('/tmp/recalld-huygens-active-validation/schema-wheels')
 wheels=json.loads((wheelroot/'manifest.json').read_text());assert len(wheels)==6
 for name,sha in wheels.items():assert hashlib.sha256((wheelroot/name).read_bytes()).hexdigest()==sha
 pages=output/'page-fixtures';page_pins={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in pages.glob('*.json')};assert len(page_pins)>=22
 fixtures=output/'schema-fixtures';fixture_pins={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in fixtures.glob('*.json')};assert fixture_pins
 schemaout=output/'schema-validation';schemaout.mkdir();(schemaout/'wheels.json').write_text(json.dumps(wheels));(schemaout/'fixture-before.json').write_text(json.dumps(fixture_pins));(schemaout/'page-before.json').write_text(json.dumps(page_pins))
 runner=source/'tests/query_schema_conformance.py';runner_before=hashlib.sha256(runner.read_bytes()).hexdigest();page_runner=source/'tests/page_schema_conformance.py';page_runner_before=hashlib.sha256(page_runner.read_bytes()).hexdigest()
 schema_script="""import pathlib,json,hashlib,zipfile,sys,runpy,importlib.metadata
out=pathlib.Path('/evidence');target=out/'python';target.mkdir()
for name,sha in json.loads((out/'wheels.json').read_text()).items():
 p=pathlib.Path('/wheels')/name;assert hashlib.sha256(p.read_bytes()).hexdigest()==sha
 with zipfile.ZipFile(p) as z:
  assert all(not pathlib.PurePosixPath(n).is_absolute() and '..' not in pathlib.PurePosixPath(n).parts for n in z.namelist());z.extractall(target)
sys.path.insert(0,str(target));assert importlib.metadata.version('jsonschema')=='4.25.1'
actual={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/fixtures').glob('*.json')};assert actual==json.loads((out/'fixture-before.json').read_text())
count=sum(len(v) if isinstance(v,list) else 1 for v in (json.loads(p.read_text()) for p in pathlib.Path('/fixtures').glob('*.json')));assert count>=63,count
sys.argv=['/runner.py','/fixtures'];runpy.run_path('/runner.py',run_name='__main__')
assert actual=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/fixtures').glob('*.json')}
(out/'result.json').write_text(json.dumps({'caseCount':count,'fixtureHashes':actual,'jsonschema':'4.25.1','runnerSha256':hashlib.sha256(pathlib.Path('/runner.py').read_bytes()).hexdigest()}))
page_actual={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/page-fixtures').glob('*.json')};assert page_actual==json.loads((out/'page-before.json').read_text()) and len(page_actual)>=22
sys.argv=['/page-runner.py','/page-fixtures'];runpy.run_path('/page-runner.py',run_name='__main__')
assert page_actual=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/page-fixtures').glob('*.json')}
(out/'page-result.json').write_text(json.dumps({'caseCount':len(page_actual),'fixtureHashes':page_actual,'runnerSha256':hashlib.sha256(pathlib.Path('/page-runner.py').read_bytes()).hexdigest()}))
"""
 schema_command=['docker','run','--rm','--name','recalld-merge-queue-o1-schema','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,size=64m','--mount',f'type=bind,src={fixtures},dst=/fixtures,readonly','--mount',f'type=bind,src={runner},dst=/runner.py,readonly','--mount',f'type=bind,src={page_runner},dst=/page-runner.py,readonly','--mount',f'type=bind,src={pages},dst=/page-fixtures,readonly','--mount',f'type=bind,src={wheelroot},dst=/wheels,readonly','--mount',f'type=bind,src={schemaout},dst=/evidence','-e','HOME=/tmp',image,'python3','-c',schema_script]
 (schemaout/'command.json').write_text(json.dumps(schema_command,indent=2))
 with (schemaout/'schema.log').open('wb') as log:code=subprocess.run(schema_command,stdout=log,stderr=subprocess.STDOUT).returncode
 assert hashlib.sha256(runner.read_bytes()).hexdigest()==runner_before and hashlib.sha256(page_runner.read_bytes()).hexdigest()==page_runner_before
 assert page_pins=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in pages.glob('*.json')}
 assert fixture_pins=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in fixtures.glob('*.json')}
 (schemaout/'exit.json').write_text(json.dumps({'exitCode':code,'runnerSha256':runner_before,'fixtureFiles':len(fixture_pins),'pageRunnerSha256':page_runner_before,'pageFixtureFiles':len(page_pins)}))
post={name:helper['sha'](source/name) for name in pins};assert post==pins
metadata.update(status='passed' if code==0 else 'failed',exitCode=code,finished=time.time(),sourceAfterSha256=post)
(output/'build.json').write_bytes(h['encoded'](metadata));print(json.dumps(dict(run=str(run),source=head,exitCode=code)),flush=True)
raise SystemExit(code)
