import hashlib,json,pathlib,subprocess,sys,time,os
r=pathlib.Path('/evidence')
exes=[p for p in pathlib.Path('/target/debug/deps').glob('recalld-*') if p.is_file() and os.access(p,os.X_OK) and not p.suffix]
assert len(exes)==1,exes
e=exes[0]
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1048576),b''):h.update(b)
 return h.hexdigest()
proof={'path':str(e),'sha256':sha(e),'bytes':e.stat().st_size}
(r/'executable-before.json').write_text(json.dumps(proof,indent=2)+'\n')
filters=['native_api1_purge_success_counts_actual_orphan_fts_rows_and_directory','native_api1_purge_race_actual_mcp_eras_remote_and_legacy_frames','native_api1_purge_full_start_and_child_errors_survive_actual_frames','native_api1_purge_structured_remote_validation_keeps_f32_bits_and_rejects_extras','native_api1_purge_unknown_outcomes_survive_actual_bounded_frames','w31_a_forced_delete_removes_the_records_indexes_and_directory','live_metadata_schemas_actual_outputs_preserve_eras_scope_and_truncation','w52_a_partial_purge_says_partially_destroyed_not_failed','w53_an_ordinary_delete_failure_keeps_the_failed_prefix']
results=[]
for name in filters:
 start=time.monotonic()
 with (r/(name+'.log')).open('wb') as log:p=subprocess.run([str(e),name,'--nocapture','--test-threads=2'],stdout=log,stderr=subprocess.STDOUT)
 results.append({'filter':name,'exit':p.returncode,'seconds':time.monotonic()-start})
 (r/'focused-results.json').write_text(json.dumps(results,indent=2)+'\n');print(name,p.returncode,flush=True)
assert sha(e)==proof['sha256']
(r/'executable-after.json').write_text(json.dumps(proof,indent=2)+'\n')
sys.exit(any(x['exit'] for x in results))
