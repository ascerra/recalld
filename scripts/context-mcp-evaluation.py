"""Context track hooks for the existing public actor and evaluator."""
import copy
import json
from pathlib import Path
import runpy
import subprocess

TOKEN=runpy.run_path(str(Path(__file__).with_name('context_tokenizer.py')))
TRACK='serialized-mcp-tokenized-context-synthetic-diagnostic-v1'
PROFILES={'default':('default','standard'),'boundary':('boundary','standard'),'oversized-first':('boundary','oversized-first'),'empty':('boundary','empty')}


def fixtures(h,kind):
    if kind not in ('standard','oversized-first','empty'):raise ValueError('unsupported context fixture')
    actor,labels,cache,schedule=h['fixtures']()
    if kind=='empty':
        actor['sources']=[];labels['relevant']={'q0':[],'q1':[],'q3':[]}
        cache['entries']={k:v for k,v in cache['entries'].items() if k.startswith('a6-query:') or k=='a6-document:health check'}
    else:
        long='cedar résumé 🌲 '+('cedar river ' * 300)
        vectors=([1.,0.,0.,0.],[.8,.6,0.,0.],[.6,.8,0.,0.],[1.,0.,0.,0.])
        for index,source in enumerate(actor['sources']):
            del cache['entries']['a6-document:'+source['summary']]
            if index==(0 if kind=='oversized-first' else 1):source['summary']=long
            source['contentSha256']=h['digest'](source['summary'].encode())
            cache['entries']['a6-document:'+source['summary']]=vectors[index]
        labels['relevant']['q0']=['turn-0','turn-1'];labels['relevant']['q3']=['turn-1','turn-3']
    return actor,labels,cache,schedule


def preflight(manifest,schedule,bundle):
    TOKEN['validate_policy'](manifest['contextPolicy'])
    if manifest['contextFixture'] not in ('standard','oversized-first','empty'):raise ValueError('unsupported context fixture')
    if bundle is None:raise ValueError('context assets required')
    encoder=TOKEN['load'](bundle)
    for q in [*schedule['queries'],dict(text='unrecorded-query')]:TOKEN['overhead'](encoder,q['text'],manifest['contextPolicy'])


def consume(args,h,m,schedule,wire,score_input):
    encoder=TOKEN['load'](args.context_assets);questions={q['queryId']:q['text'] for q in schedule['queries']}
    public={}
    for row in wire:
        if row['event']=='outcome' and row['tag'].startswith('query:'):
            q=row['tag'][6:]
            if q in public:raise ValueError('duplicate public context outcome')
            public[q]=json.loads(row['response']['result']['content'][0]['text'])['memories']
    receipts={}
    for row in wire:
        if row['event']!='context':continue
        q=row['tag'][8:]
        if q in receipts or q not in questions or q not in public:raise ValueError('unknown/duplicate context outcome')
        receipts[q]=TOKEN['verify'](encoder,questions[q],public[q],m['contextPolicy'],row['context'])
    if set(receipts)!=set(questions) or set(public)!=set(questions):raise ValueError('missing context query outcome')
    # Existing scorer consumes included IDs from verified actual renders, not from labels.
    packed=copy.deepcopy(score_input);packed['queries']=[dict(queryId=q,memoryIds=receipts[q]['includedMemoryIds']) for q in sorted(questions)]
    dest=args.out/'evaluator';h['save'](dest/'packed-context-score-input.json',packed)
    subprocess.run([str(args.scorer),'--input',str(dest/'packed-context-score-input.json'),'--out',str(dest/'packed-context-scores.json')],check=True)
    h['save'](dest/'context-validation.json',dict(contextVersion=1,publicPayloadVerified=True,queries=receipts,metricBoundary='packed-context-evidence',
        originalRetrievalScores='scores.json',modelTokenIdentity=None,answerQuality=None,cost=None))
