import hashlib,json,pathlib,subprocess,time
out=pathlib.Path('/output'); source=pathlib.Path('/source')
manifest=json.loads(pathlib.Path('/SOURCE.json').read_text())
def verify(label):
    actual={str(p.relative_to(source)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(source.rglob('*')) if p.is_file()}
    ok=actual==manifest['files']
    (out/(label+'.json')).write_text(json.dumps({'matches':ok,'commit':manifest['commit'],'files':len(actual)}))
    assert ok
def run(name,args,timeout=900):
    start=time.monotonic()
    with (out/(name+'.stdout')).open('wb') as stdout,(out/(name+'.stderr')).open('wb') as stderr:
        result=subprocess.run(args,stdout=stdout,stderr=stderr,timeout=timeout)
    (out/(name+'.json')).write_text(json.dumps({'command':args,'exit':result.returncode,'seconds':time.monotonic()-start}))
    return result.returncode
verify('source-before')
try:
    # Preserve prior package executable attribution before cleaning this owner's package.
    old=[]
    for p in pathlib.Path('/target/debug/deps').glob('recalld-*'):
        if p.is_file() and p.suffix=='' and p.stat().st_mode & 0o111:
            old.append({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
    (out/'previous-executables.json').write_text(json.dumps(old,indent=2))
    assert run('clean',['cargo','clean','-p','recalld'])==0
    rc=run('compile',['cargo','test','--offline','--lib','--no-run','--message-format=json'])
    if rc==0:
        artifacts=[]
        for line in (out/'compile.stdout').read_text().splitlines():
            try: v=json.loads(line)
            except json.JSONDecodeError: continue
            if v.get('reason')=='compiler-artifact' and v.get('executable') and v['target']['name']=='recalld':artifacts.append(v['executable'])
        assert len(artifacts)==1,artifacts
        exe=pathlib.Path(artifacts[0]); pre=hashlib.sha256(exe.read_bytes()).hexdigest()
        (out/'executable.json').write_text(json.dumps({'path':str(exe),'sha256':pre}))
        run('test',[str(exe),'storage::metadata::effects::ack_tests::startup_ack_actual_rebuild_has_linear_validation_work','--exact','--nocapture'],120)
        assert hashlib.sha256(exe.read_bytes()).hexdigest()==pre
finally:
    verify('source-after')
