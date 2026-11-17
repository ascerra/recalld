import pathlib,json,subprocess,hashlib,re,time,math
out=pathlib.Path('/work/output');plan=json.loads((out/'continuation-plan.json').read_text())
artifacts=json.loads((out/'executables-before.json').read_text())
artifacts.update(json.loads((out/'subprocess-executables-before-cli.json').read_text())['executables'])
def verify():
 for exe,meta in artifacts.items():
  h=hashlib.sha256()
  with pathlib.Path(exe).open('rb') as f:
   for data in iter(lambda:f.read(1024*1024),b''):h.update(data)
  assert h.hexdigest()==meta['sha256'],exe
verify();(out/'executables-before-verified.txt').write_text('All14 test +4 subprocess executables match006.\n')
exe=plan['library_executable'];names=plan['remaining_library'];assert len(names)==len(set(names))==277
base=[exe,'--exact','--test-threads=1',*names]
listing=subprocess.run([*base,'--list','--format=terse'],capture_output=True,text=True,timeout=30,check=True)
(out/'selected-list.txt').write_text(listing.stdout)
selected=[line[:-6] for line in listing.stdout.splitlines() if line.endswith(': test')]
assert len(selected)==len(set(selected))==277 and set(selected)==set(names)
(out/'selected-list-verified.json').write_text(json.dumps({'count':len(selected),'exact_set_equality':True,'names':selected},indent=2)+'\n')
results=[]
def run(label,command,seconds,expected):
 argv=['timeout','--signal=TERM','--kill-after=10s',str(seconds)+'s',*command]
 (out/(label+'-command.json')).write_text(json.dumps(argv,indent=2)+'\n')
 start=time.monotonic()
 with (out/(label+'.log')).open('w') as log: code=subprocess.call(argv,stdout=log,stderr=subprocess.STDOUT)
 text=(out/(label+'.log')).read_text()
 summaries=re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;.*$',text,re.M)
 last=tuple(map(int,summaries[-1])) if summaries else None
 row={'label':label,'expected':expected,'exit':code,'root_summary':last,'seconds':time.monotonic()-start}
 results.append(row);(out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
 if code: raise SystemExit(code)
 assert last==(expected,0,0),(label,last,expected)
run('library-remaining',base,1200,277)
deadline=time.monotonic()+600
for i,(binary,item) in enumerate(plan['other_harnesses'].items()):
 seconds=math.floor(deadline-time.monotonic());assert seconds>0,'combined other-harness deadline exceeded'
 label=f'other-{i:02d}-'+item['artifact']['name']
 run(label,[binary,'--test-threads=1'],seconds,len(item['tests']))
assert sum(x['root_summary'][0] for x in results)==302
verify();(out/'executables-after-verified.txt').write_text('All14 test +4 subprocess executables unchanged after continuation, before lint.\n')
(out/'coverage.json').write_text(json.dumps({'runtime':plan['runtime'],'prior_confirmed_library':len(plan['confirmed_single_ok']),'prior_ignored_library':len(plan['confirmed_ignored']),'continued_library':277,'other_tests':25,'total_library_covered':len(plan['confirmed_single_ok'])+len(plan['confirmed_ignored'])+277,'single_full_command_pass':False,'note':'Combined006/007 exactinventory coverage;006 remainsTIMEOUT124. Allfeatures lint is separate.'},indent=2)+'\n')
