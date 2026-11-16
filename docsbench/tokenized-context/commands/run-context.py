import hashlib,json,pathlib,runpy,subprocess,time
root=pathlib.Path('/tmp/recalld-implementation/a6-tokenized-context');base=pathlib.Path('/tmp/recalld-owned-native-controls-validation')
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
run=base/('context-'+head[:8]);run.mkdir();source=run/'source';source.mkdir();output=run/'output';output.mkdir()
h=runpy.run_path(str(root/'scripts/public-mcp-evaluation.py'));helper=runpy.run_path(str(root/'scripts/offline-evaluation.py'))
pins=helper['snapshot_source'](root,source,h['EXTRA'])
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
metadata=dict(sourceCommit=head,sourceFilesSha256=pins,containerImageId=image,started=time.time(),status='running',cache='one reused private Cargo/target; shared registry read-only',scope='targeted tokenizer/Python and actual public contexts; no unchanged Rust suite',cpus=2,memoryBytes=4294967296)
commands=[['python3','scripts/prepare-context-assets.py','--prefetch','/prefetch','--out','/output/tokenizer'],['python3','scripts/test_public_mcp_evaluation.py'],['python3','scripts/test_context_mcp_evaluation.py','--assets','/output/tokenizer'],['cargo','build','--offline','--locked','-j1','--bin','recalld','--example','score_mcp_episode'],['python3','scripts/check_context_mcp_evaluation.py','--binary','/target/debug/recalld','--scorer','/target/debug/examples/score_mcp_episode','--source-root','/work/source','--build','/output/build.json','--assets','/output/tokenizer','--out','/output/actual']]
script='''import subprocess,json,pathlib,hashlib,time
out=pathlib.Path('/output');events=[];commands=json.loads((out/'commands.json').read_text());expected=json.loads((out/'build.json').read_text())['sourceFilesSha256']
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def sources():return {n:digest(pathlib.Path('/work/source',n)) for n in expected}
def cgroup():return {n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}
(out/'inside-source-before.json').write_text(json.dumps(sources(),indent=2));assert sources()==expected
(out/'cgroup-before.json').write_text(json.dumps(cgroup()))
for index,command in enumerate(commands):
 binaries={str(p):digest(p) for p in (pathlib.Path('/target/debug/recalld'),pathlib.Path('/target/debug/examples/score_mcp_episode'))} if index==4 else {}
 with (out/f'step-{index}.log').open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 executables={str(p):digest(p) for p in pathlib.Path('/target/debug/examples').glob('score_mcp_episode-*') if p.is_file() and p.stat().st_mode & 0o111} if False else {}
 events.append(dict(command=command,exitCode=code,binariesBeforeSha256=binaries,testExecutablesSha256=executables,finished=time.time()))
 (out/'steps.json').write_text(json.dumps(events,indent=2)+'\\n');(out/f'cgroup-step-{index}.json').write_text(json.dumps(cgroup()))
 (out/'inside-source-after.json').write_text(json.dumps(sources(),indent=2));assert sources()==expected
 for name,sha in binaries.items():assert digest(pathlib.Path(name))==sha
 if code:raise SystemExit(code)
'''
(output/'commands.json').write_text(json.dumps(commands,indent=2)+'\n');(output/'execute.py').write_text(script)
args=['docker','run','--name','recalld-tokenized-context','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev,exec,size=1g','--mount',f'type=bind,src={source},dst=/work/source,readonly','--mount',f'type=bind,src={base/"cargo"},dst=/cargo','--mount','type=bind,src=/tmp/recalld-unit-tests-l__ja1yo/cargo/registry,dst=/cargo/registry,readonly','--mount',f'type=bind,src={base/"target"},dst=/target','--mount','type=bind,src=/tmp/recalld-tokenizer-prefetch,dst=/prefetch,readonly','--mount',f'type=bind,src={output},dst=/output','--workdir','/work/source','-e','CARGO_HOME=/cargo','-e','CARGO_TARGET_DIR=/target','-e','HOME=/tmp','-e','CARGO_NET_OFFLINE=true','-e','CARGO_BUILD_JOBS=1','-e','PYTHONDONTWRITEBYTECODE=1','-e',f'RECALLD_A6_SOURCE_COMMIT={head}','-e',f'RECALLD_A6_SOURCE_TREE_SHA256={h["digest"](h["encoded"](pins))}','-e',f'RECALLD_A6_IMAGE_ID={image}',image,'python3','/output/execute.py']
metadata['command']=args;(output/'build.json').write_bytes(h['encoded'](metadata))
with (output/'container.log').open('wb') as log:code=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT).returncode
(output/'container-inspect.json').write_text(subprocess.check_output(['docker','inspect','recalld-tokenized-context'],text=True));subprocess.run(['docker','rm','recalld-tokenized-context'],check=True,stdout=subprocess.DEVNULL)
post={n:helper['sha'](source/n) for n in pins};assert post==pins
metadata.update(status='passed' if code==0 else 'failed',exitCode=code,finished=time.time(),sourceAfterSha256=post)
(output/'build.json').write_bytes(h['encoded'](metadata));print(json.dumps(dict(run=str(run),source=head,exitCode=code)),flush=True);raise SystemExit(code)
