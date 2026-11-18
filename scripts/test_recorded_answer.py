#!/usr/bin/env python3
"""Focused parser/provenance/preflight checks; no runtime/model claims."""
import copy
import json
from pathlib import Path
import runpy
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

H=runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))
A=H['ANSWER'];R=A['ROLE'];C=H['COMBINED']


def envelope(variant='baseline'):
    entry=A['cache']('episode-0',variant)['entries'][0]
    return dict(role='ANSWER',roleConfig=A['policy']()['configuration'],**{k:v for k,v in entry.items() if k!='queryId'})


class Recorded(unittest.TestCase):
    def test_actual_frozen_outputs_and_actor_parser(self):
        frozen=A['fixture']();self.assertEqual(len(frozen['records']),28)
        for i in range(7):
            entries=A['cache'](f'episode-{i}','baseline')['entries'];self.assertEqual(len(entries),4)
            for entry in entries:
                env=dict(role='ANSWER',roleConfig=A['policy']()['configuration'],**{k:v for k,v in entry.items() if k!='queryId'})
                self.assertEqual(R['consume'](env)['status'],'accepted')

    def test_changed_input_config_role_output_fails_closed(self):
        for mutation in (
            lambda e:e.update(role='JUDGE'),
            lambda e:e['input']['currentQuestion'].update(text='different'),
            lambda e:e['roleConfig'].update(retrievalLimit=1),
            lambda e:e.update(outputSha256='0'*64),
            lambda e:e.update(futureQuestions=[])):
            value=envelope();mutation(value)
            with self.assertRaises(ValueError):R['consume'](value)

    def test_refusal_parse_utf8_are_distinct_not_success(self):
        self.assertEqual(R['consume'](envelope('refusal'))['reason'],'recorded-refusal')
        for variant in ('parse','utf8'):
            self.assertEqual(R['consume'](envelope(variant))['reason'],'output-parse')
        for raw in (b'{"x":1,"x":2}',b'{"x":NaN}',b'{"x":"\\ud800"}'):
            with self.assertRaises((ValueError,UnicodeError)):R['decode'](raw)

    def test_citation_checked_against_actual_included_payload(self):
        env=envelope();result=R['consume'](env)['output'];citation=result['citations'][0]
        public=[dict(id=citation['memoryId'],summary=citation['quote'])]
        R['verify_citations'](result,public,env['input']['context'])
        for changed in ([],[dict(id=citation['memoryId'],summary='self-consistent unrelated content')]):
            with self.assertRaises(ValueError):R['verify_citations'](result,changed,env['input']['context'])
        bad=R['consume'](envelope('citation'))['output']
        with self.assertRaises(ValueError):R['verify_citations'](bad,public,env['input']['context'])

    def setup_manifest(self,variant='baseline'):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);root=Path(tmp.name)
        binary=root/'binary';binary.write_bytes(b'not-executable')
        build=root/'build.json'
        runtime={str(p.relative_to(H['ROOT'])):H['digest'](p.read_bytes()) for p in (H['ROOT']/'src').rglob('*.rs')}
        runtime.update({name:H['digest']((H['ROOT']/name).read_bytes()) for name in ('Cargo.toml','Cargo.lock')})
        H['save'](build,dict(sourceCommit='a'*40,sourceFilesSha256=runtime,containerImageId='sha256:'+'b'*64))
        args=SimpleNamespace(track=A['TRACK'],binary=binary,scorer=binary,build=build,out=root/'inputs',
            context_profile='default',context_assets=root/'assets',combined_variant='baseline',answer_variant=variant,
            source_root=H['ROOT'],manifest=root/'inputs/manifest.json')
        h=dict(H);h['CONTEXT']=dict(H['CONTEXT']);h['CONTEXT']['TOKEN']=dict(H['CONTEXT']['TOKEN'],load=lambda _:None)
        with mock.patch('subprocess.check_output',return_value=H['encoded'](dict(activeEmbedding=A['policy']()['configuration']['activeEmbedding']))):
            C['prepare'](args,h,answer=True)
        m=json.loads(args.manifest.read_text());m['executables']['ownedEvaluator']=A['policy']()['configuration']['ownedEvaluatorSha256'];H['save'](args.manifest,m)
        h['validate_build']=lambda *a:None;h['CONTEXT']['preflight']=lambda *a:None
        return args,h

    def test_whole_roster_and_missing_entry_is_declared(self):
        args,h=self.setup_manifest('middle-miss');m,bundles,roster,_=C['validate'](args,h,answer=True)
        self.assertEqual((len(bundles),sum(len(r['queryIds']) for r in roster)),(7,28))
        self.assertEqual([len(b[0]['answerCache']['entries']) for b in bundles],[4,4,4,3,4,4,4])

    def test_last_member_cache_preflight_without_output(self):
        args,h=self.setup_manifest();m=json.loads(args.manifest.read_text())
        ref=m['episodes'][-1]['roleCache'];path=args.manifest.parent/ref['file']
        value=json.loads(path.read_text());value['entries'][0]['outputSha256']='0'*64
        H['save'](path,value);ref['sha256']=H['digest'](path.read_bytes());H['save'](args.manifest,m)
        args.out=args.out.parent/'forbidden'
        with self.assertRaisesRegex(ValueError,'frozen'):C['validate'](args,h,answer=True)
        self.assertFalse(args.out.exists())

    def test_policy_change_cannot_rekey_frozen_output(self):
        args,h=self.setup_manifest();m=json.loads(args.manifest.read_text());m['answerPolicy']['configuration']['parser']='new'
        H['save'](args.manifest,m)
        with self.assertRaisesRegex(ValueError,'policy'):C['validate'](args,h,answer=True)
        with self.assertRaises(ValueError):C['validate'](args,h) # Closed old consumer.

    def test_role_reconcile_complete_identity_and_correlation(self):
        args,h=self.setup_manifest();_,bundles,_,_=C['validate'](args,h,answer=True)
        m=bundles[0][0];environment=bundles[0][-1];role=[];trace=[]
        for i,entry in enumerate(m['answerCache']['entries']):
            context=entry['input']['context'];step=str(20+i);ordinal=i+1
            trace.append(dict(eventKind='context',stepId=step,deliveryOrdinal=ordinal,observation=context))
            base=dict(roleTraceSchemaVersion=1,runId=environment,environmentHash=environment,episodeId='episode-0',groupId=m['groupContext']['groupId'],
                queryId=entry['queryId'],deliveryOrdinal=ordinal,publicStepId=step,role='ANSWER',roleConfigSha256=m['answerCache']['roleConfigSha256'],
                inputSha256=entry['inputSha256'],key=entry['key'],contextSha256=H['digest'](H['encoded'](context)),cacheArtifactSha256=m['roleCache']['sha256'])
            env=dict(role='ANSWER',roleConfig=m['answerPolicy']['configuration'],**{k:v for k,v in entry.items() if k!='queryId'})
            role.extend([dict(base,sequence=str(2*i+1),eventKind='attempt'),dict(base,sequence=str(2*i+2),eventKind='accepted',cacheStatus='hit',outputSha256=entry['outputSha256'],
                outputBytes=str(len(bytes.fromhex(entry['outputHex']))),elapsedSeconds=0.1,outcome=R['consume'](env))])
        self.assertEqual(A['reconcile'](m,role,trace,environment)['accepted'],4)
        for field,value in [('environmentHash','bad'),('queryId','unknown'),('publicStepId','wrong'),('cacheArtifactSha256','bad'),('eventKind','rejected')]:
            changed=copy.deepcopy(role);changed[-1][field]=value
            with self.assertRaises(ValueError):A['reconcile'](m,changed,trace,environment)
        with self.assertRaises(ValueError):A['reconcile'](m,role[:-2],trace,environment)

    def test_labels_schedule_not_in_role_fixture(self):
        self.assertEqual(A['cache']('episode-0','baseline'),A['cache']('episode-0','gold'))
        self.assertEqual(A['cache']('episode-0','baseline'),A['cache']('episode-0','future'))
        source=Path(__file__).with_name('recorded_answer.py').read_text()
        self.assertNotIn('fixtures',source.replace('No fixtures',''))
        self.assertNotIn('EVALUATOR_LABEL_CANARY',json.dumps(A['cache']('episode-0','baseline')))


if __name__=='__main__':unittest.main()
