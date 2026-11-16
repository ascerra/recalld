#!/usr/bin/env python3
"""Stream actual events/syscalls; retain individual observations, no percentiles."""
import argparse, collections, datetime, decimal, json, pathlib, re
D=decimal.Decimal
LINE=re.compile(r'^(\d+)\s+(\d+\.\d+)\s+(.*)$')
CALL=re.compile(r'^(fsync|fdatasync|msync)\((.*)\)\s+=\s+(.*?)\s+<([\d.]+)>$')
def syscalls(path):
    pending={}
    with path.open() as f:
        for line_no,line in enumerate(f,1):
            m=LINE.match(line.strip())
            if not m: raise ValueError(f'unparsed strace line {line_no}: {line[:120]}')
            tid,t,body=m.groups()
            if body.startswith(('+++','---')): continue
            if body.endswith('<unfinished ...>'):
                assert tid not in pending
                pending[tid]=(t,body[:-len('<unfinished ...>')]);continue
            if body.startswith('<...'):
                t,first=pending.pop(tid)
                body=first+body.split('resumed>',1)[1]
            m=CALL.match(body)
            if not m: raise ValueError(f'unparsed syscall line {line_no}: {body[:120]}')
            call,args,result,elapsed=m.groups()
            start=int(D(t)*10**9);ns=int(D(elapsed)*10**9)
            if 'meta.db' in args: file='metadata'
            elif 'edges.db' in args: file='edges'
            elif 'vectors' in args: file='vectors'
            elif 'fulltext' in args: file='text'
            elif 'fts' in args: file='fts'
            else: file='unresolved_or_other'
            yield {'tid':int(tid),'call':call,'file':file,'start_ns':start,'end_ns':start+ns,'elapsed_ns':ns,'result':result}
    assert not pending, f'unfinished syscalls: {pending.keys()}'
