#!/usr/bin/env python3
"""Source/checker tests. Mocked provenance/tokenizer never claims actual execution."""
import copy
import json
from pathlib import Path
import runpy
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

H = runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))
C = H['COMBINED']


def receipt(ordinal=-1, step='0'):
    at = dict(storeId='00000000-0000-4000-8000-000000000001', generation='0')
    barrier = dict(storeId=at['storeId'], committedGeneration='0', ready=True, pendingWork=0,
        derived=[dict(name=n, appliedGeneration='0', ready=True) for n in ('graph','vectors','fts','entities')])
    return dict(deliveryOrdinal=ordinal, acknowledgedThroughStepId=step, target=at, barrier=barrier, after=at)


class Combined(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def setup_manifest(self, variant='baseline'):
        binary=self.root/'binary'; binary.write_bytes(b'not-executable')
        build=self.root/'build.json'; H['save'](build, dict(sourceCommit='a'*40, sourceFilesSha256={},containerImageId='sha256:'+'b'*64))
        args=SimpleNamespace(binary=binary,scorer=binary,build=build,out=self.root/'inputs',context_profile='boundary',
            context_assets=self.root/'assets',combined_variant=variant,source_root=self.root,manifest=self.root/'inputs/manifest.json')
        h=dict(H); h['CONTEXT']=dict(H['CONTEXT']); h['CONTEXT']['TOKEN']=dict(H['CONTEXT']['TOKEN'],load=lambda _:None)
        with mock.patch('subprocess.check_output', return_value=b'{"activeEmbedding":{}}'):
            C['prepare'](args,h)
        h['validate_build']=lambda *a:None
        h['CONTEXT']['preflight']=lambda *a:None
        return args,h

    def test_source_grouping_ignores_labels_future_and_keeps_duplicates(self):
        registry=C['registry'](H); groups,splits=H['GROUPED']['maps'](registry,H)
        self.assertEqual(len(groups['groups']),5)
        self.assertEqual(groups['episodes']['episode-0'],groups['episodes']['episode-1'])
        for episode in registry['episodes']:
            baseline=C['fixture'](episode,H,'baseline')
            gold=C['fixture'](episode,H,'gold'); future=C['fixture'](episode,H,'future')
            self.assertEqual(baseline[0],gold[0]); self.assertEqual(baseline[2:],gold[2:])
            self.assertEqual(baseline[0],future[0]); self.assertEqual(baseline[3]['queries'][:3],future[3]['queries'][:3])
            self.assertNotEqual(baseline[3]['queries'][3],future[3]['queries'][3])
            self.assertEqual(sum(s['namespace']=='default' for s in baseline[0]['sources']),24)

    def test_complete_roster_and_last_member_preflight_before_output(self):
        args,h=self.setup_manifest(); m,bundles,roster,_=C['validate'](args,h)
        self.assertEqual(len(bundles),7); self.assertEqual(sum(len(r['queryIds']) for r in roster),28)
        victim=args.out/'episodes/episode-6/actor-inputs.json'; victim.write_text('{}')
        args.out=self.root/'forbidden-output'
        with self.assertRaisesRegex(ValueError,'hash/reference'): C['run'](args,h)
        self.assertFalse(args.out.exists())

    def test_duplicate_roster_and_cross_split_tampering_rejected(self):
        args,h=self.setup_manifest(); m=json.loads(args.manifest.read_text())
        original=copy.deepcopy(m); m['episodes'][-1]=m['episodes'][0]; H['save'](args.manifest,m)
        with self.assertRaisesRegex(ValueError,'duplicate'): C['validate'](args,h)
        H['save'](args.manifest,original)
        p=args.out/'split-map.json'; value=json.loads(p.read_text()); value['groups'][next(iter(value['groups']))]='wrong'
        H['save'](p,value); original['splitMap']['sha256']=H['digest'](p.read_bytes());H['save'](args.manifest,original)
        with self.assertRaisesRegex(ValueError,'grouping/split'): C['validate'](args,h)

    def test_real_negative_fixture_cache_miss_is_declared_and_preflight_valid(self):
        args,h=self.setup_manifest('middle-cache-miss'); _,bundles,_,_=C['validate'](args,h)
        for i,bundle in enumerate(bundles):
            query='a6-query:'+bundle[4]['queries'][0]['text']
            self.assertEqual(query in bundle[3]['entries'],i!=3)

    def test_midcohort_failure_preserves_roster_without_aggregate(self):
        args,h=self.setup_manifest('middle-cache-miss');prepared=C['validate'](args,h);args.out=self.root/'run'
        def episode(child,prepared):
            child.out.mkdir();(child.out/'evaluator').mkdir()
            if prepared[0]['groupContext']['episodeId']=='episode-3': raise RuntimeError('simulated episode failure; not runtime proof')
            (child.out/'evaluator/score-input.json').write_text('{}')
        h['run_episode']=episode
        with self.assertRaisesRegex(RuntimeError,'simulated'):H['GROUPED']['run'](args,h,prepared)
        status=json.loads((args.out/'episode-status.json').read_text())
        self.assertEqual((status['planned'],status['attempted'],status['succeeded'],status['failed'],status['unattempted']),(7,4,3,1,3))
        self.assertFalse((args.out/'grouped-scores.json').exists())

    def test_trace2_rejects_old_reader_and_bad_control_receipts(self):
        path=self.root/'trace.jsonl'; t=C['recorder'](H,path,'a'*64,dict(episodeId='e',groupId='g'),{})
        t.barrier(receipt()); t.close(); rows=H['rows'](path)
        self.assertEqual(C['reconcile'](H,rows,[])['barrierObservations'],'1')
        with self.assertRaisesRegex(ValueError,'trace1'): H['reconcile_accounting'](rows,[])
        for change in (lambda r:r['barrier'].update(pendingWork=1),lambda r:r['barrier'].update(storeId='foreign'),
                       lambda r:r['barrier']['derived'][0].update(appliedGeneration='1')):
            value=copy.deepcopy(rows);change(value[0]['observation']['receipt'])
            with self.assertRaises(ValueError):C['reconcile'](H,value,[])
        value=copy.deepcopy(rows);value[0]['eventKind']='future_unknown'
        with self.assertRaisesRegex(ValueError,'unknown'):C['reconcile'](H,value,[])

    def test_unmatched_duplicate_context_rejected_at_delivery(self):
        t=C['recorder'](H,self.root/'trace.jsonl','a'*64,dict(episodeId='e',groupId='g'),{})
        row=dict(event='context',tag='context:q0',stepId='1',deliveryOrdinal=1,context={})
        with self.assertRaisesRegex(ValueError,'unmatched'):t.consume(row)
        t.completed['1']=dict(tag='query:q0',deliveryOrdinal=1,response=dict(result={}))
        t.consume(row)
        with self.assertRaisesRegex(ValueError,'duplicate'):t.consume(row)
        t.close()

    def test_version_rejection_precedes_executables(self):
        for track,version in ((C['TRACK'],1),(H['TRACK'],2),(C['TRACK'],True)):
            with self.assertRaisesRegex(ValueError,'independent harness version'):
                H['validate_build'](dict(track=track,manifestVersion=1,sourceMappingVersion=1,traceSchemaVersion=version),None,None,None)

    def test_current_constructor_exact_capabilities_and_state_not_old_helper(self):
        control=C['control'](H,-1,-1,self.root/'never-written.jsonl')
        expected=H['SWEEP']['control_capabilities'](H['RESET']['CONTROL_CAPABILITIES'])
        control.call=lambda method,params,error=None:copy.deepcopy(expected) if method=='capabilities' else {}
        control.handshake()
        for value in (H['OWNED']['CAPABILITIES'],dict(expected,methods=expected['methods']+['future-method'])):
            control.call=lambda *a,**k:value
            with self.assertRaisesRegex(ValueError,'owned-constructor'):control.handshake()
        state=dict(committed=receipt()['target'],logicalHash=None,snapshot=False,
            runtimeState=dict(nowMillis=1000,allocation=dict(version=1,seed=[7]*32,counter=0,lastMemoryMillis=None)))
        control.call=lambda method,params:copy.deepcopy(state if method=='inspect' else receipt()['barrier'])
        value=control.checkpoint(-1,0)
        self.assertEqual(value['runtimeState'],state['runtimeState'])
        state['snapshot']=True
        with self.assertRaisesRegex(ValueError,'not a checkpoint'):control.checkpoint(0,1)

    def test_control_timeout_is_checker_only_and_no_automatic_retry(self):
        import os
        read,write=os.pipe(); request,peer=os.pipe()
        try:
            c=H['OWNED']['Control'](read,peer,self.root/'control.jsonl')
            with mock.patch('select.select',return_value=([],[],[])):
                with self.assertRaisesRegex(RuntimeError,'timeout'):c.call('barrier',dict(target={},deadlineMs=1))
            self.assertEqual(c.serial,1)
            self.assertEqual([v['event'] for v in H['rows'](self.root/'control.jsonl')],['attempt'])
        finally:
            for fd in (read,write,request,peer):os.close(fd)


if __name__=='__main__':unittest.main()
