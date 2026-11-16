#!/usr/bin/env python3
"""Actual serialized public121-to100 graded reference consumer and isolation gates."""
import argparse
import copy
import json
import math
from pathlib import Path
import runpy
import subprocess
import sys

h=runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))
b=h['BEIR']


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for flag in ('binary','scorer','source-root','build','out','beir-reference'): p.add_argument('--'+flag,type=Path,required=True)
    a=p.parse_args(); a.out.mkdir(); receipts=[]
    runner=[sys.executable,str(Path(__file__).with_name('public-mcp-evaluation.py'))]
    def command(name, argv, success=True, absent=None):
        with (a.out/(name+'.log')).open('wb') as log: code=subprocess.run(list(map(str,argv)),stdout=log,stderr=subprocess.STDOUT).returncode
        receipts.append(dict(name=name,command=list(map(str,argv)),exitCode=code,expectedSuccess=success,outputAbsent=None if absent is None else not absent.exists()))
        h['save'](a.out/'receipts.json',receipts)
        assert (code==0)==success, name
        if absent is not None: assert not absent.exists(), name
    def prepare(name,track):
        path=a.out/name
        command(name,[*runner,'prepare','--track',track,'--binary',a.binary,'--scorer',a.scorer,'--build',a.build,'--out',path,'--beir-reference',a.beir_reference])
        return path
    def run(name,inputs,success=True):
        path=a.out/name
        command(name,[*runner,'run','--binary',a.binary,'--scorer',a.scorer,'--source-root',a.source_root,'--manifest',inputs/'manifest.json','--out',path,'--beir-reference',a.beir_reference],success,None if success else path)
        return path
    def rewrite(inputs, ref, value):
        path=inputs/ref['file']; h['save'](path,value); ref['sha256']=h['digest'](path.read_bytes())
    def variant(name):
        path=a.out/name; path.mkdir()
        for f in base_inputs.glob('*.json'): (path/f.name).write_bytes(f.read_bytes())
        return path,json.loads((path/'manifest.json').read_text())
    def verify(path):
        status=json.loads((path/'result.json').read_text()); assert status['status']=='passed'
        value=json.loads((path/'evaluator/beir-scores.json').read_text())
        source=json.loads((path/'evaluator/beir-score-input.json').read_text())
        mapping=json.loads((path/'source-mappings.json').read_text())
        assert len(mapping['mappings'])==122 and sum(s['namespace']=='default' for s in mapping['sources'])==121
        assert value['knownQueries']==['q0','q1','q2'] and value['unknownQueries']==['q3'] and value['officialDenominator']=='3'
        assert len(source['qrels']['q0'])==121 and len(source['runs']['q0'])==100
        assert len(set(source['qrels']['q0'])-set(source['runs']['q0']))==21
        dense=value['perQuery']['q0']
        assert math.isclose(dense['map_cut_100'],100/121)
        for k in (5,10,20): assert math.isclose(dense[f'recall_{k}'],k/121)
        binary=json.loads((path/'evaluator/scores.json').read_text())['scoring']
        assert (binary['scored'],binary['zeroLabelUnscored'],binary['unknownAnnotations'])==('2','1','1')
        for q in source['roster']:
            observations=value['publicOrder'][q]
            assert [r['referenceOrdinalScore'] for r in observations]==list(range(100,0,-1))
            assert list(source['runs'][q])==[r['sourceId'] for r in observations] or all(source['runs'][q][r['sourceId']]==r['referenceOrdinalScore'] for r in observations)
        return value
    base_inputs=prepare('inputs',b['TRACK']); baseline=run('baseline',base_inputs); verify(baseline)
    inputs,m=variant('changed-label-inputs'); labels=json.loads((inputs/m['evaluatorLabels']['file']).read_text())
    labels['metricQrels']['q0']={d:4-g for d,g in labels['metricQrels']['q0'].items()}; labels['labelCanary']='CHANGED_EVALUATOR_LABEL_CANARY'
    rewrite(inputs,m['evaluatorLabels'],labels); h['save'](inputs/'manifest.json',m)
    changed=run('changed-labels',inputs); verify(changed)
    attempts=lambda root:[r for r in h['rows'](root/'actor-wire.jsonl') if r['event']=='attempt']
    providers=lambda root:[{k:v for k,v in r.items() if k not in ('cacheKeys','environmentHash')} for r in h['rows'](root/'actor-provider.jsonl')]
    comparison=dict(actorCodeEqual=(baseline/'actor-code.py').read_bytes()==(changed/'actor-code.py').read_bytes(),
        actorInputsEqual=all(f.read_bytes()==(changed/'actor-inputs'/f.name).read_bytes() for f in (baseline/'actor-inputs').glob('*.json')),
        requestsEqual=attempts(baseline)==attempts(changed),providerInputsVectorsEqual=providers(baseline)==providers(changed),
        environmentChanged=json.loads((baseline/'result.json').read_text())['environmentHash']!=json.loads((changed/'result.json').read_text())['environmentHash'])
    h['save'](a.out/'label-comparison.json',comparison); assert all(comparison.values())
    inputs,m=variant('future-inputs'); actor,labels,cache,schedule=b['fixtures'](h,True)
    for key,value in [('actorInputs',actor),('evaluatorLabels',labels),('replayCache',cache),('evaluatorSchedule',schedule)]: rewrite(inputs,m[key],value)
    h['save'](inputs/'manifest.json',m); future=run('changed-future',inputs); verify(future)
    before=lambda root:[r for r in attempts(root) if r['deliveryOrdinal']<4]
    assert before(baseline)==before(future)
    for i in range(4): assert (baseline/'actor-inputs'/f'step-{i}.json').read_bytes()==(future/'actor-inputs'/f'step-{i}.json').read_bytes()
    for name in ('version','bool-version','reference-pin','negative-grade','bool-grade','empty-judgments','binary-projection','foreign-grade','limit101','old-track'):
        inputs,m=variant('negative-'+name); labels=json.loads((inputs/m['evaluatorLabels']['file']).read_text())
        if name=='version': m['metricPolicy']['metricAdapterVersion']=2
        elif name=='bool-version': m['metricPolicy']['metricAdapterVersion']=True
        elif name=='reference-pin': m['referenceBundleSha256']='0'*64
        elif name=='old-track': m['track']=h['TRACK']
        elif name=='limit101':
            schedule=json.loads((inputs/m['evaluatorSchedule']['file']).read_text()); schedule['steps'][0]['arguments']['limit']=101; rewrite(inputs,m['evaluatorSchedule'],schedule)
        else:
            if name=='negative-grade': labels['metricQrels']['q0']['doc000']=-1
            elif name=='bool-grade': labels['metricQrels']['q0']['doc000']=True
            elif name=='empty-judgments': labels['metricQrels']['q2']={}
            elif name=='foreign-grade': labels['metricQrels']['q0']['doc121']=1
            else: labels['relevant']['q0'].pop()
            rewrite(inputs,m['evaluatorLabels'],labels)
        h['save'](inputs/'manifest.json',m); run('reject-'+name,inputs,False)
    old=prepare('old-inputs',h['TRACK']); run('old-profile',old)
    h['save'](a.out/'validation.json',dict(status='passed',actualEpisodes=4,saturatedEpisodes=3,defaultDocumentsPerEpisode=121,returnedPerQuery=100,preflightNegatives=10,
        officialKnownQueries=3,binaryScoredQueries=2,binaryZeroLabelUnscored=1,unknownQueries=1,labelComparison=comparison,futurePrefixEqual=True,
        scope='Synthetic graded reference contract; ordinal scores are not engine scores; no dataset or quality claim.'))


if __name__=='__main__': main()
