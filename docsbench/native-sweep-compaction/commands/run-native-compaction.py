import hashlib,json,pathlib,runpy,subprocess,time
root=pathlib.Path('/tmp/recalld-implementation/a6-native-sweep-compaction');base=pathlib.Path('/tmp/recalld-owned-native-controls-validation')
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
run=base/('native-compaction-'+head[:8]);run.mkdir();source=run/'source';source.mkdir();output=run/'output';output.mkdir()
h=runpy.run_path(str(root/'scripts/public-mcp-evaluation.py'));helper=runpy.run_path(str(root/'scripts/offline-evaluation.py'))
pins=helper['snapshot_source'](root,source,h['EXTRA'])
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
metadata=dict(sourceCommit=head,sourceFilesSha256=pins,containerImageId=image,started=time.time(),status='running',cache='single bounded private cache; shared registry read-only')
(output/'build.json').write_bytes(h['encoded'](metadata))
commands=[['cargo','test','--offline','--locked','-j1','--features','bench','--lib','sweep','--','--test-threads=1'],['cargo','test','--offline','--locked','-j1','--features','bench','--lib','compaction','--','--test-threads=1'],['python3','scripts/test_native_sweep_episode.py'],['python3','scripts/test_public_mcp_evaluation.py'],['python3','scripts/test_native_prefix_episode.py'],['cargo','build','--offline','--locked','-j1','--features','bench','--example','owned_evaluator','--example','native_prefix_fixture','--example','score_mcp_episode'],['python3','scripts/public-mcp-evaluation.py','prepare','--track',h['NATIVE_SWEEP']['TRACK'],'--binary','/target/debug/examples/owned_evaluator','--scorer','/target/debug/examples/score_mcp_episode','--build','/output/build.json','--out','/output/inputs'],['python3','scripts/public-mcp-evaluation.py','run','--binary','/target/debug/examples/owned_evaluator','--scorer','/target/debug/examples/score_mcp_episode','--source-root','/work/source','--manifest','/output/inputs/manifest.json','--out','/output/episode']]
script='''import subprocess,json,pathlib,hashlib,time
out=pathlib.Path('/output');commands=json.loads((out/'commands.json').read_text());events=[]
expected=json.loads((out/'build.json').read_text())['sourceFilesSha256']
before={name:hashlib.sha256(pathlib.Path('/work/source',name).read_bytes()).hexdigest() for name in expected}
(out/'inside-source-before.json').write_text(json.dumps(before,indent=2));assert before==expected
for index,command in enumerate(commands):
 binaries={}
 if index>=6:
  binaries={name:hashlib.sha256(pathlib.Path('/target/debug/examples',name).read_bytes()).hexdigest() for name in ('owned_evaluator','native_prefix_fixture','score_mcp_episode')}
 with (out/f'step-{index}.log').open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 test_binaries={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/target/debug/deps').glob('recalld-*') if p.is_file() and p.stat().st_mode & 0o111} if index in (0,1) else {}
 events.append(dict(command=command,exitCode=code,binariesBeforeSha256=binaries,testExecutablesSha256=test_binaries,finished=time.time()))
 (out/'steps.json').write_text(json.dumps(events,indent=2)+'\\n')
 if code:raise SystemExit(code)
 if binaries:assert all(hashlib.sha256(pathlib.Path('/target/debug/examples',name).read_bytes()).hexdigest()==sha for name,sha in binaries.items())
'''
script += "\nafter={name:hashlib.sha256(pathlib.Path('/work/source',name).read_bytes()).hexdigest() for name in expected}\n(out/'inside-source-after.json').write_text(json.dumps(after,indent=2));assert after==expected\n"
(output/'commands.json').write_text(json.dumps(commands,indent=2)+'\n');(output/'execute.py').write_text(script)
args=['docker','run','--rm','--name','recalld-native-compaction-consumer','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g','--mount',f'type=bind,src={source},dst=/work/source,readonly','--mount',f'type=bind,src={base/"cargo"},dst=/cargo','--mount','type=bind,src=/tmp/recalld-unit-tests-l__ja1yo/cargo/registry,dst=/cargo/registry,readonly','--mount',f'type=bind,src={base/"target"},dst=/target','--mount',f'type=bind,src={output},dst=/output','--workdir','/work/source','-e','CARGO_HOME=/cargo','-e','CARGO_TARGET_DIR=/target','-e','HOME=/tmp','-e','CARGO_NET_OFFLINE=true','-e','CARGO_BUILD_JOBS=1','-e',f'RECALLD_A6_SOURCE_COMMIT={head}','-e',f'RECALLD_A6_SOURCE_TREE_SHA256={h["digest"](h["encoded"](pins))}','-e',f'RECALLD_A6_IMAGE_ID={image}',image,'python3','/output/execute.py']
metadata['command']=args;(output/'build.json').write_bytes(h['encoded'](metadata))
with (output/'container.log').open('wb') as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
post={name:helper['sha'](source/name) for name in pins};assert post==pins
metadata.update(status='passed' if code==0 else 'failed',exitCode=code,finished=time.time(),sourceAfterSha256=post)
(output/'build.json').write_bytes(h['encoded'](metadata));print(json.dumps(dict(run=str(run),source=head,exitCode=code)),flush=True)
raise SystemExit(code)
