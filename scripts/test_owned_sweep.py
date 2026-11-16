#!/usr/bin/env python3
"""Named legacy chronological sweep contract fixture, not retrieval quality/reset.
Run only in isolated network-none Docker; actual public MCP + private controls.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading
from test_owned_evaluator import JsonLines, files, save

DAY = 86_400_000


def episode(binary, output, endpoint, number):
    root=output/f'chronological-{number}';root.mkdir();home=root/'home';home.mkdir()
    config=root/'config.toml'
    config.write_text(f'''[embedding]
provider="openai"
model_name="offline-sweep-v1"
base_url="{endpoint}"
api_key_env="RECALLD_EVAL_OFFLINE_KEY"
dimensions=4
document_prefix=""
query_prefix=""
[decay]
disable_sweep=true
decay_rate_multiplier=1.0
[rif]
enabled=false
[graph]
autolink_enabled=false
''')
    rq,wq=os.pipe();rr,wr=os.pipe();log=[]
    command=[str(binary),'--config',str(config),'--data-root',str(root/'data'),
             '--control-read-fd',str(rq),'--control-write-fd',str(wr)]
    with (root/'child.log').open('wb') as stderr:
        child=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,
            bufsize=0,pass_fds=(rq,wr),env={'PATH':'/usr/local/bin:/usr/bin:/bin','HOME':str(home),
            'TMPDIR':str(home),'RECALLD_EVAL_OFFLINE_KEY':'offline-fixture'})
        os.close(rq);os.close(wr)
        actor=JsonLines(child.stdout.fileno(),child.stdin.fileno(),log);control=JsonLines(rr,wq,log)
        try:
            def rpc(method,params):
                value=actor.call(method,params,public=True);assert 'error' not in value,value;return value['result']
            def tool(name,args,ok=True):
                value=rpc('tools/call',{'name':name,'arguments':args});assert (value.get('isError') is not True)==ok,value
                return json.loads(value['content'][0]['text']) if ok else value
            def ctl(method,args):
                value=control.call(method,args);assert 'error' not in value,value;return value['result']
            rpc('initialize',{'protocolVersion':'2025-06-18','capabilities':{},'clientInfo':{'name':'legacy-sweep-fixture','version':'1'}})
            os.write(child.stdin.fileno(),b'{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}\n')
            names={x['name'] for x in rpc('tools/list',{})['tools']};assert not names.intersection({'sweep','sweepStatus','advance','snapshot'})
            caps=ctl('capabilities',{});assert caps['controlVersion']==caps['sweepControlVersion']==1
            assert caps['publicContract']=='legacy-v1' and caps['guarantees']['explicitLegacySweep'] is True
            assert caps['guarantees']['nativeSweep'] is False
            assert ctl('sweepStatus',{})=={'episodeInvalidated':False,'lastSweep':None}
            memory=tool('store_memory',{'summary':'chronological-sweep','fullText':'one retained payload','namespace':'default'})
            original=tool('get_memory',{'id':memory['id'],'namespace':'default','recordAccess':False})
            assert original['phase']=='Full' and original['fullText']=='one retained payload'
            before=ctl('inspect',{});before_files=files(root/'data')
            invalid=[{}, {'deadlineMs':0},{'deadlineMs':30001},{'deadlineMs':True},
                     {'deadlineMs':1.5},{'deadlineMs':100,'extra':True}]
            for params in invalid:
                assert control.call('sweep',params)['error']['code']=='INVALID_PARAMS'
            assert control.call('sweepStatus',{'extra':True})['error']['code']=='INVALID_PARAMS'
            assert ctl('inspect',{})==before and files(root/'data')==before_files
            now=ctl('advance',{'deltaMs':36_500*DAY,'deadlineMs':30000})['nowMillis']
            reports=[]
            for index,(phase,count) in enumerate([('Summary','fullToSummary'),('Ghost','summaryToGhost'),(None,'deletions')]):
                result=ctl('sweep',{'deadlineMs':30000});reports.append(result)
                assert result['sampledAtMillis']==now and result['phase']=='completed' and result['executionCompleted'] is True,result
                assert result['result'][count]==1 and result['readiness']['ready'] is True,result
                assert result['readiness']['pendingWork']==0 and len(result['readiness']['derived'])==4
                assert all(v['ready'] and v['appliedGeneration']==result['readiness']['committedGeneration'] for v in result['readiness']['derived'])
                assert ctl('sweepStatus',{})=={'episodeInvalidated':False,'lastSweep':result}
                frozen=ctl('inspect',{});raw=files(root/'data')
                if phase:
                    fetched=tool('get_memory',{'id':memory['id'],'recordAccess':False,'namespace':'default'})
                    assert fetched['phase']==phase and fetched.get('fullText') is None,fetched
                    assert fetched['summary']==('chronological-sweep' if phase=='Summary' else ''),fetched
                else:
                    tool('get_memory',{'id':memory['id'],'recordAccess':False,'namespace':'default'},ok=False)
                assert ctl('inspect',{})==frozen and files(root/'data')==raw,'false fetch mutated sweep state'
            ctl('advance',{'deltaMs':1,'deadlineMs':30000})
            os.close(wq);wq=None;child.wait(timeout=5);assert child.returncode==0
            return {'passed':True,'fixture':'legacy-chronological-sweep-v1','replay':number,
                    'command':command,'sampledAtMillis':now,'storedId':memory['id'],'reports':reports,
                    'invalidRequestsWithoutMutation':len(invalid)+1,'falseFetchesUnchanged':3,
                    'claim':'actual one-phase sweep/control diagnostic; no native/whole-state reset/rank or full-A6 claim'}
        finally:
            save(root/'transcript.json',log)
            if child.poll() is None:child.kill();child.wait(timeout=5)
            if wq is not None:os.close(wq)
            os.close(rr);child.stdin.close();child.stdout.close()
            save(root/'process.json',{'command':command,'exitCode':child.returncode})


def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    args.output.mkdir(parents=True);requests=[]
    class Embeddings(http.server.BaseHTTPRequestHandler):
        def log_message(self,*args):pass
        def do_POST(self):
            body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(body)
            inputs=body.get('input');inputs=[inputs] if isinstance(inputs,str) else inputs
            ok=body.get('model')=='offline-sweep-v1' and inputs==['chronological-sweep\n\none retained payload']
            self.send_response(200 if ok else 404);self.send_header('Content-Type','application/json');self.end_headers()
            self.wfile.write(json.dumps({'data':[{'index':0,'embedding':[1.,0.,0.,0.]}],'usage':{'prompt_tokens':0,'total_tokens':0}} if ok else {'error':'offline replay miss'}).encode())
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Embeddings);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    try:
        runs=[episode(args.binary.resolve(),args.output,f'http://127.0.0.1:{server.server_port}/v1',i) for i in range(1,4)]
        assert len(requests)==3,requests
        # Compare complete reported values, not a normalized hash/receipt projection.
        assert runs[0]['reports']==runs[1]['reports']==runs[2]['reports'],'sweep report drift'
        save(args.output/'summary.json',{'binarySha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'episodes':runs,'providerCalls':len(requests)})
    finally:
        save(args.output/'embedding-requests.json',requests);server.shutdown();server.server_close()
if __name__=='__main__':main()
