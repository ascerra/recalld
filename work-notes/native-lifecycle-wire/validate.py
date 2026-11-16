import pathlib,hashlib,json,tarfile,subprocess,sys,time
root=pathlib.Path('.test-scratch/final')
paths=sorted(list(pathlib.Path('src').rglob('*.rs'))+[pathlib.Path('Cargo.toml'),pathlib.Path('Cargo.lock')])
def hashes():return {str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
expected=json.load(open(root/'input.json'));assert hashes()==expected,'mounted source differs from host'
commands=[('fmt',['cargo','fmt','--all']),('focused',['cargo','test','--offline','--lib','native_lifecycle_wire','--','--nocapture']),('full',['cargo','test','--offline']),('fmt-check',['cargo','fmt','--all','--','--check']),('clippy',['cargo','clippy','--offline','--all-targets'])]
manifest={'commands':[]}
for name,cmd in commands:
 start=time.time_ns()
 with open(root/(name+'.log'),'w') as out:result=subprocess.run(cmd,stdout=out,stderr=subprocess.STDOUT)
 manifest['commands'].append(dict(name=name,command=cmd,exit_code=result.returncode,start_ns=start,end_ns=time.time_ns()))
 if name=='fmt':
  expected=hashes();json.dump(expected,open(root/'source.json','w'),indent=2)
  with tarfile.open(root/'source.tar.gz','w:gz') as archive:
   for p in paths:archive.add(p)
 else:assert hashes()==expected,'source changed during validation'
 binaries=[]
 for p in sorted(pathlib.Path('/target/debug/deps').glob('recalld-*')):
  if p.is_file() and p.stat().st_mode&0o111:binaries.append(dict(name=p.name,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),size=p.stat().st_size,mtime_ns=p.stat().st_mtime_ns))
 manifest['binaries']=binaries;json.dump(manifest,open(root/'run.json','w'),indent=2)
 print(name,result.returncode,flush=True)
 if result.returncode:sys.exit(result.returncode)
