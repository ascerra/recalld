#!/usr/bin/env python3
"""Actual public contexts, separate packed scores and explicit per-run identity evidence."""
import argparse
import copy
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import sys

h=runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')));c=h['CONTEXT'];t=c['TOKEN']


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for flag in ('binary','scorer','source-root','build','assets','out'):p.add_argument('--'+flag,type=Path,required=True)
    a=p.parse_args();a.out.mkdir();receipts=[];runner=[sys.executable,str(Path(__file__).with_name('public-mcp-evaluation.py'))]
    def command(name,argv,success=True,absent=None):
        with (a.out/(name+'.log')).open('wb') as log:code=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT).returncode
        receipts.append(dict(name=name,command=list(map(str,argv)),exitCode=code,expectedSuccess=success,outputAbsent=None if absent is None else not absent.exists()));h['save'](a.out/'receipts.json',receipts)
        assert (code==0)==success,name
        if absent is not None:assert not absent.exists(),name
    def prepare(name,profile='default',old=False):
        dest=a.out/(name+'-inputs')
        command('prepare-'+name,[*runner,'prepare','--track',h['TRACK'] if old else c['TRACK'],'--context-profile',profile,'--context-assets',a.assets,'--binary',a.binary,'--scorer',a.scorer,'--build',a.build,'--out',dest]);return dest
    def run(name,inputs,success=True,assets=None):
        dest=a.out/name
        command('run-'+name,[*runner,'run','--context-assets',assets or a.assets,'--manifest',inputs/'manifest.json','--binary',a.binary,'--scorer',a.scorer,'--source-root',a.source_root,'--out',dest],success,None if success else dest);return dest
    def variant(source,name):
        dest=a.out/name;shutil.copytree(source,dest);return dest,json.loads((dest/'manifest.json').read_text())
    inputs={name:prepare(name,name) for name in ('default','boundary','oversized-first','empty')}
    altered,m=variant(inputs['boundary'],'changed-label-inputs');labels=json.loads((altered/'evaluator-labels.json').read_text());labels.update(labelCanary='CHANGED_CONTEXT_LABEL_CANARY',relevant={'q0':['turn-3']});h['save'](altered/'evaluator-labels.json',labels);m['evaluatorLabels']['sha256']=h['digest']((altered/'evaluator-labels.json').read_bytes());h['save'](altered/'manifest.json',m)
    inputs['changed-label']=altered
    # Freeze all fixtures/budgets/label canaries before any actor outcome.
    frozen={name:{str(path.relative_to(root)):h['digest'](path.read_bytes()) for path in root.glob('*.json')} for name,root in inputs.items()};h['save'](a.out/'frozen-before-outcomes.json',frozen)
    outputs={name:run(name,root) for name,root in inputs.items()}
    assert frozen=={name:{str(path.relative_to(root)):h['digest'](path.read_bytes()) for path in root.glob('*.json')} for name,root in inputs.items()}
    summaries={};encoder=t['load'](a.assets)
    for name,out in outputs.items():
        result=json.loads((out/'result.json').read_text());validation=json.loads((out/'evaluator/context-validation.json').read_text());original=json.loads((out/'evaluator/scores.json').read_text());packed=json.loads((out/'evaluator/packed-context-scores.json').read_text())
        assert result['status']=='passed' and result['futureQueryCanaryWithheldUntilDelivery']
        assert validation['publicPayloadVerified'] and set(validation['queries'])=={'q0','q1','q2','q3'}
        manifests=json.loads((inputs[name]/'manifest.json').read_text());source=json.loads((out/'source-mappings.json').read_text())['mappings'];evidence=[]
        for q,receipt in validation['queries'].items():
            assert receipt['fullTokenCount']==len(t['tokens'](encoder,receipt['renderedText']))
            assert receipt['fullTokenCount']<=manifests['contextPolicy']['renderedContextTokenCap'] and receipt['evidenceTokenCount']<=manifests['contextPolicy']['evidenceTokenCap']
            included=[source[v][0] for v in receipt['includedMemoryIds']];omitted=[source[v][0] for v in receipt['omittedMemoryIds']]
            assert 'turn-3' not in included+omitted
            if name=='default':assert included==['turn-0','turn-1','turn-2'] and omitted==[]
            elif name=='boundary':assert included==['turn-0'] and omitted==['turn-1','turn-2']
            elif name=='changed-label':assert included+omitted==['turn-0','turn-1','turn-2']
            elif name=='oversized-first':assert included==[] and omitted==['turn-0','turn-1','turn-2']
            else:assert included==[] and omitted==[]
            evidence.append(dict(queryId=q,includedSources=included,omittedSources=omitted,fullTokens=receipt['fullTokenCount'],evidenceTokens=receipt['evidenceTokenCount'],renderedSha256=receipt['renderedSha256']))
        if name in ('default','boundary','oversized-first'):
            assert original['scoring']['scored']=='2' and packed['scoring']['scored']=='2'
            assert original['scoring']['means']['retrievalRecallAt5']==.75
            assert packed['scoring']['means']['retrievalRecallAt5']=={'default':.75,'boundary':.25,'oversized-first':0.}[name]
        if name=='empty':assert original['scoring']['scored']=='0' and packed['scoring']['means']['retrievalRecallAt5'] is None
        summaries[name]=dict(actions=result['actorActions'],queries=evidence,retrievalScoring=original['scoring'],packedScoring=packed['scoring'])
        h['save'](a.out/'context-summary.json',summaries)
    left,right=[outputs[n] for n in ('boundary','changed-label')]
    attempts=lambda root:[r for r in h['rows'](root/'actor-wire.jsonl') if r['event']=='attempt']
    providers=lambda root:[{k:v for k,v in r.items() if k not in ('environmentHash','cacheKeys')} for r in h['rows'](root/'actor-provider.jsonl')]
    canary=dict(actorCodeEqual=(left/'actor-code.py').read_bytes()==(right/'actor-code.py').read_bytes(),actorInputsEqual=all(path.read_bytes()==(right/'actor-inputs'/path.name).read_bytes() for path in (left/'actor-inputs').glob('*.json')),requestsEqual=attempts(left)==attempts(right),providerInputsVectorsEqual=providers(left)==providers(right),evidenceIdentityOnly=True,rawRenderOrTokenEqualityClaim=False)
    assert all(canary[k] for k in ('actorCodeEqual','actorInputsEqual','requestsEqual','providerInputsVectorsEqual'))
    # Do not erase different UUID lengths or selected-prefix differences.
    canary['perRunComparison']=[dict(queryId=x['queryId'],left=x,right=y,selectedPrefixDiverged=x['includedSources']!=y['includedSources']) for x,y in zip(summaries['boundary']['queries'],summaries['changed-label']['queries'])]
    h['save'](a.out/'label-canary.json',canary)
    wire=h['rows'](left/'actor-wire.jsonl');manifest=json.loads((inputs['boundary']/'manifest.json').read_text());schedule=json.loads((inputs['boundary']/'evaluator-schedule.json').read_text());score_input=json.loads((left/'evaluator/score-input.json').read_text())
    # Actual public payload with a self-consistent fabricated actor receipt must fail.
    altered_wire=copy.deepcopy(wire);outcome=next(r for r in wire if r['event']=='outcome' and r['tag']=='query:q0');memories=json.loads(outcome['response']['result']['content'][0]['text'])['memories'];fake=copy.deepcopy(memories);fake[0]['summary']='fabricated context evidence'
    row=next(r for r in altered_wire if r['event']=='context' and r['tag']=='context:q0');row['context']=t['assemble'](encoder,schedule['queries'][0]['text'],fake,manifest['contextPolicy'])
    args=argparse.Namespace(**vars(a));args.context_assets=a.assets;args.out=a.out/'forged-consumer'
    try:c['consume'](args,h,manifest,schedule,altered_wire,score_input)
    except ValueError as error:
        assert 'actual public payload' in str(error);h['save'](a.out/'forged-public-payload-rejection.json',dict(error=str(error),outputAbsent=not args.out.exists()))
    else:raise AssertionError('forged actor payload accepted')
    assert not args.out.exists()
    for name in ('version','cap','encoding','rank','config','old-track'):
        dest,m=variant(inputs['boundary'],'negative-'+name)
        if name=='version':m['contextPolicy']['contextVersion']=True
        elif name=='cap':m['contextPolicy']['evidenceTokenCap']=129
        elif name=='encoding':m['contextPolicy']['tokenizer']['encoding']='model-inferred'
        elif name=='rank':m['contextPolicy']['tokenizer']['rankSha256']='0'*64
        elif name=='config':m['contextPolicy']['tokenizer']['configSha256']='0'*64
        else:m['track']=h['TRACK']
        h['save'](dest/'manifest.json',m);run('reject-'+name,dest,False)
    bad=a.out/'corrupt-assets';shutil.copytree(a.assets,bad);(bad/'context-encoding.json').write_text('{}');run('reject-asset',inputs['boundary'],False,bad)
    old_inputs=prepare('old',old=True);old=run('old',old_inputs);assert json.loads((old/'result.json').read_text())['actorActions']==15
    h['save'](a.out/'validation.json',dict(status='passed',actualEpisodes=6,actualActions=sum(v['actions'] for v in summaries.values())+15,contextQueries=20,
        manifestAssetNegatives=7,actualForgedPayloadRejected=True,canary=canary,contextSummaries=summaries,modelTokenIdentity=None,answerQuality=None,cost=None))


if __name__=='__main__':main()
