#!/usr/bin/env python3
"""Focused actual-tokenizer/actor boundary checks; no live provider/model calls."""
import argparse
import copy
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import context_tokenizer as t
import test_public_mcp_evaluation as legacy

parser=argparse.ArgumentParser(add_help=False);parser.add_argument('--assets',type=Path,required=True)
ASSETS,REST=parser.parse_known_args();sys.argv=[sys.argv[0],*REST]
h=legacy.m;c=h['CONTEXT']


class Tokenizer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.encoder=t.load(ASSETS.assets)

    def test_real_encoding_unicode_special_literals_and_invalid_utf8(self):
        for value in ('résumé e\u0301 🌲','  a\n b\t','<|endoftext|>',''):
            ids=t.tokens(self.encoder,value);self.assertEqual(self.encoder.decode(ids),value)
        self.assertNotIn(100257,t.tokens(self.encoder,'<|endoftext|>'))
        with self.assertRaises(UnicodeEncodeError):t.tokens(self.encoder,'\ud800')

    def test_ranked_prefix_omission_no_skip_ahead(self):
        rows=[dict(id='first',summary='amber'),dict(id='too-long',summary='cedar ' * 400),dict(id='later-short',summary='birch')]
        v=t.assemble(self.encoder,'amber',rows,t.policy('boundary'))
        self.assertEqual(v['includedMemoryIds'],['first']);self.assertEqual(v['omittedMemoryIds'],['too-long','later-short'])
        self.assertEqual(v['entries'][-1]['reason'],'after-first-nonfitting-entry')
        self.assertEqual(v['fullTokenCount'],len(self.encoder.encode_ordinary(v['renderedText'])))
        self.assertLessEqual(v['evidenceTokenCount'],128);self.assertLessEqual(v['fullTokenCount'],256)
        self.assertFalse(v['countsAreAdditive'])
        self.assertEqual(t.assemble(self.encoder,'amber',rows,t.policy('default'))['includedMemoryIds'],[v['id'] for v in rows])

    def test_actual_rendered_payload_reconstruction_rejects_self_consistent_forgery(self):
        rows=[dict(id='public-id',summary='actual public summary')];configuration=t.policy('default')
        actual=t.assemble(self.encoder,'question',rows,configuration)
        self.assertEqual(t.verify(self.encoder,'question',rows,configuration,actual),actual)
        forged=t.assemble(self.encoder,'question',[dict(id='public-id',summary='fabricated evidence')],configuration)
        with self.assertRaisesRegex(ValueError,'actual public payload'):t.verify(self.encoder,'question',rows,configuration,forged)
        forged=copy.deepcopy(actual);forged['renderedText']+='hidden suffix'
        with self.assertRaises(ValueError):t.verify(self.encoder,'question',rows,configuration,forged)

    def test_empty_oversized_first_and_duplicate_public_identity(self):
        configuration=t.policy('boundary')
        self.assertEqual(t.assemble(self.encoder,'question',[],configuration)['evidenceTokenCount'],0)
        value=t.assemble(self.encoder,'question',[dict(id='first',summary='cedar '*400),dict(id='second',summary='x')],configuration)
        self.assertEqual(value['includedMemoryIds'],[]);self.assertEqual(len(value['omittedMemoryIds']),2)
        with self.assertRaisesRegex(ValueError,'duplicate public memory'):t.assemble(self.encoder,'q',[dict(id='same',summary='a'),dict(id='same',summary='b')],configuration)

    def test_exact_full_cap_and_one_over_reject_before_actor_io(self):
        configuration=t.policy('boundary');question=next('word '*n for n in range(300) if len(t.tokens(self.encoder,t.render('word '*n,'')))==256)
        self.assertEqual(t.assemble(self.encoder,question,[],configuration)['fullTokenCount'],256)
        with self.assertRaisesRegex(ValueError,'overhead'):t.overhead(self.encoder,question+'word ',configuration)
        projection=dict(phase='query',startId=0,deliveryOrdinal=1,currentQuestion=dict(queryId='q0',text=question+'word '),expectedMiss=False,contextPolicy=configuration)
        result=subprocess.run([sys.executable,str(Path(__file__).with_name('public-mcp-actor.py')),'--context-bundle',str(ASSETS.assets),'--read-fd','-1','--write-fd','-1','--deny-fd','-1'],input=t.encoded(projection),capture_output=True,env=dict(PATH='/usr/bin:/bin',PYTHONDONTWRITEBYTECODE='1',PYTHONNOUSERSITE='1'))
        self.assertNotEqual(result.returncode,0);self.assertIn(b'overhead exceeds',result.stderr);self.assertEqual(result.stdout,b'')

    def test_missing_corrupt_extension_rank_and_config_fail_before_import(self):
        with tempfile.TemporaryDirectory() as tmp:
            bundle=Path(tmp)/'bundle';shutil.copytree(ASSETS.assets,bundle)
            for name in ['cl100k_base.tiktoken','context-encoding.json','site/tiktoken/_tiktoken.cpython-311-aarch64-linux-gnu.so']:
                path=bundle/name;raw=path.read_bytes();path.write_bytes(raw+b'corrupt')
                with self.assertRaisesRegex(ValueError,'hash mismatch'):t.verify_bundle(bundle)
                path.write_bytes(raw)
            path=bundle/'cl100k_base.tiktoken';path.unlink()
            with self.assertRaisesRegex(ValueError,'asset/config'):t.verify_bundle(bundle)


