#!/usr/bin/env python3
"""Offline installed-reference and consumer-boundary checks; synthetic judgments only."""
import copy
import math
from pathlib import Path
import runpy
import tempfile
from types import SimpleNamespace
import unittest

h = runpy.run_path(str(Path(__file__).with_name('public-mcp-evaluation.py')))
b = h['BEIR']
score = b['score_reference']


class ReferenceTests(unittest.TestCase):
    def test_perfect_and_swapped_linear_grades(self):
        q = {'q': {'a': 3, 'b': 1}}
        perfect = score(q, {'q': {'a': 100., 'b': 99.}}, ['q'])
        swapped = score(q, {'q': {'b': 100., 'a': 99.}}, ['q'])
        self.assertEqual(perfect['perQuery']['q']['ndcg_cut_10'], 1.)
        self.assertAlmostEqual(swapped['perQuery']['q']['ndcg_cut_10'], (1 + 3/math.log2(3))/(3 + 1/math.log2(3)))
        self.assertEqual(swapped['perQuery']['q']['map_cut_100'], 1.)

    def test_empty_zero_and_complete_denominator(self):
        for qrels, runs in [({'q': {'a': 3}}, {'q': {}}), ({'q': {'a': 0}}, {'q': {'a': 1.}})]:
            value = score(qrels, runs, ['q'])
            self.assertEqual(list(value['perQuery']), ['q'])
            self.assertTrue(all(v == 0 for v in value['perQuery']['q'].values()))
            self.assertEqual(value['official']['mrr']['MRR@10'], 0.)
        value = score({'q': {'a': 1}, 'empty': {'b': 1}}, {'q': {'a': 1.}, 'empty': {}}, ['q', 'empty'])
        self.assertEqual(value['official']['ndcg']['NDCG@10'], .5)
        self.assertEqual(value['official']['mrr']['MRR@10'], .5)
        self.assertEqual(value['officialDenominator'], '2')

    def test_unjudged_retains_rank(self):
        value = score({'q': {'a': 3, 'b': 1}}, {'q': {'unjudged': 2., 'b': 1.}}, ['q'])
        raw = value['perQuery']['q']
        self.assertEqual(raw['recall_5'], .5)
        self.assertEqual(raw['map_cut_100'], .25)
        self.assertAlmostEqual(raw['ndcg_cut_10'], (1/math.log2(3))/(3 + 1/math.log2(3)))
        self.assertEqual(value['official']['mrr']['MRR@10'], .5)

    def test_unknown_and_all_unknown(self):
        value = score({'q': {'a': 1}}, {'q': {}, 'unknown': {}}, ['q', 'unknown'])
        self.assertEqual(value['unknownQueries'], ['unknown'])
        self.assertEqual(value['officialDenominator'], '1')
        self.assertIsNone(score({}, {'unknown': {}}, ['unknown'])['official'])

    def test_missing_duplicate_invalid_rosters_and_grades(self):
        for q, run, roster in [({'q': {'a': 1}}, {}, ['q']), ({}, {'q': {}}, ['q', 'q']),
                ({'absent': {'a': 1}}, {'q': {}}, ['q']), ({'q': {}}, {'q': {}}, ['q']),
                ({'q': {'q': 1}}, {'q': {}}, ['q']), ({'q': {'a': True}}, {'q': {}}, ['q']),
                ({'q': {'a': -1}}, {'q': {}}, ['q']), ({'q': {'a': 4}}, {'q': {}}, ['q']),
                ({'q': {'a': 1}}, {'q': {'a': float('nan')}}, ['q'])]:
            with self.subTest(q=q, roster=roster), self.assertRaises(ValueError): score(q, run, roster)

    def test_reference_ties_are_distinct_contracts(self):
        a = score({'q': {'a': 1}}, {'q': {'a': 1., 'z': 1.}}, ['q'])
        z = score({'q': {'a': 1}}, {'q': {'z': 1., 'a': 1.}}, ['q'])
        self.assertEqual(a['perQuery']['q']['map_cut_100'], .5)
        self.assertEqual(z['perQuery']['q']['map_cut_100'], .5)
        self.assertEqual(a['official']['mrr']['MRR@10'], 1.)
        self.assertEqual(z['official']['mrr']['MRR@10'], .5)

    def test_121_to_100_reference_denominator(self):
        q = {'q': {f'd{i}': 1 for i in range(121)}}
        value = score(q, {'q': {f'd{i}': float(100-i) for i in range(100)}}, ['q'])['perQuery']['q']
        self.assertAlmostEqual(value['map_cut_100'], 100/121)
        for k in (5, 10, 20): self.assertAlmostEqual(value[f'recall_{k}'], k/121)

    def test_fixture_projection_and_grade_boundary(self):
        actor, labels, _, schedule = b['fixtures'](h)
        b['validate_labels'](labels, actor, schedule)
        self.assertEqual(sum(s['namespace']=='default' for s in actor['sources']), 121)
        for mutate in ('foreign', 'bool', 'projection', 'empty'):
            bad = copy.deepcopy(labels)
            if mutate == 'foreign': bad['metricQrels']['q0']['doc121']=1
            elif mutate == 'bool': bad['metricQrels']['q0']['doc000']=True
            elif mutate == 'empty': bad['metricQrels']['q2']={}
            else: bad['relevant']['q0'].pop()
            with self.subTest(mutate=mutate), self.assertRaises(ValueError): b['validate_labels'](bad, actor, schedule)

    def test_actual_boundary_rejects_incomplete_failed_short_duplicate_foreign(self):
        actor, labels, _, schedule = b['fixtures'](h)
        mappings = {f'id{i}': [s['sourceId']] for i,s in enumerate(actor['sources'])}
        wire=[]
        for step in schedule['steps'][:4]:
            req=dict(params=dict(name=step['tool'], arguments=step['arguments']))
            response=dict(result=dict(content=[dict(text=h['encoded'](dict(count=100, memories=[dict(id=f'id{i}') for i in range(100)])).decode())]))
            wire.extend([dict(event='attempt',tag=step['tag'],request=req), dict(event='outcome',tag=step['tag'],request=req,response=response)])
        for mutation in ('missing', 'duplicate', 'failed', 'short', 'duplicate-id', 'foreign', 'mapping'):
            bad=copy.deepcopy(wire); mp=copy.deepcopy(mappings)
            if mutation=='missing': bad.pop()
            elif mutation=='duplicate': bad.append(bad[-1])
            elif mutation=='failed': bad[1]['response']={'result': {'isError': True, 'content': [{'text': 'actual tool error'}]}}
            elif mutation=='mapping': mp['id0']=['doc001']
            else:
                import json
                value=json.loads(bad[1]['response']['result']['content'][0]['text'])
                if mutation=='short': value['memories'].pop(); value['count']=99
                elif mutation=='duplicate-id': value['memories'][-1]['id']='id0'
                else: value['memories'][-1]['id']='id121'
                bad[1]['response']['result']['content'][0]['text']=json.dumps(value)
            with tempfile.TemporaryDirectory() as directory:
                out=Path(directory); (out/'evaluator').mkdir()
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    b['consume'](SimpleNamespace(out=out), h, {}, actor, labels, schedule, bad, {'mappings':mp})
                self.assertFalse((out/'evaluator/beir-score-input.json').exists())


if __name__=='__main__': unittest.main()
