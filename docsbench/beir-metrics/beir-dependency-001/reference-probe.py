import sys,json,math,copy,hashlib
from pathlib import Path
import numpy,pytrec_eval,pytrec_eval_ext,tqdm,beir
from beir.retrieval.evaluation import EvaluateRetrieval
from beir.retrieval.custom_metrics import mrr
out=Path('/output');rows=[]
for m in [numpy,pytrec_eval,pytrec_eval_ext,tqdm,beir]:assert Path(m.__file__).resolve().is_relative_to('/output/site')
def evaluate(name,qrels,run):
 before=copy.deepcopy(run);raw=pytrec_eval.RelevanceEvaluator(qrels,{'ndcg_cut.10','map_cut.100','recall.5,10,20'}).evaluate(run)
 official=EvaluateRetrieval.evaluate(qrels,run,[5,10,20,100],ignore_identical_ids=False);reciprocal=EvaluateRetrieval.evaluate_custom(qrels,run,[10],metric='mrr');assert run==before
 row=dict(name=name,qrels=qrels,run=run,raw=raw,official=official,mrr=reciprocal);rows.append(row);(out/'reference-cases.json').write_text(json.dumps(rows,indent=2)+'\n');return row
r=evaluate('known-empty',{'q':{'a':3,'b':1}},{'q':{}});assert set(r['raw'])=={'q'} and all(v==0 for v in r['raw']['q'].values()) and r['mrr']['MRR@10']==0
r=evaluate('known-zero',{'q':{'a':0}},{'q':{'a':100.0}});assert set(r['raw'])=={'q'} and all(v==0 for v in r['raw']['q'].values()) and r['mrr']['MRR@10']==0
r=evaluate('perfect',{'q':{'a':3,'b':1}},{'q':{'a':100.0,'b':99.0}});assert r['raw']['q']['ndcg_cut_10']==1 and r['raw']['q']['map_cut_100']==1
r=evaluate('swapped-graded',{'q':{'a':3,'b':1}},{'q':{'b':100.0,'a':99.0}});assert abs(r['raw']['q']['ndcg_cut_10']-(1+3/math.log2(3))/(3+1/math.log2(3)))<1e-12 and r['raw']['q']['map_cut_100']==1
r=evaluate('unjudged-partial',{'q':{'a':3,'b':1}},{'q':{'u':100.0,'b':99.0}});assert abs(r['raw']['q']['ndcg_cut_10']-(1/math.log2(3))/(3+1/math.log2(3)))<1e-12 and r['raw']['q']['map_cut_100']==.25 and r['raw']['q']['recall_5']==.5 and r['mrr']['MRR@10']==.5
r=evaluate('complete-empty-mixed',{'q':{'a':1},'empty':{'a':1}},{'q':{'a':100.0},'empty':{}});assert set(r['raw'])=={'q','empty'} and r['official'][0]['NDCG@10']==.5 and r['mrr']['MRR@10']==.5
r=evaluate('missing-query-characterization',{'q':{'a':1},'missing':{'a':1}},{'q':{'a':100.0}});assert set(r['raw'])=={'q'} and r['official'][0]['NDCG@10']==1 and r['mrr']['MRR@10']==.5
raw=pytrec_eval.RelevanceEvaluator({'q':{'a':1}},{'recall.5'}).evaluate({'q':{'a':1.0},'unknown':{'a':1.0}});assert set(raw)=={'q'}
for name,run in [('tie-a-first',{'q':{'a':1.0,'z':1.0}}),('tie-z-first',{'q':{'z':1.0,'a':1.0}})]:
 r=evaluate(name,{'q':{'a':1,'z':0}},run);assert r['raw']['q']['map_cut_100']==.5
assert rows[-2]['mrr']['MRR@10']==1 and rows[-1]['mrr']['MRR@10']==.5
qrels={'q':{f'd{i:03d}':1 for i in range(121)}};run={'q':{f'd{i:03d}':float(100-i) for i in range(100)}};r=evaluate('reference-only-121-to100',qrels,run);assert abs(r['raw']['q']['map_cut_100']-100/121)<1e-12 and abs(r['raw']['q']['recall_20']-20/121)<1e-12
(out/'reference-summary.json').write_text(json.dumps(dict(cases=len(rows),unknownSkipped=list(raw),versions=dict(numpy=numpy.__version__,tqdm=tqdm.__version__,pytrec_eval='0.5'),scope='Installed official reference contract; not actual public100 proof',allPassed=True),indent=2)+'\n')
print((out/'reference-summary.json').read_text())
