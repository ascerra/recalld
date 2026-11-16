#!/usr/bin/env python3
"""Run the actual synthetic cohort/scorer and retain positive and negative receipts."""
import argparse
import copy
import json
from pathlib import Path
import runpy
import subprocess
import sys

h = runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))
g = h['GROUPED']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for flag in ('binary', 'scorer', 'source-root', 'build', 'out'):
        p.add_argument('--' + flag, type=Path, required=True)
    a = p.parse_args(); a.out.mkdir()
    receipts = []

    def command(name, argv, success=True, absent=None):
        with (a.out / (name + '.log')).open('wb') as log:
            code = subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT).returncode
        receipts.append(dict(name=name, command=list(map(str, argv)), exitCode=code, expectedSuccess=success,
                             outputAbsent=None if absent is None else not absent.exists()))
        h['save'](a.out / 'receipts.json', receipts)
        assert (code == 0) == success, name
        if absent is not None: assert not absent.exists(), name

    runner = [sys.executable, str(Path(__file__).with_name('public-mcp-evaluation.py'))]
    def prepare(name, track):
        dest = a.out / name
        command(name, [*runner, 'prepare', '--track', track, '--binary', a.binary, '--scorer', a.scorer, '--build', a.build, '--out', dest])
        return dest
    def run(name, inputs, success=True):
        dest = a.out / name
        command(name, [*runner, 'run', '--binary', a.binary, '--scorer', a.scorer, '--source-root', a.source_root,
                      '--manifest', inputs / 'manifest.json', '--out', dest], success, None if success else dest)
        return dest
    def rewrite(inputs, ref, value):
        path = inputs / ref['file']; h['save'](path, value); ref['sha256'] = h['digest'](path.read_bytes())
    def variant(name):
        # Small fixture JSON only; never clone stores, binaries, or build caches.
        dest = a.out / name; dest.mkdir()
        for path in base_inputs.rglob('*.json'):
            target = dest / path.relative_to(base_inputs); target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(path.read_bytes())
        return dest, json.loads((dest / 'manifest.json').read_bytes())

    base_inputs = prepare('grouped-inputs', g['TRACK'])
    baseline = run('baseline', base_inputs)
    aggregate = json.loads((baseline / 'grouped-scores.json').read_text())
    status = json.loads((baseline / 'episode-status.json').read_text())
    assert (status['planned'], status['succeeded'], status['failed'], status['unattempted']) == (7, 7, 0, 0)
    assert len(status['episodes']) == 7 and all(r['status'] == 'passed' for r in status['episodes'])
    # Hand-calculated per-episode metrics for the actual limit-one source-0 fixture.
    expected = ((2,1,1,.75), (2,1,1,0.), (2,0,2,1.), (2,0,2,.25), (1,0,3,1.), (0,0,4,None), (0,4,0,None))
    for i, (scored, zero, unknown, recall) in enumerate(expected):
        ep = baseline / 'episodes' / f'episode-{i}'
        value = json.loads((ep / 'evaluator/scores.json').read_text())['scoring']
        assert (value['scored'], value['zeroLabelUnscored'], value['unknownAnnotations']) == tuple(map(str, (scored, zero, unknown)))
        assert value['means']['retrievalRecallAt5'] == recall
        score = json.loads((ep / 'evaluator/score-input.json').read_text())
        assert all(score['mappings'][query['memoryIds'][0]] == ['turn-0'] for query in score['queries'])
    assert sum(int(v['queryMicro']['queryEvents']) for v in aggregate['splits'].values()) == 28
    assert sum(int(v['groupMacro']['groups']) for v in aggregate['splits'].values()) == 5
    assert sum(int(v['groupMacro']['unscoredGroups']) for v in aggregate['splits'].values()) == 2

    changed, manifest = variant('changed-label-inputs')
    for entry in manifest['episodes']:
        ref = entry['artifacts']['evaluatorLabels']; labels = json.loads((changed / ref['file']).read_text())
        labels.update(labelCanary='CHANGED_EVALUATOR_LABEL_CANARY', relevant={'q0':['turn-3']})
        rewrite(changed, ref, labels)
    h['save'](changed / 'manifest.json', manifest)
    perturbed = run('changed-labels', changed)
    comparisons = []
    for identity in (f'episode-{i}' for i in range(7)):
        left, right = [root / 'episodes' / identity for root in (baseline, perturbed)]
        attempts = lambda root: [r for r in h['rows'](root / 'actor-wire.jsonl') if r['event'] == 'attempt']
        providers = lambda root: [{k:v for k,v in r.items() if k not in ('cacheKeys', 'environmentHash')} for r in h['rows'](root / 'actor-provider.jsonl')]
        lr, rr = [json.loads((root / 'result.json').read_text()) for root in (left, right)]
        checks = dict(episodeId=identity, actorCodeEqual=(left/'actor-code.py').read_bytes()==(right/'actor-code.py').read_bytes(),
            actorInputsEqual=all(path.read_bytes()==(right/'actor-inputs'/path.name).read_bytes() for path in (left/'actor-inputs').glob('*.json')),
            actualRequestsEqual=attempts(left)==attempts(right), providerInputsVectorsEqual=providers(left)==providers(right),
            environmentChanged=lr['environmentHash']!=rr['environmentHash'],
            cacheIdentityChanged=all(l['cacheKeys']!=r['cacheKeys'] for l,r in zip(h['rows'](left/'actor-provider.jsonl'),h['rows'](right/'actor-provider.jsonl'))),
            futureCanaryWithheldBoth=lr['futureQueryCanaryWithheldUntilDelivery'] and rr['futureQueryCanaryWithheldUntilDelivery'])
        comparisons.append(checks); h['save'](a.out/'label-comparison.json', comparisons)
        assert all(v for k,v in checks.items() if k != 'episodeId'), checks
    for name in ('source-registry.json','group-map.json','split-map.json'):
        assert (base_inputs/name).read_bytes()==(changed/name).read_bytes()

    for name in ('last-cache','duplicate-history','split','version','missing','duplicate','unsupported-reset','old-track-fields'):
        inputs, m = variant('negative-' + name)
        if name == 'last-cache':
            ref=m['episodes'][-1]['artifacts']['replayCache']; value=json.loads((inputs/ref['file']).read_text());value['entries'].pop('a6-document:amber fir');rewrite(inputs,ref,value)
        elif name == 'duplicate-history':
            ref=m['groupMap']; value=json.loads((inputs/ref['file']).read_text()); old=value['episodes']['episode-1'];value['groups'][old].remove('episode-1');value['groups']['f'*64]=['episode-1'];value['episodes']['episode-1']='f'*64;rewrite(inputs,ref,value)
            ref=m['splitMap']; value=json.loads((inputs/ref['file']).read_text());value['groups']['f'*64]='test' if value['groups'][old]!='test' else 'train';rewrite(inputs,ref,value)
        elif name == 'split':
            ref=m['splitMap']; value=json.loads((inputs/ref['file']).read_text());group=next(iter(value['groups']));value['groups'][group]='test' if value['groups'][group]!='test' else 'train';rewrite(inputs,ref,value)
        elif name == 'version': m['groupMapVersion']=2
        elif name == 'missing': m['episodes'].pop()
        elif name == 'duplicate': m['episodes'][-1]=m['episodes'][0]
        elif name == 'unsupported-reset': m['capabilities']['reset']=True
        else: m['track']=h['TRACK']
        h['save'](inputs/'manifest.json',m);run('reject-'+name,inputs,False)
    assert 'group map differs from source histories' in (a.out/'reject-duplicate-history.log').read_text()
    assert 'split map differs from whole-group assignments' in (a.out/'reject-split.log').read_text()
    original=json.loads((baseline/'grouped-score-input.json').read_text())
    for name in ('missing-episode','duplicate-episode','missing-query','swapped-member','permuted'):
        value=copy.deepcopy(original)
        if name=='missing-episode':value['episodes'].pop()
        elif name=='duplicate-episode':value['episodes'].append(value['episodes'][0])
        elif name=='swapped-member':value['episodes'][0].update({k:value['episodes'][1][k] for k in ('inputJson','inputSha256')})
        elif name=='missing-query':
            # Remove an unknown query: the old per-query scorer alone cannot detect omission.
            row=value['episodes'][0]; raw=json.loads(row['inputJson']);raw['queries']=[q for q in raw['queries'] if q['queryId']!='q2'];text=h['encoded'](raw).decode();row.update(inputJson=text,inputSha256=h['digest'](text.encode()))
        else:
            value['episodes'].reverse();value['episodeRoster'].reverse()
            for row in value['episodes']:
                raw=json.loads(row['inputJson']);raw['queries'].reverse();text=h['encoded'](raw).decode();row.update(inputJson=text,inputSha256=h['digest'](text.encode()))
        path=a.out/(name+'.json');h['save'](path,value);output=a.out/(name+'-scores.json')
        command('score-'+name,[a.scorer,'--grouped-input',path,'--out',output],name=='permuted', None if name=='permuted' else output)
        if name=='permuted': assert json.loads(output.read_text())==aggregate
    old=prepare('old-inputs',h['TRACK']); run('old-profile',old)
    h['save'](a.out/'validation.json',dict(status='passed',episodes=15,groupedEpisodes=14,groupedQueryEvents=56,
        manifestNegatives=8,scorerNegatives=4,scorerPermutationEqual=True,labelComparisons=comparisons,
        note='Actual requests/provider vectors compared exactly; independent UUIDs, timestamps and raw outputs retained without equality claim. Synthetic contract only.'))


if __name__=='__main__': main()
