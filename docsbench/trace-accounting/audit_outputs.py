#!/usr/bin/env python3
"""Read-only audit of recorded public source/rank references and exact byte counters."""
import argparse
import hashlib
import json
from pathlib import Path


def read(p):return json.loads(p.read_text())
def rows(p):return [json.loads(line) for line in p.read_text().splitlines()]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()


def audit(actual):
    results=[]
    for name in ('baseline','changed-labels'):
        root=actual/name;trace=rows(root/'trace.jsonl');wire=rows(root/'actor-wire.jsonl');provider=rows(root/'actor-provider.jsonl')
        summary=read(root/'evaluator/accounting.json');mapping=read(root/'source-mappings.json');sources={s['sourceId'] for s in mapping['sources']}
        assert sources=={f'turn-{i}' for i in range(4)}
        stores=[r for r in trace if r['eventKind']=='outcome' and r['sourceId'] is not None]
        assert {r['sourceId'] for r in stores}==sources
        for row in stores:
            memory=json.loads(row['outcome']['result']['content'][0]['text'])['id'];assert mapping['mappings'][memory]==[row['sourceId']]
        queries=[r for r in trace if r['publicRankObservations'] is not None];assert len(queries)==4
        for row in queries:
            records=json.loads(row['outcome']['result']['content'][0]['text'])['memories']
            assert row['publicRankObservations']==[dict(publicResponseRank=str(i+1),memoryId=r['id'],publicScore=r.get('score')) for i,r in enumerate(records)]
            assert all(mapping['mappings'][r['memoryId']]==['turn-0'] for r in row['publicRankObservations'])
        for row in trace:
            assert row['providerEvidence']['scope']=='delivery-window-includes-shared-startup-not-per-rpc'
            assert row['providerEvidence']['file']=='actor-provider.jsonl'
            assert row['providerEvidence']['deliveryOrdinal']==row['deliveryOrdinal']
            assert row['readGeneration'] is None and row['readiness']==dict(available=False)
        assert len(trace)==30 and len([r for r in wire if r['event']=='attempt'])==15
        request=sum(len(bytes.fromhex(r['accounting']['requestWireHex'])) for r in trace if r['eventKind']=='attempt')
        response=sum(len(bytes.fromhex(r['accounting']['responseWireHex'])) for r in trace if r['eventKind']=='outcome')
        assert summary['requestBytesWrittenKnown']==str(request)==summary['requestBytesIntended']
        assert summary['responseBytesReadKnown']==str(response)
        assert summary['providerRequestBodyBytes']==str(sum(len(bytes.fromhex(r['accounting']['requestBodyHex'])) for r in provider))
        assert summary['providerResponseBodyBytesWrittenKnown']==str(sum(len(bytes.fromhex(r['accounting']['responseBodyHex'])) for r in provider))
        assert summary['providerRequestsObserved']=='9' and summary['providerStatuses']=={'200':'8','400':'1'}
        assert all(summary[k] is None for k in ('measuredTokens','tokenizer','answerContextCapTokens','productionCost','httpFramingBytes','providerInternalRetries','backendDurability'))
        result=dict(run=name,sourceMappingsVerified=4,publicRankRowsVerified=4,attempts=15,outcomes=15,
            requestBytes=request,responseBytes=response,providerRequests=9,providerRequestBodyBytes=int(summary['providerRequestBodyBytes']),
            providerResponseBodyBytes=int(summary['providerResponseBodyBytesWrittenKnown']),categories=summary['categories'],
            providerAttribution='delivery-window-only',clientDurationSummed=False,unknownFieldsPreserved=True)
        results.append(result)
    return dict(status='passed',runs=results,auditorSha256=sha(Path(__file__)),
        inputFilesSha256={str(p.relative_to(actual)):sha(p) for p in sorted(actual.rglob('*')) if p.is_file()})


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('actual',type=Path);p.add_argument('out',type=Path);a=p.parse_args();v=audit(a.actual)
    with a.out.open('x') as f:json.dump(v,f,indent=2,sort_keys=True);f.write('\n')
    print(json.dumps({k:x for k,x in v.items() if k!='inputFilesSha256'},indent=2))
