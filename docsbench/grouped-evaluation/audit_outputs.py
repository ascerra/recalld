#!/usr/bin/env python3
"""Audit retained actual cohort outputs, including semantic rosters and denominators."""
import argparse
import hashlib
import json
from pathlib import Path


def read(path): return json.loads(path.read_text())
def rows(path): return [json.loads(line) for line in path.read_text().splitlines()]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def audit(actual):
    baseline, changed = actual/'baseline', actual/'changed-labels'
    inputs, other = actual/'grouped-inputs', actual/'changed-label-inputs'
    group_map, split_map = read(inputs/'group-map.json'), read(inputs/'split-map.json')
    assert group_map == read(other/'group-map.json') and split_map == read(other/'split-map.json')
    assert group_map['episodes']['episode-0'] == group_map['episodes']['episode-1']
    assert group_map['episodes']['episode-2'] == group_map['episodes']['episode-3']
    aggregate = read(baseline/'grouped-scores.json')
    expected = {'train':(16,4,6,6,3,1,2,.375,.25), 'development':(8,4,0,4,1,1,0,.625,.5), 'test':(4,1,0,3,1,1,0,1.,1.)}
    summaries=[]
    for split, (queries,scored,zero,unknown,groups,scored_groups,unscored_groups,recall,all_required) in expected.items():
        value=aggregate['splits'][split]; micro=value['queryMicro']; macro=value['groupMacro']
        assert tuple(micro[k] for k in ('queryEvents','scored','zeroLabelUnscored','unknownAnnotations')) == tuple(map(str,(queries,scored,zero,unknown)))
        assert tuple(macro[k] for k in ('groups','scoredGroups','unscoredGroups')) == tuple(map(str,(groups,scored_groups,unscored_groups)))
        assert micro['means']['retrievalRecallAt5']==macro['means']['retrievalRecallAt5']==recall
        assert micro['allRequiredEvidenceRate']==macro['allRequiredEvidenceRate']==all_required
        summaries.append(dict(split=split,queryEvents=queries,scoredQueries=scored,zeroLabelUnscored=zero,unknownAnnotations=unknown,
                              groups=groups,scoredGroups=scored_groups,unscoredGroups=unscored_groups,recallAt5=recall,allRequiredRate=all_required))
    for episode, status_key in (('episode-5','unknownAnnotations'),('episode-6','zeroLabelUnscored')):
        group=group_map['episodes'][episode];split=split_map['groups'][group]
        summary=aggregate['splits'][split]['groups'][group]
        assert summary['scored']=='0' and summary[status_key]=='4'
        assert all(v is None for v in summary['means'].values()) and summary['allRequiredEvidenceRate'] is None
    comparisons=[]; action_count=0
    for i in range(7):
        identity=f'episode-{i}'; left,right=[root/'episodes'/identity for root in (baseline,changed)]
        ls,rs=[read(root/'source-mappings.json') for root in (left,right)]
        # Actual acknowledged public IDs are separately retained; compare source membership,
        # not renamed IDs or rewritten output. Each run must map all four exactly once.
        assert ls['sources']==rs['sources']
        source_ids=[s['sourceId'] for s in ls['sources']]
        assert source_ids==[f'turn-{j}' for j in range(4)]
        assert sorted(ls['mappings'].values())==sorted(rs['mappings'].values())==[[s] for s in source_ids]
        group=group_map['episodes'][identity]; split=split_map['groups'][group]
        for root in (left,right):
            context=read(root/'evaluator/group-context.json')
            assert context['episodeId']==identity and context['groupId']==group and context['split']==split
            assert context['queryIds']==['q0','q1','q2','q3']
            raw=read(root/'evaluator/score-input.json');assert sorted(q['queryId'] for q in raw['queries'])==context['queryIds']
            events=rows(root/'evaluator/source-events.jsonl')
            assert sorted(e['sourceId'] for e in events if e['status']=='accepted')==source_ids
            assert len(events)==12
            trace=rows(root/'trace.jsonl');assert len(trace)==15 and all(r['groupId']==group for r in trace)
            assert sum(r['status']==400 for r in rows(root/'actor-provider.jsonl'))==1
            action_count+=len(trace)
        comparisons.append(dict(episodeId=identity,groupId=group,split=split,sourceRowsEqual=True,
            acknowledgedSourceMembershipEqual=True,actualQueryIds=['q0','q1','q2','q3'],acceptedSourcesPerRun=4))
    for root in (baseline,changed):
        status=read(root/'episode-status.json');assert len(status['episodes'])==7 and status['succeeded']==7 and status['failed']==0 and status['unattempted']==0
        assert sorted(e['episodeId'] for e in status['episodes'])==[f'episode-{i}' for i in range(7)]
        assert all(e['status']=='passed' and e['queryIds']==['q0','q1','q2','q3'] for e in status['episodes'])
        scores=read(root/'grouped-scores.json');assert scores['episodes']=='7'
        assert sum(int(s['queryMicro']['queryEvents']) for s in scores['splits'].values())==28
    changed_scores=read(changed/'grouped-scores.json')
    assert sum(int(s['queryMicro']['scored']) for s in changed_scores['splits'].values())==7
    assert sum(int(s['queryMicro']['unknownAnnotations']) for s in changed_scores['splits'].values())==21
    assert all(s['queryMicro']['means']['retrievalRecallAt5']==0 for s in changed_scores['splits'].values())
    action_count+=len(rows(actual/'old-profile/trace.jsonl'));assert action_count==225
    return dict(status='passed',actualEpisodes=15,actualActions=action_count,groupedQueries=56,baselineDenominators=summaries,
        labelPerturbedScoredQueries=7,labelPerturbedUnknownQueries=21,labelPerturbedRecallAt5=0,
        sourceMappingComparisons=comparisons,sourceGroupMapsEqual=True,
        limitation='Exact source-history membership only; public UUIDs and timestamps retained without cross-run identity claim.')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('actual',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args()
    result=audit(args.actual)
    result['auditorSha256']=digest(Path(__file__))
    result['inputFilesSha256']={str(p.relative_to(args.actual)):digest(p) for p in sorted(args.actual.rglob('*')) if p.is_file()}
    with args.output.open('x') as out:json.dump(result,out,indent=2,sort_keys=True);out.write('\n')
    print(json.dumps({k:v for k,v in result.items() if k not in ('sourceMappingComparisons','inputFilesSha256')},indent=2))
