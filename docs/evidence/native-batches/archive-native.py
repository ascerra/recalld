from pathlib import Path
import hashlib,json,gzip,re,subprocess,datetime
root=Path.cwd(); v=root/'.validation'; ev=root/'docs/evidence/native-batches'
ev.mkdir(parents=True,exist_ok=True)
sha=lambda data:hashlib.sha256(data).hexdigest()
run=lambda *args:subprocess.check_output(args,cwd=root)
runtime='febe91310153f2caab9209ca4cfae0d9fbbc5844'
source=json.loads((v/'source-before-final.json').read_text())
assert source==json.loads((v/'source-after-final.json').read_text())
for name,digest in source.items():
 assert sha((root/name).read_bytes())==digest,name
 assert sha(run('git','show',runtime+':'+name))==digest,name
log=(v/'native-full-final.log').read_text()
assert 'test result: FAILED' not in log
assert 'Finished `dev` profile' in log
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;[^\n]*',log)
assert results and all(failed=='0' for _,failed,_ in results)
binaries={}
for name in sorted(set(re.findall(r'\(/target/([^\)]+)\)',log))):
 p=v/'target'/name
 if p.is_file():
  h=hashlib.sha256()
  with p.open('rb') as f:
   for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
  binaries[name]=h.hexdigest()
assert any('deps/recalld-' in p for p in binaries)
logs={}
for p in sorted(v.glob('*.log')):
 if p.name.startswith('archive-'):continue
 data=p.read_bytes();logs[p.name]=sha(data);(ev/(p.name+'.gz')).write_bytes(gzip.compress(data,mtime=0))
for pattern in ('*source.json','*binary.json','*source.tar.gz','*.patch.gz'):
 for p in v.glob(pattern):(ev/p.name).write_bytes(p.read_bytes())
for name in ('source-manifest.py','validate-final.sh','validate-full.sh','source-before-full.json','source-after-full.json','source-before-final.json','source-after-final.json','archive-native.py'):
 (ev/name).write_bytes((v/name).read_bytes())
(ev/'final-source.tar.gz').write_bytes(gzip.compress(run('git','archive',runtime,'src','tests','Cargo.toml','Cargo.lock'),mtime=0))
(ev/'source-adjustments.patch.gz').write_bytes(gzip.compress(b'\n'.join(run('git','show','--format=fuller','--remerge-diff',commit,'--','src','tests') for commit in ('4328577','206b745','b425240','010d2ad','cb3bd31',runtime)),mtime=0))
artifacts={p.name:sha(p.read_bytes()) for p in sorted(ev.iterdir()) if p.is_file() and p.name not in ('manifest.json','README.md')}
manifest={'runtime':runtime,'parents':run('git','show','-s','--format=%P',runtime).decode().strip().split(),'suite_results_pass_failed_ignored':results,'source_files_sha256':source,'test_executables_sha256':binaries,'full_logs_sha256':logs,'retained_artifacts_sha256':artifacts,'toolchain':'1.94.1','image':'recalld-ci:1.94','image_id':'sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef','network':'none','cpus':2,'memory':'4g','cargo_build_jobs':2,'source_root_switching':False,'initial_copied_package_cleaned':True,'guard_binary_provenance':'guard binary built before formatting; guard-source.json captured after formatting and is not claimed as exact binary source. Final source/binary pairing is exact.','archived_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
(ev/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
print(json.dumps({'runtime':runtime,'suite_results':results,'source_files':len(source),'artifact_files':len(artifacts)},indent=2))
