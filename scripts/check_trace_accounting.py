#!/usr/bin/env python3
"""Validate consumed accounting on actual serialized production MCP episodes."""
import argparse
import copy
import json
from pathlib import Path
import runpy
import subprocess
import sys

h=runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for flag in ('binary','scorer','source-root','build','out'):p.add_argument('--'+flag,type=Path,required=True)
    a=p.parse_args();a.out.mkdir();receipts=[]
    runner=[sys.executable,str(Path(__file__).with_name('public-mcp-evaluation.py'))]
    def command(name,argv,success=True,absent=None):
        with (a.out/(name+'.log')).open('wb') as log:code=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT).returncode
        receipts.append(dict(name=name,command=list(map(str,argv)),exitCode=code,expectedSuccess=success,outputAbsent=None if absent is None else not absent.exists()))
        h['save'](a.out/'receipts.json',receipts);assert (code==0)==success,name
        if absent is not None:assert not absent.exists(),name
    def prepare(name,track):
        dest=a.out/name
        command(name,[*runner,'prepare','--track',track,'--binary',a.binary,'--scorer',a.scorer,'--build',a.build,'--out',dest]);return dest
    def run(name,inputs,success=True):
        dest=a.out/name
        command(name,[*runner,'run','--binary',a.binary,'--scorer',a.scorer,'--source-root',a.source_root,'--manifest',inputs/'manifest.json','--out',dest],success,None if success else dest)
        return dest
    inputs=prepare('inputs',h['ACCOUNTING_TRACK']);baseline=run('baseline',inputs)
    def variant(name):
        dest=a.out/name;dest.mkdir()
        for path in inputs.glob('*.json'):(dest/path.name).write_bytes(path.read_bytes())
        return dest,json.loads((dest/'manifest.json').read_text())
    changed,m=variant('changed-inputs');labels=json.loads((changed/'evaluator-labels.json').read_text())
    labels.update(labelCanary='CHANGED_LABEL_CANARY',relevant={f'q{i}':['turn-3'] for i in range(4)})
    h['save'](changed/'evaluator-labels.json',labels);m['evaluatorLabels']['sha256']=h['digest']((changed/'evaluator-labels.json').read_bytes());h['save'](changed/'manifest.json',m)
    other=run('changed-labels',changed)
    audits=[]
    for root in (baseline,other):
        trace=h['rows'](root/'trace.jsonl');wire=h['rows'](root/'actor-wire.jsonl');provider=h['rows'](root/'actor-provider.jsonl')
        summary=json.loads((root/'evaluator/accounting.json').read_text());assert summary==h['reconcile_accounting'](trace,provider)
        attempts=[r for r in wire if r['event']=='attempt'];outcomes=[r for r in wire if r['event']=='outcome']
        assert len(attempts)==len(outcomes)==15 and len(trace)==30
        assert all(trace[i]['eventKind']=='attempt' and trace[i+1]['eventKind']=='outcome' and trace[i]['stepId']==trace[i+1]['stepId'] for i in range(0,30,2))
        assert summary['attemptedActions']==summary['terminalRpcObservations']=='15'
        assert summary['failedRpcObservations']==summary['incompleteActions']==summary['actorProcessErrors']=='0'
        assert summary['categories']==dict(notification='1',protocol_error='1',success='12',tool_error='1')
        assert summary['requestWriteUnknownActions']==summary['providerResponseWriteUnknown']=='0'
        assert summary['requestBytesWrittenKnown']==str(sum(len(bytes.fromhex(r['accounting']['requestWireHex'])) for r in outcomes))
        assert summary['responseBytesReadKnown']==str(sum(len(bytes.fromhex(r['accounting']['responseWireHex'])) for r in outcomes))
        assert summary['providerRequestsObserved']==str(len(provider)) and summary['providerStatuses']['400']=='1'
        assert int(summary['providerRequestBodyBytes'])>0 and int(summary['providerResponseBodyBytesWrittenKnown'])>0
        for row in provider:
            fact=row['accounting'];assert fact['responseBodyBytesWritten']==fact['responseBodyBytesIntended']
            payload=json.loads(bytes.fromhex(fact['responseBodyHex']));assert fact['returnedVectors']==str(len(payload.get('data',[])))
        assert all(summary[k] is None for k in ('measuredTokens','tokenizer','answerContextCapTokens','productionCost','httpFramingBytes','providerInternalRetries','backendDurability'))
        assert json.loads((root/'result.json').read_text())['futureQueryCanaryWithheldUntilDelivery']
        audits.append(dict(run=root.name,status='passed',accounting=summary))
        h['save'](a.out/'semantic-audit.json',audits)
    attempts=lambda root:[r['request'] for r in h['rows'](root/'actor-wire.jsonl') if r['event']=='attempt']
    provider_projection=lambda root:[{k:r[k] for k in ('requestBody','status','deliveryOrdinal','responseVectors')} for r in h['rows'](root/'actor-provider.jsonl')]
    checks=dict(actorRequestsEqual=attempts(baseline)==attempts(other),providerRequestsVectorsEqual=provider_projection(baseline)==provider_projection(other),
        actorInputsEqual=all(path.read_bytes()==(other/'actor-inputs'/path.name).read_bytes() for path in (baseline/'actor-inputs').glob('*.json')),
        actorCodeEqual=(baseline/'actor-code.py').read_bytes()==(other/'actor-code.py').read_bytes(),
        environmentChanged=json.loads((baseline/'result.json').read_text())['environmentHash']!=json.loads((other/'result.json').read_text())['environmentHash'],
        cacheKeysChanged=all(l['cacheKeys']!=r['cacheKeys'] for l,r in zip(h['rows'](baseline/'actor-provider.jsonl'),h['rows'](other/'actor-provider.jsonl'))))
    h['save'](a.out/'label-comparison.json',checks);assert all(checks.values())
    negatives=[('version',lambda m:m['accountingPolicy'].update(accountingVersion=2)),('bool-version',lambda m:m['accountingPolicy'].update(accountingVersion=True)),
        ('tokenizer',lambda m:m['accountingPolicy'].update(tokenizer='invented')),('context',lambda m:m['accountingPolicy'].update(answerContextCapTokens=4096)),
        ('cost',lambda m:m['accountingPolicy'].update(productionPricing=0)),('sampled',lambda m:m['accountingPolicy'].update(sampled=True)),
        ('old-fields',lambda m:m.update(track=h['TRACK'])),('readiness',lambda m:m['capabilities'].update(coherentReadView=True))]
    for name,change in negatives:
        path,m=variant('negative-'+name);change(m);h['save'](path/'manifest.json',m);run('reject-'+name,path,False)
    old=run('old-profile',prepare('old-inputs',h['TRACK']))
    assert not (old/'evaluator/accounting.json').exists()
    assert len(h['rows'](old/'trace.jsonl'))==15 and all('accounting' not in r for r in h['rows'](old/'actor-wire.jsonl'))
    h['save'](a.out/'validation.json',dict(status='passed',actualEpisodes=3,actualActions=45,appendOnlyTraceEvents=60,manifestNegatives=8,
        labelComparison=checks,scope='Synthetic RPC/body observations only; injected interruption/write/parse faults are separate unit tests; no latency-performance/reset/token-budget/cost claim.'))


if __name__=='__main__':main()
