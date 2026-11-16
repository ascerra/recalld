from pathlib import Path
import json,hashlib,subprocess,shutil
root=Path('/tmp/recalld-implementation/a6-tokenized-context');base=Path('/tmp/recalld-owned-native-controls-validation');dest=root/'docsbench/tokenized-context';source='5426da206a354924e26be8920a735b755550f1c7'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
summary=dict(sourceCommit=source,directParent='003d857dd99acca9eb33801fa6c5d7b137d124a2',runs={},scope='Actual encoding-specific rendered-context budget and source-evidence consumption; no model identity/answer quality/cost/full A6')
for label,name,status in [('fixture-failure','context-aeec2ebe','failed'),('store-rejection','context-1518c449','failed'),('final','context-5426da20','passed')]:
 out=base/name/'output';build=json.loads((out/'build.json').read_text());pins=build['sourceFilesSha256'];assert build['status']==status;assert pins==build['sourceAfterSha256']
 for n,v in pins.items():assert hashlib.sha256(subprocess.check_output(['git','show',build['sourceCommit']+':'+n],cwd=root)).hexdigest()==v,n
 for n in ['inside-source-before.json','inside-source-after.json']:assert json.loads((out/n).read_text())==pins
 target=dest/label;target.mkdir()
 for p in sorted(out.rglob('*')):
  if not p.is_file() or 'tokenizer' in p.relative_to(out).parts or 'corrupt-assets' in p.relative_to(out).parts or p.suffix not in {'.json','.jsonl','.log','.py','.toml'}:continue
  copy=target/p.relative_to(out);copy.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,copy)
 lock=json.loads(subprocess.check_output(['git','show',build['sourceCommit']+':scripts/context-tokenizer-lock.json'],cwd=root));bundle=out/'tokenizer';installed={str(p.relative_to(bundle/'site')):sha(p) for p in (bundle/'site').rglob('*') if p.is_file()};assert installed==lock['installedFilesSha256'];assert sha(bundle/'cl100k_base.tiktoken')==lock['rankSha256'];assert sha(bundle/'context-encoding.json')==lock['configSha256']
 install=json.loads((bundle/'installation.json').read_text());assert install['installedFilesSha256']==installed and install['wheels']==lock['wheels']
 (target/'installed-tokenizer-prepost.json').write_text(json.dumps(dict(beforeInstallation=install,afterInstalledFilesSha256=installed,afterRankSha256=sha(bundle/'cl100k_base.tiktoken'),afterConfigSha256=sha(bundle/'context-encoding.json'),allEqual=True),indent=2)+'\n')
 steps=json.loads((out/'steps.json').read_text());last=sorted(out.glob('cgroup-step*.json'),key=lambda p:int(p.stem.split('-')[-1]))[-1]
 summary['runs'][label]=dict(sourceCommit=build['sourceCommit'],status=status,sourceFiles=len(pins),gitAndPrePostVerified=True,steps=steps,cgroupFinal=json.loads(last.read_text()),installedFiles=len(installed),installedAssetsPrePostVerified=True)
assert subprocess.check_output(['git','diff','003d857d',source,'--','src','examples','Cargo.toml','Cargo.lock'],cwd=root)==b''
summary['productionRustCargoAndExamplesUnchangedFrom90']=True
summary['validation']=json.loads((dest/'final/actual/validation.json').read_text());summary['executables']=summary['runs']['final']['steps'][-1]['binariesBeforeSha256']
summary['nativeExtensionsSha256']={p:v for p,v in installed.items() if p.endswith('.so')}
summary['tokenizerLockSha256']=sha(root/'scripts/context-tokenizer-lock.json');summary['encodingConfigSha256']=sha(root/'scripts/context-encoding.json')
(dest/'validation.json').write_text(json.dumps(summary,indent=2)+'\n');commands=dest/'commands';commands.mkdir()
for n in ['run-context.py','seal-context.py']:shutil.copyfile(base/n,commands/n)
shutil.copyfile('/tmp/recalld-tokenizer-prefetch/download.log',dest/'dependency-prefetch.log')
print(json.dumps(dict(source=source,executables=summary['executables'],nativeExtensions=summary['nativeExtensionsSha256'],runs={k:v['cgroupFinal'] for k,v in summary['runs'].items()},files=sum(p.is_file() for p in dest.rglob('*')),bytes=sum(p.stat().st_size for p in dest.rglob('*') if p.is_file())),indent=2))
