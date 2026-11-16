import hashlib, json, pathlib, shutil, subprocess, time

base = pathlib.Path('/tmp/recalld-implementation/merge-queue/.queue')
prior = base / 'validation-pr184-90daa782'
old = prior / 'output'
run = base / 'validation-pr184-90daa782-tail'
out = run / 'output'
b = json.loads((old / 'build.json').read_text())
steps = json.loads((old / 'steps.json').read_text().removesuffix('\\n'))
assert b['sourceCommit'] == '90daa782e9482ff265b03969fcfae393aaf75660' and b['status'] == 'failed'
assert len(steps) == 18 and all(s['exitCode'] == 0 for s in steps)
assert 'zero-match test filter' in (old / 'container.log').read_text()
assert 'running 0 tests' in (old / 'step-17.log').read_text()
assert b['sourceFilesSha256'] == b['sourceAfterSha256']
exe = 'debug/deps/recalld-8b31964f64b2cfc4'
sha = steps[17]['testExecutablesSha256'][exe]
assert sha == '4af09b83bec0e9b7c802bf199c2e0ce6c944a3dcb71d91edabf69b05f38dcf25'
assert hashlib.sha256((base / 'validation-pr22-a20ab593/target' / exe).read_bytes()).hexdigest() == sha
out.mkdir(parents=True)
for name in ('schema-wheels.json', 'check-schema.py', 'check-public.py'):
    shutil.copyfile(old / name, out / name)
shutil.copytree(old / 'schema-fixtures', out / 'schema-fixtures')
schema_pins = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (old / 'schema-fixtures').glob('*.json')}
assert schema_pins == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (out / 'schema-fixtures').glob('*.json')}
(out / 'schema-fixture-pins.json').write_text(json.dumps(schema_pins, indent=2))
commands = [
    ['/work/target/' + exe, 'native_catalog_adapter::tests::', '--list'],
    ['/work/target/' + exe, 'native_catalog_adapter::tests::', '--nocapture'],
    ['python3', '/work/output/check-schema.py'],
    ['cargo', 'clippy', '--offline', '--locked', '-j1', '--all-features', '--all-targets'],
    ['cargo', 'build', '--offline', '--locked', '-j1', '--features', 'bench', '--bin', 'recalld'],
    ['python3', 'scripts/public-conformance.py', '--inside', '--fixture-version', '3'],
    ['python3', '/work/output/check-public.py'],
]
script = '''import pathlib,json,hashlib,subprocess,time,re
out=pathlib.Path('/work/output');b=json.loads((out/'build.json').read_text());pins=b['sourceFilesSha256']
def sources():return {n:hashlib.sha256(pathlib.Path('/work/source',n).read_bytes()).hexdigest() for n in pins}
before=sources();assert before==pins;(out/'inside-source-before.json').write_text(json.dumps(before,indent=2))
events=[]
for index,command in enumerate(json.loads((out/'commands.json').read_text())):
 binaries={}
 if index<2:binaries[command[0]]=hashlib.sha256(pathlib.Path(command[0]).read_bytes()).hexdigest();assert binaries[command[0]]==b['sameTestExecutableSha256']
 if index>=5:binaries['/work/target/debug/recalld']=hashlib.sha256(pathlib.Path('/work/target/debug/recalld').read_bytes()).hexdigest()
 with (out/f'step-{index}.log').open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 events.append(dict(command=command,exitCode=code,binariesBeforeSha256=binaries,finished=time.time()))
 (out/'steps.json').write_text(json.dumps(events,indent=2))
 (out/f'cgroup-step-{index}.json').write_text(json.dumps({n:pathlib.Path('/sys/fs/cgroup',n).read_text() for n in ('memory.max','memory.peak','memory.events')}))
 if code:raise SystemExit(code)
 text=(out/f'step-{index}.log').read_text()
 if index==0:assert len(re.findall(r'^native_catalog_adapter::tests::[^ ]+: test$',text,re.M))==10,text
 if index==1:assert re.search(r'test result: ok[.] 10 passed; 0 failed; 0 ignored;',text),text
 for name,sha in binaries.items():assert hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest()==sha
after=sources();assert after==pins;(out/'inside-source-after.json').write_text(json.dumps(after,indent=2))
'''
(out / 'execute.py').write_text(script)
(out / 'commands.json').write_text(json.dumps(commands, indent=2))
args = list(b['command'])
name = 'recalld-merge-queue-pr184-tail'
args[args.index('--name') + 1] = name
args = [a.replace('src=' + str(old) + ',dst=/work/output', 'src=' + str(out) + ',dst=/work/output') for a in args]
assert sum('dst=/work/output' in a for a in args)==1 and any('src='+str(out)+',dst=/work/output' in a for a in args)
b.update(status='running', started=time.time(), command=args, cache='Same frozen source and verified test executable; no clean or source switch', parentFailedRun=str(prior), sameTestExecutableSha256=sha,
         validationScope='Same frozen90daa source/executable continuation after retained zero-filter driver failure. Exact10 catalog/policy cases then61schema/lint/CLI/public9; no full rerun or source change.')
b.pop('exitCode', None)
b.pop('finished', None)
(out / 'build.json').write_text(json.dumps(b, indent=2))
with (out / 'container.log').open('wb') as log:
    code = subprocess.run(args, stdout=log, stderr=subprocess.STDOUT).returncode
inspection = subprocess.run(['docker', 'inspect', name], capture_output=True, text=True)
(out / 'container-inspect.json').write_text(inspection.stdout)
subprocess.run(['docker', 'rm', name], check=True, stdout=subprocess.DEVNULL)
after = {n: hashlib.sha256((prior / 'source' / n).read_bytes()).hexdigest() for n in b['sourceFilesSha256']}
assert after == b['sourceFilesSha256']
b.update(status='passed' if code == 0 else 'failed', exitCode=code, finished=time.time(), sourceAfterSha256=after)
(out / 'build.json').write_text(json.dumps(b, indent=2))
print(json.dumps(dict(source=b['sourceCommit'], run=str(run), exitCode=code)), flush=True)
raise SystemExit(code)
