#!/usr/bin/env python3
"""Actual paired public episodes and seeded whole-group diagnostic receipts."""
import argparse
import copy
import json
from pathlib import Path
import runpy
import subprocess
import sys

h=runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))
p=h['PAIRED']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for flag in ('binary','scorer','source-root','build','out'):parser.add_argument('--'+flag,type=Path,required=True)
    a=parser.parse_args();a.out.mkdir();receipts=[]
    def command(name,argv,success=True,absent=None):
        with (a.out/(name+'.log')).open('wb') as log:code=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT).returncode
        receipts.append(dict(name=name,command=list(map(str,argv)),exitCode=code,expectedSuccess=success,outputAbsent=None if absent is None else not absent.exists()))
        h['save'](a.out/'receipts.json',receipts)
        assert (code==0)==success,name
        if absent is not None:assert not absent.exists(),name
    runner=[sys.executable,str(Path(__file__).with_name('public-mcp-evaluation.py'))]
    inputs=a.out/'inputs';out=a.out/'paired'
    command('prepare',[*runner,'prepare','--track',p['TRACK'],'--binary',a.binary,'--scorer',a.scorer,'--build',a.build,'--out',inputs])
    frozen={str(path.relative_to(inputs)):h['digest'](path.read_bytes()) for path in inputs.rglob('*.json')}
    h['save'](a.out/'frozen-before-outcomes.json',frozen)
    def run(name,source,dest,success=True):
        command(name,[*runner,'run','--manifest',source/'manifest.json','--binary',a.binary,'--scorer',a.scorer,'--source-root',a.source_root,'--out',dest],success,None if success else dest)
    run('actual-pair',inputs,out)
    assert frozen=={str(path.relative_to(inputs)):h['digest'](path.read_bytes()) for path in inputs.rglob('*.json')}
    status=json.loads((out/'pair-status.json').read_text());scores=json.loads((out/'paired-scores.json').read_text())
    assert status['status']=='passed' and (status['plannedMembers'],status['passedMembers'],status['failedMembers'],status['unattemptedMembers'])==(14,14,0,0)
    assert len(status['members'])==14 and all(v['status']=='passed' for v in status['members'])
    comparisons=[]
    for i in range(7):
        roots=[out/role/'episodes'/f'episode-{i}' for role in ('baseline','candidate')]
        attempts=[];providers=[]
        for limit,root in zip((1,2),roots):
            result=json.loads((root/'result.json').read_text());raw=json.loads((root/'evaluator/score-input.json').read_text())
            scoring=json.loads((root/'evaluator/scores.json').read_text())['scoring']
            assert result['status']=='passed' and result['actorActions']==15 and result['futureQueryCanaryWithheldUntilDelivery']
            assert (scoring['scored'],scoring['zeroLabelUnscored'],scoring['unknownAnnotations'])==('2','1','1')
            assert sorted(q['queryId'] for q in raw['queries'])==['q0','q1','q2','q3']
            for query in raw['queries']:
                assert [raw['mappings'][memory] for memory in query['memoryIds']]==[['turn-0'],['turn-1']][:limit]
            rows=copy.deepcopy([v for v in h['rows'](root/'actor-wire.jsonl') if v['event']=='attempt'])
            for row in rows:
                if row['request'].get('params',{}).get('name')=='recall_memories':
                    assert row['request']['params']['arguments']['limit']==limit
                    row['request']['params']['arguments'].pop('limit')
            attempts.append(rows)
            providers.append([{k:v for k,v in row.items() if k not in ('cacheKeys','environmentHash')} for row in h['rows'](root/'actor-provider.jsonl')])
        left,right=roots
        comparison=dict(episodeId=f'episode-{i}',actorCodeEqual=(left/'actor-code.py').read_bytes()==(right/'actor-code.py').read_bytes(),
            actorInputsEqual=all(path.read_bytes()==(right/'actor-inputs'/path.name).read_bytes() for path in (left/'actor-inputs').glob('*.json')),
            onlyDeclaredRequestLimitDiffers=attempts[0]==attempts[1],providerInputsVectorsEqual=providers[0]==providers[1])
        comparisons.append(comparison);h['save'](a.out/'arm-comparison.json',comparisons)
        assert all(v for k,v in comparison.items() if k!='episodeId'),comparison
    assert sum(int(v['baseline']['queryMicro']['queryEvents']) for v in scores['splits'].values())==28
    for split,n in (('train',3),('development',1),('test',1)):
        value=scores['splits'][split];paired=value['paired']
        assert len(value['eligibleGroupIds'])==n and not value['excludedGroups']
        assert paired['eligibleGroups']==str(n) and not paired['qualityPromotionEligible']
        assert paired['executedReplicates']==('10000' if n>1 else '0')
        if n==1:assert paired['interval95'] is None and paired['status']=='insufficient_eligible_groups'
    train=scores['splits']['train']['paired']
    assert train['point']['retrievalRecallAt5']['queryMicro']['effect']==0.5
    assert abs(train['point']['retrievalRecallAt5']['groupMacro']['effect']-11/24)<1e-15
    assert not scores['qualityPromotionEligible'] and not scores['semanticQualityClaim']

    # No extra episodes: exercise the real Rust scorer with complete recorded inputs.
    original=json.loads((out/'paired-score-input.json').read_text())
    def update(arm,key,value):
        text=h['encoded'](value).decode();arm[key+'Json']=text;arm[key+'Sha256']=h['digest'](text.encode())
    for name in ('missing-arm','duplicate-arm','failed-arm','missing-member','duplicate-member','missing-query','changed-label','permuted'):
        value=copy.deepcopy(original);arm=value['arms'][1]
        if name=='missing-arm':value['arms'].pop()
        elif name=='duplicate-arm':value['arms'][1]=value['arms'][0]
        elif name=='failed-arm':
            raw=json.loads(arm['statusJson']);raw.update(status='failed',failed=1,succeeded=6);update(arm,'status',raw)
        else:
            raw=json.loads(arm['groupedInputJson'])
            if name=='missing-member':raw['episodes'].pop()
            elif name=='duplicate-member':raw['episodes'].append(raw['episodes'][0])
            elif name in ('missing-query','changed-label'):
                row=raw['episodes'][0];episode=json.loads(row['inputJson'])
                if name=='missing-query':episode['queries']=[v for v in episode['queries'] if v['queryId']!='q2']
                else:episode['relevant']['q0']=['turn-3']
                text=h['encoded'](episode).decode();row.update(inputJson=text,inputSha256=h['digest'](text.encode()))
            else:
                raw['episodes'].reverse();raw['episodeRoster'].reverse();value['arms'].reverse()
                for row in raw['episodes']:
                    episode=json.loads(row['inputJson']);episode['queries'].reverse();text=h['encoded'](episode).decode();row.update(inputJson=text,inputSha256=h['digest'](text.encode()))
            update(arm,'groupedInput',raw)
        path=a.out/(name+'.json');dest=a.out/(name+'-scores.json');h['save'](path,value)
        command('score-'+name,[a.scorer,'--paired-input',path,'--out',dest],name=='permuted',None if name=='permuted' else dest)
        if name=='permuted':assert json.loads(dest.read_text())==scores
    assert 'paired source/labels/query/split universe differs' in (a.out/'score-changed-label.log').read_text()
    for name in ('last-cache','labels','seed','stream','version','missing-arm','duplicate-arm','reset'):
        dest=a.out/('negative-'+name);dest.mkdir()
        for path in inputs.rglob('*.json'):
            target=dest/path.relative_to(inputs);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(path.read_bytes())
        manifest=json.loads((dest/'manifest.json').read_text())
        if name in ('last-cache','labels'):
            entry=next(v for v in manifest['arms'] if v['role']=='candidate');arm_path=dest/entry['manifest']['file'];arm=json.loads(arm_path.read_text())
            ref=arm['episodes'][-1]['artifacts']['replayCache' if name=='last-cache' else 'evaluatorLabels'];path=arm_path.parent/ref['file'];data=json.loads(path.read_text())
            if name=='last-cache':data['entries']['unrecorded-key']=[1.,0.,0.,0.]
            else:data['relevant']['q0']=['turn-3']
            h['save'](path,data);ref['sha256']=h['digest'](path.read_bytes());h['save'](arm_path,arm);entry['manifest']['sha256']=h['digest'](arm_path.read_bytes())
        elif name=='seed':manifest['uncertaintyPolicy']['seed']=8
        elif name=='stream':manifest['uncertaintyPolicy']['streams']['train']=1
        elif name=='version':manifest['pairVersion']=2
        elif name=='missing-arm':manifest['arms'].pop()
        elif name=='duplicate-arm':manifest['arms'][1]=manifest['arms'][0]
        else:manifest['capabilities']['reset']=True
        h['save'](dest/'manifest.json',manifest);run('reject-'+name,dest,a.out/('rejected-'+name),False)
    h['save'](a.out/'validation.json',dict(status='passed',episodes=14,actions=210,queryEvents=56,scoredQueries=28,zeroLabelUnscored=14,unknownAnnotations=14,
        manifestNegatives=8,scorerNegatives=7,permutationEqual=True,trainReplicates=10000,otherSplitIntervals=None,
        frozenBeforeOutcomes=frozen,armComparisons=comparisons,qualityPromotionEligible=False,
        note='Two actual public limits on identical predeclared synthetic source/labels; same whole-group draws. Three training groups are diagnostic only. No model tokens, cost, training, reset or upstream semantics claim.'))


if __name__=='__main__':main()
