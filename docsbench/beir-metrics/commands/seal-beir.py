from pathlib import Path
import hashlib,json,subprocess,shutil
root=Path('/tmp/recalld-implementation/a6-beir-metrics');dest=root/'docsbench/beir-metrics';base=Path('/tmp/recalld-owned-native-controls-validation');out=base/'beir-1a793ee7/output'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
build=json.loads((out/'build.json').read_text());head=build['sourceCommit'];pins=build['sourceFilesSha256'];assert build['status']=='passed' and pins==build['sourceAfterSha256']
for n,v in pins.items(): assert hashlib.sha256(subprocess.check_output(['git','show',head+':'+n],cwd=root)).hexdigest()==v,n
for n in ('inside-source-before.json','inside-source-after.json'):assert json.loads((out/n).read_text())==pins
assert subprocess.check_output(['git','diff','36b7f840ff65b95a3805503e0134f0db498984cf',head,'--','src','examples','Cargo.toml','Cargo.lock'],cwd=root)==b''
installed=json.loads((base/'beir-dependency-002/installed-files.json').read_text());current={str(p.relative_to(base/'beir-dependency-002/site')):sha(p) for p in (base/'beir-dependency-002/site').rglob('*') if p.is_file()};assert current==installed==json.loads((root/'scripts/beir-reference-lock.json').read_text())['files']
for folder in ('beir-dependency-001','beir-dependency-002','beir-dependency-002-import'):
 target=dest/folder;target.mkdir(exist_ok=True)
 for p in (base/folder).iterdir():
  if p.is_file() and p.suffix in ('.json','.txt','.log','.py','.stdout','.stderr'):
   shutil.copyfile(p,target/p.name)
# Output contains only compact JSON/wire/log/probe evidence, never copied sources, stores or binaries.
for p in out.rglob('*'):
 if not p.is_file():continue
 target=dest/'actual-1a793ee7'/p.relative_to(out);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,target)
original=Path('/Users/caleb.evans/repos/recalld/docs/research/2026-11-recalld/evidence/a6-beir-metric-plan-2026-11-15')
shutil.copytree(original/'notices',dest/'notices',dirs_exist_ok=True)
shutil.copyfile(original/'image-abi-metadata.txt',dest/'image-abi-metadata.txt')
commands=dest/'commands';commands.mkdir(exist_ok=True)
for name in ('run-beir.py','run-beir-dependency.py','run-beir-dependency-002.py','seal-beir.py'):shutil.copyfile(base/name,commands/name)
summary=dict(sourceCommit=head,directParent='36b7f840ff65b95a3805503e0134f0db498984cf',runtimeCargoExamplesUnchangedFromParent=True,sourceFiles=len(pins),gitPrePostSourceVerified=True,
 installedFiles=len(installed),installedReferencePrePostVerified=True,referenceLockSha256=sha(root/'scripts/beir-reference-lock.json'),
 validation=json.loads((out/'actual/validation.json').read_text()),independentPostflight=json.loads((out/'independent-postflight.json').read_text()),
 steps=json.loads((out/'steps.json').read_text()),cgroupFinal=json.loads((out/'cgroup-step-5.json').read_text()),
 referenceIntegrity=json.loads((out/'reference-integrity.log').read_text()),dependencyBuild=json.loads((base/'beir-dependency-002/PASS.json').read_text()),
 pythonTests=dict(newReference=9,existingPublic=16,existingGrouped=23),scorerExampleTests=5,
 limitations=['synthetic contract only','external research-only reference dependency','no source/engine score normalization','known-empty reference proof is not actual empty-catalog readiness proof','no full Rust suite or performance claim'])
(dest/'validation.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(dict(source=head,sourceFiles=len(pins),installedFiles=len(installed),copiedBytes=sum(p.stat().st_size for p in dest.rglob('*') if p.is_file()),files=sum(p.is_file() for p in dest.rglob('*'))),indent=2))
