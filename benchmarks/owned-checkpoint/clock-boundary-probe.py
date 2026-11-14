import hashlib, json, os, subprocess, sys
from pathlib import Path
sys.path.insert(0, '/probe')
from test_owned_reset import Child
from test_owned_evaluator import save
out=Path('/out'); (out/'home').mkdir()
config=out/'config.toml'
config.write_text('''[embedding]
provider="openai"
model_name="offline-known-v1"
api_key_env="RECALLD_EVAL_OFFLINE_KEY"
base_url="http://127.0.0.1:1/v1"
dimensions=4
[decay]
disable_sweep=true
[rif]
enabled=false
[graph]
autolink_enabled=false
''')
child=Child(Path('/binary/owned_evaluator'),config,out/'source')
checkpoint=None
try:
    child.initialize()
    before=child.private('inspect',{})['runtimeState']['nowMillis']
    at=child.private('advance',{'deltaMs':(1<<48)-before,'deadlineMs':30000})
    assert at['nowMillis']==1<<48
    response=child.control.call('snapshot',{'deadlineMs':30000})
    save(out/'snapshot-response.json',response)
    checkpoint=response.get('result')
finally:
    child.close()
if checkpoint:
    restore=Child(Path('/binary/owned_evaluator'),config,out/'restore',checkpoint)
    try:
        assert restore.process.wait(timeout=15)!=0
        assert not restore.root.exists()
    finally:
        restore.close()
    save(out/'result.json',{'reproduced':True,'unsupportedClock':1<<48,'snapshotSucceeded':True,'restoreRejected':True})
    sys.exit(2)
else:
    assert not list(out.glob('recalld-frozen-canonical-*/manifest.json'))
    save(out/'result.json',{'reproduced':False,'unsupportedClock':1<<48,'snapshotRejected':True,'manifestPublished':False})
