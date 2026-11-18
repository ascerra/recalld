import hashlib,json,pathlib,subprocess,sys,time,os
root=pathlib.Path('/evidence')
exes=[p for p in pathlib.Path('/target/debug/deps').glob('recalld-*') if p.is_file() and os.access(p,os.X_OK) and not p.suffix]
assert len(exes)==1,[str(p) for p in exes]
exe=exes[0]
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for c in iter(lambda:f.read(1048576),b''):h.update(c)
 return h.hexdigest()
proof={'path':str(exe),'sha256':digest(exe),'bytes':exe.stat().st_size}
(root/'executable-before.json').write_text(json.dumps(proof,indent=2)+'\n')
filters=['live_record_schema_', 'live_metadata_schemas_actual_outputs_preserve_eras_scope_and_truncation',
'ordinary_point_native_receipt_and_owning_timezone_survive_actual_rpc',
'ordinary_point_native_unknown_mcp_rpc_keeps_operation_and_pending_root',
'ordinary_point_failed_rpc_decoder_preserves_real_raw_receipt_through_final_frame',
'ordinary_point_native_publication_failure_mcp_rpc_keeps_full_typed_receipt',
'ordinary_point_legacy_ghost_direct_rpc_preserve_payload_and_one_event',
'ordinary_point_legacy_final_admission_keeps_exact_receipt_and_old_diagnostic',
'native_api1_store_actual_mcp_unicode_and_ignored_single_vector',
'native_api1_store_publication_failure_keeps_full_receipt_and_projection_evidence',
'native_api1_store_legacy_shapes_and_explicit_profile_constraint',
'api1_batch_public_mcp_native_unicode',
'api1_batch_final_barrier_failure_keeps_receipts_and_repair_replays',
'api1_batch_mixed_and_all_invalid_keep_every_index_and_no_false_rollback',
'api1_batch_malformed_remote_evidence_never_claims_commit_or_counts',
'api1_batch_remote_request_binding_keeps_only_matching_reservation_committed',
'api1_batch_legacy_positive_shapes_and_explicit_profile_rejection',
'w71_a_short_delete_memories_response_is_an_error_per_id']
results=[]
for name in filters:
 start=time.monotonic()
 with (root/(name+'.log')).open('wb') as log:
  p=subprocess.run([str(exe),name,'--nocapture','--test-threads=2'],stdout=log,stderr=subprocess.STDOUT)
 results.append({'filter':name,'exit':p.returncode,'seconds':time.monotonic()-start})
 (root/'focused-results.json').write_text(json.dumps(results,indent=2)+'\n')
 print(name,p.returncode,flush=True)
assert digest(exe)==proof['sha256']
(root/'executable-after.json').write_text(json.dumps(proof,indent=2)+'\n')
sys.exit(any(r['exit'] for r in results))
