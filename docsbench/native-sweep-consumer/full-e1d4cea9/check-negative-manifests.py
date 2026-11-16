import copy,json,pathlib,subprocess
out=pathlib.Path('/output');original=json.loads((out/'inputs/manifest.json').read_text());rows=[]
changes=[('manifest-version',lambda v:v.update(manifestVersion=2)),('sweep-origin',lambda v:v['versions'].update(sweepOriginalContractVersion=2)),('native-writes',lambda v:v['capabilities'].update(nativeWrites=True)),('physical-compaction',lambda v:v['capabilities'].update(nativePhysicalCompaction=True)),('canonical-schema',lambda v:v['versions'].update(canonicalSchemaVersion=1)),('multiplier',lambda v:v['capabilities'].update(sweepDecayRateMultiplier=0)),('rng',lambda v:v['capabilities'].update(deterministicRng=True)),('full-reset',lambda v:v['capabilities'].update(reset=True)),('source-hash',lambda v:v['sourceFilesSha256'].update({'src/bench/native_prefix.rs':'0'*64})),('binary-hash',lambda v:v['executables'].update(ownedEvaluator='0'*64))]
for name,change in changes:
 value=copy.deepcopy(original);change(value);manifest=out/'inputs'/('negative-'+name+'.json');manifest.write_text(json.dumps(value));dest=out/('negative-output-'+name)
 command=['python3','scripts/public-mcp-evaluation.py','run','--binary','/target/debug/examples/owned_evaluator','--scorer','/target/debug/examples/score_mcp_episode','--source-root','/work/source','--manifest',str(manifest),'--out',str(dest)]
 with (out/('negative-'+name+'.log')).open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 rows.append(dict(case=name,command=command,exitCode=code,outputExists=dest.exists()));(out/'manifest-negatives.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert code!=0 and not dest.exists(),name
