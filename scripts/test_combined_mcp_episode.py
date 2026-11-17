#!/usr/bin/env python3
"""Actual frozen consumer proof; invokes existing public runner, never an alternate engine."""
import argparse
import json
from pathlib import Path
import runpy
import shutil
from types import SimpleNamespace


def main():
    parser=argparse.ArgumentParser()
    for key in ('source','binary','scorer','build','assets','out'):parser.add_argument('--'+key,type=Path,required=True)
    args=parser.parse_args(); h=runpy.run_path(str(args.source/'scripts/public-mcp-evaluation.py')); c=h['COMBINED']
    args.out.mkdir(); checks=[]
    def done(name,**facts):
        checks.append(dict(name=name,**facts));h['save'](args.out/'checks.json',checks);print(name,flush=True)
    def inputs(name,profile='boundary',variant='baseline',track=None):
        a=SimpleNamespace(track=track or c['TRACK'],binary=args.binary,scorer=args.scorer,build=args.build,
            out=args.out/(name+'-inputs'),context_profile=profile,context_assets=args.assets,combined_variant=variant)
        h['prepare'](a)
        return SimpleNamespace(binary=args.binary,scorer=args.scorer,source_root=args.source,context_assets=args.assets,
            manifest=a.out/'manifest.json',out=args.out/name)
    def run(a):h['run_episode'](a)
    baselines=[]
    for name,profile in (('default','default'),('boundary','boundary'),('boundary-repeat','boundary')):
        a=inputs(name,profile);run(a);baselines.append(a)
        roster=json.loads((a.out/'combined-roster.json').read_text());assert roster['plannedQueries']==28
        status=json.loads((a.out/'episode-status.json').read_text());assert (status['succeeded'],status['failed'],status['unattempted'])==(7,0,0)
        raw=json.loads((a.out/'grouped-scores.json').read_text());packed=json.loads((a.out/'packed-grouped-scores.json').read_text())
        # Verify real score denominators for every split, not just source hashes.
        for split in ('train','development','test'):
            for field in ('scored','zeroLabelUnscored','unknownAnnotations'):
                assert raw['splits'][split]['queryMicro'][field]==packed['splits'][split]['queryMicro'][field]
        done(name,episodes=7,queries=28,fullRawPackedPopulation=True)
    baseline=baselines[1]
    # Actual failed cohort: one declared cache entry absent, no outcome->empty conversion.
    negative=inputs('middle-cache-miss',variant='middle-cache-miss')
    try:run(negative)
    except (RuntimeError,ValueError,AssertionError):pass
    else:raise AssertionError('expected actual mid-cohort failure')
    status=json.loads((negative.out/'episode-status.json').read_text())
    assert (status['planned'],status['attempted'],status['succeeded'],status['failed'],status['unattempted'])==(7,4,3,1,3)
    assert not (negative.out/'grouped-scores.json').exists() and not (negative.out/'packed-grouped-scores.json').exists()
    failed=negative.out/'episodes/episode-3'
    provider=h['rows'](failed/'actor-provider.jsonl');assert provider[-1]['status']==400
    wire=h['rows'](failed/'actor-wire.jsonl')
    assert any(r.get('tag')=='query:q0' and r['event']=='outcome' and r['response']['result']['isError'] for r in wire)
    done('actual-midcohort-cache-miss',planned=7,succeeded=3,failed=1,unattempted=3,aggregatesAbsent=True)
    # Immutable valid inputs copied only for explicit negative artifact checks.
    for name,mutation in (
        ('last-member',lambda folder,m:(folder/'episodes/episode-6/actor-inputs.json').write_text('{}')),
        ('trace-version',lambda folder,m:m.update(traceSchemaVersion=1)),
        ('old-profile-v2',lambda folder,m:m.update(track=h['GROUPED']['TRACK'])),
        ('bad-binary',lambda folder,m:m['executables'].update(ownedEvaluator='0'*64))):
        folder=args.out/(name+'-inputs');shutil.copytree(baseline.manifest.parent,folder)
        m=json.loads((folder/'manifest.json').read_text());mutation(folder,m);h['save'](folder/'manifest.json',m)
        a=SimpleNamespace(**vars(baseline));a.manifest=folder/'manifest.json';a.out=args.out/(name+'-forbidden')
        try:run(a)
        except (ValueError,KeyError):pass
        else:raise AssertionError('invalid preflight accepted '+name)
        assert not a.out.exists();done(name,outputAbsent=True)
    # Actual actor boundary canaries use one member after full seven-member preflight.
    original=baseline.out/'episodes/episode-0'
    def requests(root):return [r['request'] for r in h['rows'](root/'actor-wire.jsonl') if r['event']=='attempt']
    def providers(root):return [(r['requestBody'],r['status'],r['responseVectors']) for r in h['rows'](root/'actor-provider.jsonl')]
    for variant in ('gold','future'):
        a=inputs('canary-'+variant,variant=variant);_,bundles,_,_=c['validate'](a,h)
        h['run_episode'](a,prepared=bundles[0])
        if variant=='gold':
            assert requests(original)==requests(a.out);assert providers(original)==providers(a.out)
            for p in (original/'actor-inputs').glob('*.json'):assert p.read_bytes()==(a.out/'actor-inputs'/p.name).read_bytes()
            left=h['rows'](original/'actor-provider.jsonl');right=h['rows'](a.out/'actor-provider.jsonl')
            assert all(x['cacheKeys']!=y['cacheKeys'] for x,y in zip(left,right))
        else:
            left=[r for r in h['rows'](original/'actor-wire.jsonl') if r['event']=='attempt' and r['deliveryOrdinal']<4]
            right=[r for r in h['rows'](a.out/'actor-wire.jsonl') if r['event']=='attempt' and r['deliveryOrdinal']<4]
            assert [r['request'] for r in left]==[r['request'] for r in right]
            for i in range(4):assert (original/'actor-inputs'/f'step-{i}.json').read_bytes()==(a.out/'actor-inputs'/f'step-{i}.json').read_bytes()
        # Preserve actual UUID/renders; compare inclusion only descriptively by source mapping.
        contexts=[]
        for root in (original,a.out):
            mapping=json.loads((root/'source-mappings.json').read_text())['mappings']
            values=json.loads((root/'evaluator/context-validation.json').read_text())['queries']
            contexts.append({q:dict(tokens=v['fullTokenCount'],includedSources=[mapping[i] for i in v['includedMemoryIds']],renderedSha256=v['renderedSha256']) for q,v in values.items()})
        done('actual-'+variant+'-canary',actorBoundaryPassed=True,actualContexts=contexts,identityNormalizationUsed=False)
    for name,track in (('legacy-accounting',h['ACCOUNTING_TRACK']),('legacy-owned',h['OWNED']['TRACK']),('legacy-context',h['CONTEXT']['TRACK'])):
        a=inputs(name,track=track)
        # Legacy accounting/context consume the actual production CLI, not owned_evaluator.
        if track!=h['OWNED']['TRACK']:
            a.binary=args.binary.parent.parent/'recalld'
            m=json.loads(a.manifest.read_text());m['executables']={'recalld':h['digest'](a.binary.read_bytes()),'scorer':h['digest'](args.scorer.read_bytes())};h['save'](a.manifest,m)
        if track==h['OWNED']['TRACK']:
            try:run(a)
            except ValueError as error:
                assert str(error)=='unsupported private control capabilities'
                assert not list((a.out/'actor-inputs').iterdir())
                done(name,status='expected-incompatibility',reason=str(error),oldProfileUnchanged=True)
            else:raise AssertionError('old owned profile unexpectedly accepted new constructor')
        else:run(a);done(name,status='passed')
    h['save'](args.out/'result.json',dict(status='passed',successfulCombinedCohorts=3,combinedEpisodes=21,combinedQueries=84,checks=checks,
        trueDelayedWorkerTimeoutProven=False,independentStartupDeterminism=False,fullA6=False))


if __name__=='__main__':main()