class ContextPreflight(unittest.TestCase):
    def setUp(self):
        self.old=legacy.Preflight();self.old.setUp();self.addCleanup(self.old.doCleanups);self.root=self.old.root
        self.m=copy.deepcopy(self.old.manifest);self.m.update(track=c['TRACK'],contextPolicy=t.policy('boundary'),contextFixture='standard')
        artifacts=c['fixtures'](h,'standard')
        for key,value in zip(('actorInputs','evaluatorLabels','replayCache','evaluatorSchedule'),artifacts):
            path=self.root/self.m[key]['file'];h['save'](path,value);self.m[key]['sha256']=h['digest'](path.read_bytes())
        self.m['dataset']['actorArtifactSha256']=self.m['actorInputs']['sha256']
        self.identity=dict(activeEmbedding=self.old.profile,sourceCommit=self.m['sourceCommit'],containerImageId=self.m['containerImageId'],sourceTreeSha256=h['digest'](h['encoded'](self.m['sourceFilesSha256'])))

    def validate(self):
        h['save'](self.root/'manifest.json',self.m)
        with mock.patch('subprocess.check_output',return_value=json.dumps(self.identity).encode()):return h['validate'](self.root/'manifest.json',self.old.binary,self.old.scorer,self.root,ASSETS.assets)

    def test_exact_policy_asset_identity_and_old_boundary(self):
        self.validate();original=copy.deepcopy(self.m)
        for change in (lambda v:v['contextPolicy'].update(contextVersion=True),lambda v:v['contextPolicy'].update(evidenceTokenCap=129),lambda v:v['contextPolicy']['tokenizer'].update(version='unknown'),lambda v:v['contextPolicy']['tokenizer'].update(rankSha256='0'*64),lambda v:v.update(contextFixture='unknown')):
            self.m=copy.deepcopy(original);change(self.m)
            with self.assertRaises(ValueError):self.validate()
        self.assertEqual(self.old.validate()[0]['track'],h['TRACK'])
        old=copy.deepcopy(self.old.manifest);old['contextPolicy']=t.policy('default')
        with self.assertRaises(ValueError):self.old.validate(old)

    def test_label_change_cannot_change_current_actor_or_cache(self):
        before=self.validate();labels=json.loads((self.root/'evaluator-labels.json').read_text());labels['relevant']={'q0':['turn-3']};labels['labelCanary']='CHANGED_CONTEXT_LABEL_CANARY';h['save'](self.root/'evaluator-labels.json',labels);self.m['evaluatorLabels']['sha256']=h['digest']((self.root/'evaluator-labels.json').read_bytes());after=self.validate()
        self.assertEqual(before[1],after[1]);self.assertEqual(before[3:5],after[3:5]);self.assertNotEqual(before[-1],after[-1])
        source=Path(__file__).with_name('context_tokenizer.py').read_text()
        self.assertNotIn('fixtures',source);self.assertNotIn('evaluator',source);self.assertNotIn('FUTURE_QUESTION_CANARY',source)


if __name__=='__main__':unittest.main()
