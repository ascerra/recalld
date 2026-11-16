import copy,json,pathlib,subprocess
out=pathlib.Path('/output');original=json.loads((out/'episode/evaluator/original-input.json').read_text());rows=[]
changes=[('v1-with-policy',lambda v:v.update(fixtureVersion=1)),('v1-with-null-policy',lambda v:v.update(fixtureVersion=1,namespaceDecayMultipliers=None)),('v2-wrong-policy',lambda v:v.update(namespaceDecayMultipliers={'foreign':1})),('v2-without-policy',lambda v:v.pop('namespaceDecayMultipliers')),('unknown-version',lambda v:v.update(fixtureVersion=3))]
for name,change in changes:
 value=copy.deepcopy(original);change(value);source=out/('fixture-negative-'+name+'.json');source.write_text(json.dumps(value));dest=out/('fixture-destination-'+name);report=out/('fixture-report-'+name+'.json')
 command=['/target/debug/examples/native_prefix_fixture','--input',str(source),'--root',str(dest),'--report',str(report)]
 with (out/('fixture-negative-'+name+'.log')).open('wb') as log:code=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT).returncode
 rows.append(dict(case=name,command=command,exitCode=code,destinationExists=dest.exists(),reportExists=report.exists()));(out/'fixture-negatives.json').write_text(json.dumps(rows,indent=2))
 assert code!=0 and not dest.exists() and not report.exists(),name
