"""Postflight verification against the same read-only external reference bundle."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import tempfile

b=runpy.run_path('/work/source/scripts/beir-mcp-evaluation.py')
verify=b['verify_reference']; scope=verify.__globals__; original=scope['LOCK_PATH']
lock=json.loads(original.read_text()); extension='pytrec_eval_ext.cpython-311-aarch64-linux-gnu.so'
assert extension in lock['files']
checks=[]
for case in ('extension-hash', 'missing-file-pin', 'interpreter-hash'):
    changed=copy.deepcopy(lock)
    if case=='extension-hash': changed['files'][extension]='0'*64
    elif case=='missing-file-pin': changed['files'].pop(extension)
    else: changed['pythonSha256']='0'*64
    with tempfile.TemporaryDirectory() as temp:
        path=Path(temp)/'lock.json';path.write_text(json.dumps(changed));scope['LOCK_PATH']=path
        try:
            verify('/reference/site')
        except ValueError as e:
            checks.append(dict(case=case,rejected=True,reason=str(e)))
        else: raise AssertionError(case)
        finally: scope['LOCK_PATH']=original
verified=verify('/reference/site')
assert verified==hashlib.sha256(original.read_bytes()).hexdigest()
print(json.dumps(dict(status='passed',referenceBundleSha256=verified,checks=checks,
    scope='Actual installed extension/file roster/interpreter compared against intentionally changed expected pins; no installed file was mutated.')))
