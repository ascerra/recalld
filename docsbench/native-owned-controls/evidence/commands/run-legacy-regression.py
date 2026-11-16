from pathlib import Path
import json,hashlib,subprocess,time
base=Path('/tmp/recalld-owned-native-controls-validation');run=base/'full-392034d5';out=run/'output';build=json.loads((out/'build.json').read_text())
assert build['status']=='passed','full run must finish before same-binary regression'
steps=json.loads((out/'steps.json').read_text());pins=steps[-1]['binariesBeforeSha256'];assert pins
for name,sha in pins.items():assert hashlib.sha256((base/'target/debug/examples'/name).read_bytes()).hexdigest()==sha
commands=[['python3','scripts/public-mcp-evaluation.py','prepare','--track','serialized-owned-sweep-synthetic-diagnostic-v1','--binary','/target/debug/examples/owned_evaluator','--scorer','/target/debug/examples/score_mcp_episode','--build','/output/build.json','--out','/output/legacy-inputs'],['python3','scripts/public-mcp-evaluation.py','run','--binary','/target/debug/examples/owned_evaluator','--scorer','/target/debug/examples/score_mcp_episode','--source-root','/work/source','--manifest','/output/legacy-inputs/manifest.json','--out','/output/legacy-episode']]
command=build['command'][:];command[command.index('--name')+1]='recalld-owned-native-controls-legacy';command[-2:]=['python3','/output/legacy-execute.py']
negative_script = """import copy,json,pathlib,subprocess
out=pathlib.Path('/output');base=json.loads((out/'inputs/manifest.json').read_text());records=[]
changes=[('manifest-version',lambda v:v.update(manifestVersion=2)),('schema-absence',lambda v:v['versions'].update(canonicalSchemaVersion=None)),('schema-zero',lambda v:v['versions'].update(canonicalSchemaVersion=0)),('public-v2',lambda v:v['versions'].update(publicContractVersion=2)),('all-rng',lambda v:v['capabilities'].update(deterministicRng=True)),('full-a6',lambda v:v['capabilities'].update(fullA6=True)),('native-writes',lambda v:v['capabilities'].update(nativeWrites=True)),('native-sweep',lambda v:v['capabilities'].update(nativeSweep=True)),('ordinary-startup',lambda v:v['capabilities'].update(ordinaryNativeStartup=True)),('executable-pin',lambda v:v['executables'].update(nativePrefixFixture='0'*64))]
for name,change in changes:
 m=copy.deepcopy(base);change(m);path=out/'inputs'/('negative-'+name+'.json');path.write_text(json.dumps(m));dest=out/('must-not-exist-'+name)
 cmd=['python3','scripts/public-mcp-evaluation.py','run','--binary','/target/debug/examples/owned_evaluator','--scorer','/target/debug/examples/score_mcp_episode','--source-root','/work/source','--manifest',str(path),'--out',str(dest)]
 with (out/('manifest-negative-'+name+'.log')).open('wb') as log:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
 records.append(dict(case=name,exitCode=r.returncode,outputExists=dest.exists(),command=cmd));(out/'manifest-negatives.json').write_text(json.dumps(records,indent=2)+'\\n')
 assert r.returncode!=0 and not dest.exists(),name
"""
(out/'manifest-negatives.py').write_text(negative_script)
commands.insert(0,['python3','/output/manifest-negatives.py'])
(out/'legacy-execute.py').write_text('import subprocess,json,pathlib\nout=pathlib.Path("/output")\ncommands='+repr(commands)+'\nfor i,c in enumerate(commands):\n with (out/f"legacy-step-{i}.log").open("wb") as log: code=subprocess.run(c,stdout=log,stderr=subprocess.STDOUT).returncode\n if code: raise SystemExit(code)\n')
record=dict(command=command,commands=commands,binariesBeforeSha256=pins,sourceCommit=build['sourceCommit'],started=time.time(),reason='shared initializer and schema-dispatched checkpoint regression; no Rust suite repeat')
with (out/'legacy-container.log').open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
record.update(exitCode=code,finished=time.time(),binariesAfterSha256={name:hashlib.sha256((base/'target/debug/examples'/name).read_bytes()).hexdigest() for name in pins})
assert record['binariesAfterSha256']==pins
assert all(hashlib.sha256((run/'source'/name).read_bytes()).hexdigest()==sha for name,sha in build['sourceFilesSha256'].items())
(out/'legacy-regression.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(exitCode=code,output=str(out/'legacy-episode'))));raise SystemExit(code)
