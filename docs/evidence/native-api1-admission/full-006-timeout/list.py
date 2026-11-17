import pathlib,json,subprocess,hashlib
out=pathlib.Path('/work/output'); artifacts=json.loads((out/'executables-before.json').read_text()); inventory={}
for i,(exe,meta) in enumerate(artifacts.items()):
 assert hashlib.sha256(pathlib.Path(exe).read_bytes()).hexdigest()==meta['sha256']
 result=subprocess.run([exe,'--list','--format=terse'],capture_output=True,text=True,timeout=30)
 assert result.returncode==0,(exe,result.stderr)
 (out/f'inventory-{i:02d}.txt').write_text(result.stdout)
 names=[]
 for line in result.stdout.splitlines():
  if line.endswith(': test'): names.append(line[:-6])
  elif line.endswith(': benchmark'): raise RuntimeError('Unexpected benchmark: '+line)
 assert len(names)==len(set(names))
 inventory[exe]={'artifact':meta,'tests':names,'list_file':f'inventory-{i:02d}.txt'}
 if meta['kind']==['lib']:
  help=subprocess.run([exe,'--help'],capture_output=True,text=True,timeout=30,check=True)
  (out/'test-harness-help.txt').write_text(help.stdout)
(out/'test-inventory.json').write_text(json.dumps(inventory,indent=2)+'\n')
print([(x['artifact']['name'],len(x['tests'])) for x in inventory.values()])
