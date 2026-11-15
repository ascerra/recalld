import pathlib,hashlib,json,sys,tarfile,os
root=pathlib.Path('/work' if pathlib.Path('/work/Cargo.toml').exists() else '.')
files=sorted(set([p for d in ['src','tests'] for p in (root/d).rglob('*') if p.is_file()]+[p for p in root.glob('Cargo.*')]+[p for p in root.glob('rust-toolchain*')]))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
mode,dest=sys.argv[1:];dest=root/dest;dest.mkdir(parents=True,exist_ok=True)
hashes={str(p.relative_to(root)):sha(p) for p in files}
if mode=='input':(dest/'input.json').write_text(json.dumps(hashes,indent=2)+'\n')
if mode=='verify':
 expected=json.loads((dest/'input.json').read_text());assert expected==hashes,'source mount mismatch'
 print('verified mounted inputs',len(hashes))
if mode=='seal':
 (dest/'source.json').write_text(json.dumps(hashes,indent=2)+'\n')
 with tarfile.open(dest/'source.tar.gz','w:gz') as t:
  for p in files:t.add(p,arcname=str(p.relative_to(root)))
 print('sealed',len(hashes),'inputs')
if mode=='binary':
 target=pathlib.Path(os.environ.get('CARGO_TARGET_DIR',str(root/'.test-scratch/target')))/'debug/deps'
 out=[]
 for p in target.glob('*'):
  if p.is_file() and os.access(p,os.X_OK) and p.name.startswith('recalld-'):
   s=p.stat();out.append({'path':str(p),'sha256':sha(p),'size':s.st_size,'mtime_ns':s.st_mtime_ns})
 (dest/'binaries.json').write_text(json.dumps(out,indent=2)+'\n');print('sealed executables',len(out))
