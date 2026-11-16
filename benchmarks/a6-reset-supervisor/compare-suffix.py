import json,sys,hashlib
from pathlib import Path
left,right=map(Path,sys.argv[1:3]);a=json.loads(left.read_text());b=json.loads(right.read_text());differences=[]
def walk(a,b,path=''):
 if type(a)!=type(b):differences.append(dict(path=path,before=a,after=b));return
 if isinstance(a,dict):
  for k in sorted(a.keys()|b.keys()):
   if k not in a or k not in b:differences.append(dict(path=path+'/'+k,before=a.get(k),after=b.get(k)))
   else:walk(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  if len(a)!=len(b):differences.append(dict(path=path+'/length',before=len(a),after=len(b)))
  for i,(x,y) in enumerate(zip(a,b)):walk(x,y,path+'/'+str(i))
 elif a!=b:
  if isinstance(a,str):
   try:x,y=json.loads(a),json.loads(b)
   except (ValueError,TypeError):pass
   else:
    differences.append(dict(path=path,rawBeforeSha256=hashlib.sha256(a.encode()).hexdigest(),rawAfterSha256=hashlib.sha256(b.encode()).hexdigest(),decodedEquivalent=x==y))
    walk(x,y,path+'/decoded');return
  differences.append(dict(path=path,before=a,after=b))
walk(a,b)
print(json.dumps(dict(left=str(left),right=str(right),leftSha256=hashlib.sha256(left.read_bytes()).hexdigest(),rightSha256=hashlib.sha256(right.read_bytes()).hexdigest(),exactEqual=a==b,differences=differences,comparison='Diagnostic only; original raw values and failing equality remain unchanged.'),indent=2))
