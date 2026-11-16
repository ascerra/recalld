import subprocess,pathlib,json,time,hashlib,sys
out=pathlib.Path('.test-scratch/third'); results=[]
def run(name,args):
    start=time.time()
    with (out/(name+'.log')).open('w') as log:
        code=subprocess.call(args,stdout=log,stderr=subprocess.STDOUT)
    results.append({'name':name,'command':args,'exit':code,'start':start,'end':time.time()})
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(name,code,flush=True)
    if code: raise SystemExit(code)
try:
    run('mount',['python3','.test-scratch/seal.py','verify',str(out)])
    run('format',['cargo','fmt','--all'])
    run('source',['python3','.test-scratch/seal.py','seal',str(out)])
    run('focused',['cargo','test','--offline','--lib','forget_origin','--','--nocapture'])
    run('original',['cargo','test','--offline','--lib','original_api','--','--nocapture'])
    run('record-context',['cargo','test','--offline','--lib','request_context','--','--nocapture'])
    run('forget',['cargo','test','--offline','--lib','native_forget','--','--nocapture'])
    run('full',['cargo','test','--offline'])
    run('fmt',['cargo','fmt','--all','--','--check'])
    run('clippy',['cargo','clippy','--offline','--all-targets'])
finally:
    subprocess.call(['python3','.test-scratch/seal.py','binary',str(out)])
    if (out/'source.json').exists():
        expected=json.loads((out/'source.json').read_text())
        assert all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==h for p,h in expected.items())
        print('source unchanged',len(expected),flush=True)
