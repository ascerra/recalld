#!/usr/bin/env python3
"""Actual recorded-answer cohort and narrow negatives, using the existing runner."""
import argparse
import copy
import json
from pathlib import Path
import runpy
import shutil
from types import SimpleNamespace


def main():
    p=argparse.ArgumentParser()
    for key in ('source','binary','scorer','build','assets','out'):p.add_argument('--'+key,type=Path,required=True)
    args=p.parse_args();h=runpy.run_path(str(args.source/'scripts/public-mcp-evaluation.py'));a=h['ANSWER'];c=h['COMBINED']
    args.out.mkdir();checks=[]
    def done(name,**facts):
        checks.append(dict(name=name,**facts));h['save'](args.out/'checks.json',checks);print(name,flush=True)
    def inputs(name,variant='baseline'):
        x=SimpleNamespace(track=a['TRACK'],binary=args.binary,scorer=args.scorer,build=args.build,out=args.out/(name+'-inputs'),
            context_profile='default',context_assets=args.assets,combined_variant='baseline',answer_variant=variant)
        h['prepare'](x)
        return SimpleNamespace(binary=args.binary,scorer=args.scorer,source_root=args.source,context_assets=args.assets,manifest=x.out/'manifest.json',out=args.out/name)
    baseline=inputs('default');h['run_episode'](baseline) # Stop the whole driver if exact retained contexts differ.
    summary=json.loads((baseline.out/'recorded-answer-summary.json').read_text());assert(summary['episodes'],summary['answers'])==(7,28)
    status=json.loads((baseline.out/'episode-status.json').read_text());assert(status['succeeded'],status['failed'],status['unattempted'])==(7,0,0)
    raw=json.loads((baseline.out/'grouped-scores.json').read_text());packed=json.loads((baseline.out/'packed-grouped-scores.json').read_text())
    counts={k:sum(int(raw['splits'][s]['queryMicro'][k]) for s in ('train','development','test')) for k in ('scored','zeroLabelUnscored','unknownAnnotations')}
    assert counts==dict(scored=9,zeroLabelUnscored=6,unknownAnnotations=13)
    for split in ('train','development','test'):
        for field in counts:assert raw['splits'][split]['queryMicro'][field]==packed['splits'][split]['queryMicro'][field]
    done('complete-cohort',episodes=7,answers=28,denominators=counts)
    # Real retained payload, with a self-consistent but false actor receipt.
    _,bundles,_,_=c['validate'](baseline,h,answer=True);member=bundles[0];original=baseline.out/'episodes/episode-0'
    wire=h['rows'](original/'actor-wire.jsonl');forged=copy.deepcopy(wire)
    outcome=next(r for r in forged if r['event']=='outcome' and r['tag']=='query:q0')
    public=json.loads(outcome['response']['result']['content'][0]['text'])['memories'];public[0]['summary']='forged self-consistent evidence'
    context=next(r for r in forged if r['event']=='context' and r['tag']=='context:q0')
    token=h['CONTEXT']['TOKEN'];context['context']=token['assemble'](token['load'](args.assets),member[4]['queries'][0]['text'],public,member[0]['contextPolicy'])
    forbidden=SimpleNamespace(**vars(baseline));forbidden.out=args.out/'forged-context-forbidden'
    try:a['step'](forbidden,h,member[0],member[4]['queries'][0],1,forged,Path('/unused'),Path('/unused'),[],[])
    except ValueError as error:assert str(error)=='actor rendered context differs from actual public payload/policy'
    else:raise AssertionError('self-consistent fake receipt accepted')
    assert not forbidden.out.exists();done('actual-payload-forged-receipt-rejected',actorNotDispatched=True)
    role=h['rows'](original/'evaluator/role-trace.jsonl');trace=h['rows'](original/'trace.jsonl')
    assert a['reconcile'](member[0],role,trace,member[-1])['accepted']==4
    for field,value in [('environmentHash','foreign'),('queryId','unknown'),('publicStepId','foreign'),('cacheArtifactSha256','foreign'),('eventKind','rejected')]:
        changed=copy.deepcopy(role);changed[-1][field]=value
        try:a['reconcile'](member[0],changed,trace,member[-1])
        except ValueError:pass
        else:raise AssertionError('actual trace mutation accepted '+field)
    done('actual-retained-role-trace',valid=True,rejectedMutants=5)
    miss=inputs('middle-miss','middle-miss')
    try:h['run_episode'](miss)
    except ValueError as e:assert str(e)=='recorded-answer-cache-miss'
    else:raise AssertionError('role miss accepted')
    status=json.loads((miss.out/'episode-status.json').read_text());assert(status['succeeded'],status['failed'],status['unattempted'])==(3,1,3)
    assert not(miss.out/'grouped-scores.json').exists();assert not(miss.out/'packed-grouped-scores.json').exists()
    done('actual-middle-answer-miss',succeeded=3,failed=1,unattempted=3)
    for variant,reason in [('refusal','recorded-refusal'),('parse','output-parse'),('utf8','output-parse'),('citation','citation outside actual packed public payload')]:
        x=inputs(variant,variant);_,bundles,_,_=c['validate'](x,h,answer=True)
        try:h['run_episode'](x,prepared=bundles[0])
        except ValueError as e:assert str(e)==reason,(variant,str(e))
        else:raise AssertionError('bad recorded output accepted')
        assert not(x.out/'evaluator/scores.json').exists()
        role=json.loads((x.out/'evaluator/role-status.json').read_text());assert(role['rejected'],role['unattempted'])==(1,3)
        assert(x.out/'answer-1-actor.json').exists();done('actual-'+variant,reason=reason)
    for name,mutation in [('last-member',lambda root,m:(root/m['episodes'][-1]['roleCache']['file']).write_text('{}')),
        ('old-track',lambda root,m:m.update(track=c['TRACK'])),('role-version',lambda root,m:m['answerPolicy'].update(roleTraceSchemaVersion=2))]:
        folder=args.out/(name+'-inputs');shutil.copytree(baseline.manifest.parent,folder)
        m=json.loads((folder/'manifest.json').read_text());mutation(folder,m);h['save'](folder/'manifest.json',m)
        x=SimpleNamespace(**vars(baseline));x.manifest=folder/'manifest.json';x.out=args.out/(name+'-forbidden')
        try:h['run_episode'](x)
        except (ValueError,KeyError):pass
        else:raise AssertionError('invalid preflight accepted')
        assert not x.out.exists();done(name,outputAbsent=True)
    original=baseline.out/'episodes/episode-0'
    for variant in ('gold','future'):
        x=inputs('canary-'+variant,variant);_,bundles,_,_=c['validate'](x,h,answer=True)
        if variant=='gold':h['run_episode'](x,prepared=bundles[0])
        else:
            try:h['run_episode'](x,prepared=bundles[0])
            except ValueError as e:assert str(e)=='recorded-answer-cache-miss'
            else:raise AssertionError('changed current input was silently rekeyed')
        bound=6 if variant=='gold' else 4
        for file in (original/'actor-inputs').glob('*.json'):
            ordinal=int(file.stem.split('-')[-1])
            if ordinal<bound:assert file.read_bytes()==(x.out/'actor-inputs'/file.name).read_bytes(),file
        left=h['rows'](original/'actor-wire.jsonl');right=h['rows'](x.out/'actor-wire.jsonl')
        request=lambda rows:[r['request'] for r in rows if r['event']=='attempt' and r['deliveryOrdinal']<bound]
        assert request(left)==request(right)
        lp=h['rows'](original/'actor-provider.jsonl');rp=h['rows'](x.out/'actor-provider.jsonl')
        payload=lambda rows:[(r['requestBody'],r['status'],r['responseVectors']) for r in rows if r['deliveryOrdinal']<bound]
        assert payload(lp)==payload(rp)
        for ordinal in range(1,5 if variant=='gold' else 4):
            assert(original/f'answer-{ordinal}-actor.json').read_bytes()==(x.out/f'answer-{ordinal}-actor.json').read_bytes()
        assert lp[0]['environmentHash']!=rp[0]['environmentHash'];assert lp[0]['cacheKeys']!=rp[0]['cacheKeys']
        done('actual-'+variant+'-canary',rawActorBehaviorCompared=True,identityNormalizationUsed=False)
    h['save'](args.out/'result.json',dict(status='passed',checks=checks,answerQuality=None,fullA6=False))


if __name__=='__main__':main()
