import hashlib,json,pathlib,runpy,subprocess,time,os
root=pathlib.Path('/tmp/recalld-implementation/owned-native-controls')
base=pathlib.Path('/tmp/recalld-owned-native-controls-validation')
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
run=base/('run-'+head[:8]);run.mkdir();source=run/'source';source.mkdir();output=run/'output';output.mkdir()
helper=runpy.run_path(str(root/'scripts/offline-evaluation.py'))
hashes=helper['snapshot_source'](root,source)
for name in ('cargo','target'):(base/name).mkdir(exist_ok=True)
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
args=['docker','run','--rm','--name','recalld-owned-native-controls-focused','--network','none','--cpus','2','--memory','4g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g',
'--mount',f'type=bind,src={source},dst=/work/source,readonly','--mount',f'type=bind,src={base/"cargo"},dst=/cargo','--mount','type=bind,src=/tmp/recalld-unit-tests-l__ja1yo/cargo/registry,dst=/cargo/registry,readonly','--mount',f'type=bind,src={base/"target"},dst=/target','--mount',f'type=bind,src={output},dst=/output','--workdir','/work/source','-e','CARGO_HOME=/cargo','-e','CARGO_TARGET_DIR=/target','-e','HOME=/tmp','-e','CARGO_NET_OFFLINE=true',image,'cargo','test','--offline','--locked','-j1','--features','bench','--lib','native_query_admission','--','--test-threads=1','--nocapture']
manifest={'sourceCommit':head,'sourceFilesSha256':hashes,'sourceTreeSha256':hashlib.sha256(json.dumps(hashes,sort_keys=True,separators=(',',':')).encode()).hexdigest(),'command':args,'started':time.time(),'limits':{'cpus':2,'memoryGiB':4,'jobs':1,'network':'none'},'registry':'existing shared read-only; no cache clone'}
(output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
with (output/'focused.log').open('wb') as f:code=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT).returncode
post={name:helper['sha'](source/name) for name in hashes};assert post==hashes
manifest.update(exitCode=code,finished=time.time(),sourceAfterSha256=post)
(output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'run':str(run),'source':head,'exitCode':code,'log':str(output/'focused.log')}),flush=True)
raise SystemExit(code)
