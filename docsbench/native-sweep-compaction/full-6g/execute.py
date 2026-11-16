import subprocess,json,pathlib,hashlib,time
out=pathlib.Path('/output');(out/'cgroup-before.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}));commands=json.loads((out/'commands.json').read_text());events=[]
expected=json.loads((out/'build.json').read_text())['sourceFilesSha256']
before={name:hashlib.sha256(pathlib.Path('/work/source',name).read_bytes()).hexdigest() for name in expected}
(out/'inside-source-before.json').write_text(json.dumps(before,indent=2));assert before==expected
for index,command in enumerate(commands):
 binaries={}
 if index>=9:
  binaries={name:hashlib.sha256(pathlib.Path('/target/debug/examples',name).read_bytes()).hexdigest() for name in ('owned_evaluator','native_prefix_fixture','score_mcp_episode')}
 with (out/f'step-{index}.log').open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 test_binaries={str(p.relative_to('/target')):hashlib.sha256(p.read_bytes()).hexdigest() for p in pathlib.Path('/target/debug/deps').glob('*') if p.is_file() and p.name.startswith(('recalld-','lifecycle_cli-','text_migration_cli-')) and p.stat().st_mode & 0o111} if index in (4,5) else {}
 events.append(dict(command=command,exitCode=code,binariesBeforeSha256=binaries,testExecutablesSha256=test_binaries,finished=time.time()))
 (out/'steps.json').write_text(json.dumps(events,indent=2)+'\n')
 (out/f'cgroup-step-{index}.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}))
 if code:raise SystemExit(code)
 if binaries:assert all(hashlib.sha256(pathlib.Path('/target/debug/examples',name).read_bytes()).hexdigest()==sha for name,sha in binaries.items())

after={name:hashlib.sha256(pathlib.Path('/work/source',name).read_bytes()).hexdigest() for name in expected}
(out/'inside-source-after.json').write_text(json.dumps(after,indent=2));assert after==expected
