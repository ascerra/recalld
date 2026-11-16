import sys,json,hashlib,importlib
from pathlib import Path
site=Path('/reference/site');pins=json.loads(Path('/reference/installed-files.json').read_text());assert pins=={str(p.relative_to(site)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(site.rglob('*')) if p.is_file()}
for name in ['numpy','pytrec_eval','pytrec_eval_ext','tqdm','beir.retrieval.evaluation','beir.retrieval.custom_metrics']:importlib.import_module(name)
rows=[]
for name,module in sorted(sys.modules.items()):
 p=getattr(module,'__file__',None)
 if not p or not Path(p).is_file():continue
 p=Path(p).resolve()
 if p.is_relative_to(site):rows.append(dict(module=name,path=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
 elif not p.is_relative_to('/usr/lib') and not p.is_relative_to('/usr/local/lib') and not p.is_relative_to('/output'):raise RuntimeError('unexpected module path '+str(p))
assert pins=={str(p.relative_to(site)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(site.rglob('*')) if p.is_file()}
Path('/output/module-roster.json').write_text(json.dumps(dict(modules=rows,installedFilesUnchanged=True,interpreterSha256=hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest()),indent=2)+'\n');print('verified imported external modules',len(rows))
