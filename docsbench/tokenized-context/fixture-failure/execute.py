import subprocess,json,pathlib,hashlib,time
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
 (out/'steps.json').write_text(json.dumps(events,indent=2)+'\n');(out/f'cgroup-step-{index}.json').write_text(json.dumps(cgroup()))
 (out/'inside-source-after.json').write_text(json.dumps(sources(),indent=2));assert sources()==expected
 for name,sha in binaries.items():assert digest(pathlib.Path(name))==sha
 if code:raise SystemExit(code)
