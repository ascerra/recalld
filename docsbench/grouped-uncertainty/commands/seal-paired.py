from pathlib import Path
import hashlib,json,re,shutil,subprocess
root=Path('/tmp/recalld-implementation/a6-grouped-uncertainty');base=Path('/tmp/recalld-owned-native-controls-validation');dest=root/'docsbench/grouped-uncertainty';final='427c57e9e00bfa32f09edff22a479b31e656d269'
summary=dict(sourceCommit=final,directParent='6265d05ae9fd52177dc0603d179c7c082d82b755',runs={},scope='synthetic paired public-v1 limit contrast; no quality promotion/full A6/native/reset/token/cost/performance claim')
for label,dirname,expect in [('compile-failure','paired-edc2394b','failed'),('checker-failure','paired-93598b71','failed'),('final-tail','paired-427c57e9','passed')]:
 out=base/dirname/'output';build=json.loads((out/'build.json').read_text());pins=build['sourceFilesSha256'];assert build['status']==expect;assert pins==build['sourceAfterSha256']
 for n,sha in pins.items():assert hashlib.sha256(subprocess.check_output(['git','show',build['sourceCommit']+':'+n],cwd=root)).hexdigest()==sha,n
 for n in ('inside-source-before.json','inside-source-after.json'):assert json.loads((out/n).read_text())==pins
 target=dest/label;target.mkdir()
 for p in sorted(out.rglob('*')):
  if not p.is_file() or p.suffix not in {'.json','.jsonl','.log','.py'}:continue
  to=target/p.relative_to(out);to.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,to)
 steps=json.loads((out/'steps.json').read_text());data=dict(sourceCommit=build['sourceCommit'],status=expect,sourceFiles=len(pins),gitAndPrePostVerified=True,steps=steps,
  cgroupFinal=json.loads(sorted(out.glob('cgroup-step-*.json'),key=lambda p:int(p.stem.split('-')[-1]))[-1].read_text()),
  testResults={p.name:re.findall(r'test result:.*',p.read_text(errors='replace')) for p in out.glob('step-*.log') if 'test result:' in p.read_text(errors='replace')})
 if label=='compile-failure':data['testExecutableClaim']='None: compile failed. Listed glob hashes in original steps are historical cache inventory, not executed binaries.'
 else:
  data['actualPairStatus']=json.loads((out/'actual/paired/pair-status.json').read_text());assert data['actualPairStatus']['passedMembers']==14
 summary['runs'][label]=data
assert subprocess.check_output(['git','diff','93598b71',final,'--','src','Cargo.toml','Cargo.lock','examples'],cwd=root)==b''
summary['unchangedRustCargoSince']='93598b713feb8da18a0df7ccbe3da61f5ecd1970'
summary['rustProofReuse']='8 focused scorer tests/fmt/example clippy on93598; final tail changes only checker script. Scorer rebuilt for new embedded source pins, production CLI byte-identical.'
summary['validation']=json.loads((dest/'final-tail/actual/validation.json').read_text());summary['scores']=json.loads((dest/'final-tail/actual/paired/paired-scores.json').read_text())
summary['finalExecutables']=summary['runs']['final-tail']['steps'][-1]['binariesBeforeSha256'];assert summary['finalExecutables']['/target/debug/recalld']==summary['runs']['checker-failure']['steps'][-1]['binariesBeforeSha256']['/target/debug/recalld']
summary['cargoLockSha256']=hashlib.sha256((root/'Cargo.lock').read_bytes()).hexdigest()
registry=Path('/tmp/recalld-unit-tests-l__ja1yo/cargo/registry/src/index.crates.io-1949cf8c6b5b557f')
summary['statisticalImplementationFiles']={str(p.relative_to(registry)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [registry/'rand_chacha-0.3.1/src/chacha.rs',registry/'rand-0.8.6/src/distributions/uniform.rs']}
summary['sourceDiffFinalVsFocused']=subprocess.check_output(['git','diff','--stat','93598b71',final],cwd=root,text=True)
(dest/'validation.json').write_text(json.dumps(summary,indent=2)+'\n')
(dest/'statistical-policy.json').write_text(json.dumps(dict(policy=summary['scores']['policy'],cargoLockSha256=summary['cargoLockSha256'],implementationFiles=summary['statisticalImplementationFiles'],drawHashEncoding='Each sampled zero-based group index as unsigned64 little-endian, concatenated in replicate order; groups ordered by group ID',trainDrawSha256=summary['scores']['splits']['train']['paired']['drawsSha256']),indent=2)+'\n')
commands=dest/'commands';commands.mkdir()
for name in ['run-paired.py','run-paired-tail.py','seal-paired.py']:shutil.copyfile(base/name,commands/name)
for name in ['recalld-paired-inherited-python.log','recalld-paired-python-draft.log']:
 p=Path('/tmp')/name
 if p.exists():shutil.copyfile(p,dest/name)
print(json.dumps(dict(final=final,files=sum(p.is_file() for p in dest.rglob('*')),bytes=sum(p.stat().st_size for p in dest.rglob('*') if p.is_file()),binaries=summary['finalExecutables'])))
