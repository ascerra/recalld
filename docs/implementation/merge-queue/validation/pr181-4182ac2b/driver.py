import hashlib,json,pathlib,runpy,subprocess,time
root=pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue/pr181');base=pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue');cache=base/'validation-pr22-a20ab593'
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert head=='4182ac2b6d0fea03822c0836f9de0d830ef68dc9'
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
run=base/('validation-pr181-'+head[:8]);run.mkdir();source=run/'source';source.mkdir();output=run/'output';output.mkdir()
h=runpy.run_path(str(root/'scripts/public-mcp-evaluation.py'));helper=runpy.run_path(str(root/'scripts/offline-evaluation.py'))
public=runpy.run_path(str(root/'scripts/public-conformance.py'))
pins=helper['snapshot_source'](root,source,tuple(h['EXTRA'])+tuple(public['HARNESS_INPUTS'])+('scripts/check-public-similarity.py',))
for name,sha in pins.items():
 assert hashlib.sha256(subprocess.check_output(['git','show',head+':'+name],cwd=root)).hexdigest()==sha,name
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
metadata=dict(sourceCommit=head,sourceFilesSha256=pins,containerImageId=image,started=time.time(),status='running',cache='one serial queue cache; cargo clean -p recalld before switched-source compilation', validationScope='PR181 actualmain56 normaljoin4182: ONLY Cargo/bin/profileCLItest/doc exact56. Core/src exceptaddedbin byteexactf45/b20. Feynman exact4182 SOURCEACK. Bounded actual10CLI including2existingvectorcert, fmt/7Python/allfeaturealltargetClippy/CLIhelp/public9. b20full1638lib+5CLI+15examples remains originalsource attribution, no newfullclaim. Earlier failures immutable; noactivation.', resourceChange='6GiB authorized validation profile following retained native closure linkerOOM; no resource-conformance/performance claim')
(output/'build.json').write_bytes(h['encoded'](metadata))
commands=[['cargo', 'fmt', '--all', '--check'], ['python3', 'scripts/test_public_conformance.py'], ['python3', 'scripts/test_offline_evaluation.py'], ['python3', 'scripts/test_public_mcp_evaluation.py'], ['cargo', 'test', '--offline', '--locked', '-j1', '--all-features', '--test', 'profile_attestation_cli', '--test', 'inherited_policy_cli', '--test', 'lifecycle_cli', '--test', 'text_migration_cli', '--test', 'vector_certification_cli', '--', '--nocapture'], ['cargo', 'clippy', '--offline', '--locked', '-j1', '--all-features', '--all-targets'], ['cargo', 'build', '--offline', '--locked', '-j1', '--features', 'bench', '--bin', 'recalld', '--bin', 'recalld-schema'], ['/work/target/debug/recalld-schema', '--help'], ['python3', 'scripts/public-conformance.py', '--inside', '--fixture-version', '3'], ['python3', '/work/output/check-public.py']]
script='''import subprocess,json,pathlib,hashlib,time
out=pathlib.Path('/work/output');
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
 test_binaries={str(p.relative_to('/work/target')):hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/work/target/debug/deps').glob('*') if p.is_file() and p.name.startswith(('recalld-','recalld_cli-','recalld_text_migrate-','inherited_policy_cli-','lifecycle_cli-','text_migration_cli-','profile_attestation_cli-','vector_certification_cli-','recalld_schema-','recalld_vector_certify-')) and p.stat().st_mode & 0o111} if command[:2]==['cargo','test'] else {}\n if command[:2]==['cargo','test']:\n  import re\n  for name in re.findall(r'^\\s+Running [^\\n]+ \\((/work/target/[^\\n)]+)\\)$',(out/f'step-{index}.log').read_text(),re.M):\n   p=pathlib.Path(name);test_binaries[str(p.relative_to('/work/target'))]=hashlib.sha256(p.read_bytes()).hexdigest()\n events.append(dict(command=command,exitCode=code,binariesBeforeSha256=binaries,testExecutablesSha256=test_binaries,finished=time.time()))
 (out/'steps.json').write_text(json.dumps(events,indent=2)+'\\n')
 (out/f'cgroup-step-{index}.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}))
 if code:raise SystemExit(code)
 if command[:2]==['cargo','test']:
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
args=['docker','run','--name','recalld-merge-queue-pr181','--network','none','--cpus','2','--memory','6g','--memory-swap','6g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g','--mount',f'type=bind,src={source},dst=/work/source,readonly','--mount',f'type=bind,src={cache/"cargo"},dst=/cargo','--mount',f'type=bind,src={cache/"target"},dst=/work/target','--mount',f'type=bind,src={output},dst=/work/output','--workdir','/work/source','-e','CARGO_HOME=/cargo','-e','CARGO_TARGET_DIR=/work/target','-e','HOME=/tmp','-e','CARGO_NET_OFFLINE=true','-e','CARGO_BUILD_JOBS=2','-e','RUSTUP_TOOLCHAIN=1.94.1','-e',f'RECALLD_A6_SOURCE_COMMIT={head}','-e',f'RECALLD_A6_SOURCE_TREE_SHA256={h["digest"](h["encoded"](pins))}','-e',f'RECALLD_A6_IMAGE_ID={image}',image,'python3','/work/output/execute.py']
metadata['command']=args;(output/'build.json').write_bytes(h['encoded'](metadata))
with (output/'container.log').open('wb') as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
inspect=subprocess.run(['docker','inspect','recalld-merge-queue-pr181'],capture_output=True,text=True)
(output/'container-inspect.json').write_text(inspect.stdout)
subprocess.run(['docker','rm','recalld-merge-queue-pr181'],check=True,stdout=subprocess.DEVNULL)
post={name:helper['sha'](source/name) for name in pins};assert post==pins
metadata.update(status='passed' if code==0 else 'failed',exitCode=code,finished=time.time(),sourceAfterSha256=post)
(output/'build.json').write_bytes(h['encoded'](metadata));print(json.dumps(dict(run=str(run),source=head,exitCode=code)),flush=True)
raise SystemExit(code)