def cell(root):
    setup=json.loads((root/'setup.json').read_text());complete=json.loads((root/'complete.json').read_text())
    writes=[json.loads(l) for l in (root/'writes.jsonl').open()]
    assert len(writes)==4 and complete['writes']==4 and complete['errors']==complete['sink_errors']==0
    assert [w['warmup'] for w in writes]==[True,False,False,False]
    for w in writes:
        assert w['error'] is None and w['response']['id']
        assert w['provider_calls']==int(setup['path']=='mcp-local-provider')
    roots={};events=0
    forbidden={'summary','full_text','fullText','tags','entities','topics','emotions','embedding','prompt','text','namespace'}
    for line in (root/'events.jsonl').open():
        assert 'onlineproof' not in line and 'synthetic online payload' not in line
        e=json.loads(line);events+=1;f=e['fields'];assert not forbidden.intersection(f),f
        spans=e.get('spans',[])+([e['span']] if 'span' in e else [])
        stores={s['trace_id'] for s in spans if s.get('operation')=='mcp.store'}
        if not stores: continue # post-write read-only verification has separate roots
        assert len(stores)==1
        tid=stores.pop();r=roots.setdefault(tid,{'durations':collections.defaultdict(list),'exclusive':{},'counters':collections.defaultdict(list),'receipts':[],'derived_ids':set()})
        for s in spans:
            if s.get('operation')=='derived.complete': r['derived_ids'].add(s['trace_id'])
        kind=f.get('kind')
        if kind=='duration':
            assert f['outcome'] in ('ok','released'), f
            r['durations'][f['stage']].append(f['elapsed_ns'])
        elif kind=='exclusive_pipeline':
            assert f['stage'] not in r['exclusive'];r['exclusive'][f['stage']]=f['elapsed_ns']
        elif kind=='pipeline_accounting':
            assert f['elapsed_ns']==f['accounted_ns'];r['accounted_ns']=f['accounted_ns']
        elif kind=='committed_receipt':r['receipts'].append({k:f[k] for k in ('store_id','operation_id','committed_generation')})
        else:r['counters'][kind].append(f)
    assert len(roots)==4, roots.keys()
    measured=[];all_rows=[]
    for w,(tid,r) in zip(writes,sorted(roots.items())):
        assert len(r['durations']['mcp.store'])==1 and len(r['derived_ids'])==2
        assert len(r['durations']['derived.complete'])==2
        assert sum(r['exclusive'].values())==r['accounted_ns']
        assert len(r['counters']['derived_rebuild'])==len(r['counters']['derived_noop'])==1
        rebuild=r['counters']['derived_rebuild'][0]
        assert rebuild['records']==rebuild['live_records']==setup['records']+w['index']+1
        assert rebuild['edges']==setup['before']['edges'] and rebuild['pending_effects']==1
        assert r['counters']['insert_edges'][0]['loaded']==setup['before']['edges']
        assert r['counters']['store_embedding_input'][0]['provider_calls']==w['provider_calls']
        assert len(r['durations'].get('store.provider',[]))==w['provider_calls']
        assert len(r['receipts'])==1
        receipt=r['receipts'][0]
        assert int(receipt['committed_generation'])==int(setup['before']['control']['committedGeneration'])+w['index']+1
        for stage in ['insert.worker_queue','derived.worker_queue','insert.storage.write.wait','insert.storage.write.hold','derived.storage.write.wait','derived.storage.write.hold','derived.canonical_records','derived.record_views','derived.canonical_edges','derived.graph_edges_publish','derived.effects_ack','insert.canonical_edges','insert.canonical_commit','insert.vector_sync','insert.text_sync','insert.apply_effects']:
            assert r['durations'][stage], stage
        row={'index':w['index'],'warmup':w['warmup'],'ack_ns':w['elapsed_ns'],'with_sink_flush_ns':w['with_sink_flush_ns'],'provider_calls':w['provider_calls'],'trace_id':tid,'receipt':receipt,'durations_ns':dict(r['durations']),'exclusive_ns':r['exclusive'],'rebuild':rebuild,'syscalls':[]}
        all_rows.append(row)
        if not w['warmup']:measured.append(row)
    outside=0;boundary=[];total=0;errors=[]
    for call in syscalls(root/'syscalls.log'):
        total+=1
        if not call['result'].startswith('0'): errors.append(call)
        owners=[i for i,w in enumerate(writes) if int(w['begin_ns'])<=call['start_ns'] and call['end_ns']<=int(w['ack_ns'])]
        if len(owners)==1:all_rows[owners[0]]['syscalls'].append(call)
        else:
            outside+=1
            if any(call['start_ns']<=int(w['ack_ns']) and call['end_ns']>=int(w['begin_ns']) for w in writes):boundary.append(call)
    assert not errors, errors
    for row in all_rows:
        assert row['syscalls'], 'no actual sync evidence'
        row['syscall_counts']=dict(collections.Counter(c['call']+':'+c['file'] for c in row['syscalls']))
        row['syscall_elapsed_ns']=sum(c['elapsed_ns'] for c in row['syscalls'])
        row['syscall_tids']=sorted({c['tid'] for c in row['syscalls']})
    rss=[l.strip() for l in (root/'process-status.txt').read_text().splitlines() if l.startswith(('VmHWM:','VmRSS:'))]
    return {'cell':root.name,'passed':True,'setup':setup,'events':events,'all_writes':all_rows,'measured_ack_range_ns':[min(w['ack_ns'] for w in measured),max(w['ack_ns'] for w in measured)],'syscalls_total':total,'syscalls_outside_windows':outside,'boundary_syscalls':boundary,'process_rss_including_setup':rss}
def selftest():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p=pathlib.Path(d)/'trace'
        p.write_text('11 100.000001 fsync(9</study/meta.db> <unfinished ...>\n12 100.000002 fdatasync(7</study/fulltext.dat>) = 0 <0.000003>\n11 100.000004 <... fsync resumed>) = 0 <0.000004>\n11 100.000009 +++ exited with 0 +++\n')
        rows=list(syscalls(p));assert len(rows)==2 and rows[1]['start_ns']==100000001000 and rows[1]['elapsed_ns']==4000 and rows[0]['file']=='text'
        p.write_text('11 100.0 fsync(9) <unfinished ...>\n')
        try:list(syscalls(p))
        except AssertionError:pass
        else:raise AssertionError('truncated syscall accepted')
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('root',type=pathlib.Path,nargs='?');p.add_argument('--self-test',action='store_true');a=p.parse_args()
    if a.self_test:selftest();print('syscall attribution self-test passed')
    else:
        status=json.loads((a.root/'status.json').read_text());assert status['all_six_complete']
        results=[cell(a.root/c['cell']) for c in status['cells']]
        (a.root/'qa.json').write_text(json.dumps({'passed':True,'cells':results},indent=2)+'\n')
        print('QA passed:',len(results),'cells,',sum(len(c['all_writes']) for c in results),'writes')
