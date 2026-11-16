#!/usr/bin/env python3
"""Paired preflight and failure ledger checks; subprocess faults here are injected."""
import copy
import json
from types import SimpleNamespace
import unittest
from unittest import mock
import test_public_mcp_evaluation as legacy

h = legacy.m
p = h['PAIRED']


class PairedPreflight(unittest.TestCase):
    def setUp(self):
        self.old = legacy.Preflight(); self.old.setUp(); self.addCleanup(self.old.doCleanups)
        self.root = self.old.root
        self.identity = dict(activeEmbedding=self.old.profile, sourceCommit=self.old.manifest['sourceCommit'],
            containerImageId=self.old.manifest['containerImageId'], sourceTreeSha256=h['digest'](h['encoded'](self.old.manifest['sourceFilesSha256'])))
        h['save'](self.root/'build.json', {k:self.old.manifest[k] for k in ('sourceCommit','sourceFilesSha256','containerImageId')})
        args = SimpleNamespace(binary=self.old.binary,scorer=self.old.scorer,build=self.root/'build.json',out=self.root/'inputs')
        with mock.patch('subprocess.check_output',return_value=json.dumps(self.identity).encode()): p['prepare'](args,h)
        self.args = SimpleNamespace(binary=args.binary,scorer=args.scorer,source_root=self.root,manifest=args.out/'manifest.json',out=self.root/'run')
        self.manifest = json.loads(self.args.manifest.read_text())

    def validate(self):
        h['save'](self.args.manifest,self.manifest)
        with mock.patch('subprocess.check_output',return_value=json.dumps(self.identity).encode()): return p['validate'](self.args,h)

    def change_arm(self, role, mutate):
        ref = next(e['manifest'] for e in self.manifest['arms'] if e['role']==role)
        root = self.args.manifest.parent; path = root/ref['file']; arm = json.loads(path.read_text())
        mutate(arm,path.parent); h['save'](path,arm); ref['sha256']=h['digest'](path.read_bytes())

    def artifact(self, arm, root, key, mutate):
        ref=arm['episodes'][-1]['artifacts'][key]; path=root/ref['file']; value=json.loads(path.read_text())
        mutate(value); h['save'](path,value);ref['sha256']=h['digest'](path.read_bytes())

    def test_complete_pair_frozen_shared_labels_and_source_only_groups(self):
        m, arms, env = self.validate()
        self.assertEqual(set(arms), {'baseline','candidate'})
        self.assertEqual(arms['baseline'][1][2],arms['candidate'][1][2])
        self.assertEqual(len(arms['baseline'][1][2]),7)
        self.assertEqual(m['uncertaintyPolicy']['streams'],dict(train=0,development=1,test=2))
        for left,right in zip(arms['baseline'][1][1],arms['candidate'][1][1]):
            self.assertEqual(left[1:5],right[1:5])
            self.assertNotIn('relevant',left[1]);self.assertNotIn('queries',left[1])
        source=h['GROUPED']['registry'](h)
        original=h['GROUPED']['maps'](source,h)
        changed=copy.deepcopy(source);changed['episodes'].reverse()
        self.assertEqual(original,h['GROUPED']['maps'](changed,h))
        self.assertFalse(self.args.out.exists())

    def test_last_candidate_corruption_rejects_before_first_episode(self):
        self.change_arm('candidate', lambda a,r:self.artifact(a,r,'replayCache',lambda v:v['entries'].update(extra=[1.,0.,0.,0.])))
        h['save'](self.args.manifest,self.manifest)
        consumer=mock.Mock(side_effect=AssertionError('episode must not start'))
        with mock.patch.dict(h,run_episode=consumer),mock.patch('subprocess.check_output',return_value=json.dumps(self.identity).encode()):
            with self.assertRaisesRegex(ValueError,'unsupported synthetic actor/cache fixture'):p['run'](self.args,h)
        consumer.assert_not_called();self.assertFalse(self.args.out.exists())

    def test_rehashed_label_difference_is_not_valid_arm_contrast(self):
        self.change_arm('candidate',lambda a,r:self.artifact(a,r,'evaluatorLabels',lambda v:v.update(relevant={'q0':['turn-3']})))
        with self.assertRaisesRegex(ValueError,'paired shared source/labels/query/cache differs'):self.validate()
        self.assertFalse(self.args.out.exists())

    def test_strict_versions_rng_and_complete_roster(self):
        original=copy.deepcopy(self.manifest)
        for change in (lambda m:m.update(pairVersion=True),lambda m:m['uncertaintyPolicy'].update(seed=8),
                       lambda m:m['uncertaintyPolicy']['streams'].update(train=1),lambda m:m['arms'].pop(),
                       lambda m:m['arms'].__setitem__(1,m['arms'][0]),lambda m:m.update(track=h['GROUPED']['TRACK']),
                       lambda m:m['capabilities'].update(deterministicRng=True)):
            self.manifest=copy.deepcopy(original);change(self.manifest)
            with self.assertRaises(ValueError):self.validate()
            self.assertFalse(self.args.out.exists())

    def test_role_limit_cannot_be_changed_even_with_fresh_hash(self):
        self.change_arm('candidate',lambda a,r:a['pairedArm'].update(retrievalLimit=1))
        with self.assertRaisesRegex(ValueError,'paired arm policy/context mismatch'):self.validate()

    def test_contract_edit_and_rehash_still_reject(self):
        ref=self.manifest['pairContract'];path=self.args.manifest.parent/ref['file'];value=json.loads(path.read_text())
        value['uncertaintyPolicy']['seed']=8;h['save'](path,value);ref['sha256']=h['digest'](path.read_bytes())
        with self.assertRaisesRegex(ValueError,'paired contract differs'):self.validate()

    def test_failed_arm_retains_all_fourteen_members_without_intersection(self):
        consumer=mock.Mock(side_effect=RuntimeError('injected first episode failure'))
        with mock.patch.dict(h,run_episode=consumer),mock.patch('subprocess.check_output',return_value=json.dumps(self.identity).encode()):
            with self.assertRaisesRegex(RuntimeError,'injected first episode failure'):p['run'](self.args,h)
        status=json.loads((self.args.out/'pair-status.json').read_text())
        self.assertEqual((status['plannedMembers'],status['attemptedMembers'],status['passedMembers'],status['failedMembers'],status['unattemptedMembers']),(14,1,0,1,13))
        self.assertEqual(status['arms'],dict(baseline=dict(status='failed'),candidate=dict(status='unattempted')))
        self.assertEqual(len(status['members']),14);self.assertFalse((self.args.out/'paired-scores.json').exists())

    def test_old_profile_still_valid_and_paired_fields_fail_closed(self):
        self.assertEqual(self.old.validate()[0]['track'],h['TRACK'])
        value=copy.deepcopy(self.old.manifest);value['pairVersion']=1
        with self.assertRaises(ValueError):self.old.validate(value)


if __name__=='__main__':unittest.main()
