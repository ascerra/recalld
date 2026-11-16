import datetime, hashlib, json, os, pathlib, signal, subprocess, time
out = pathlib.Path('/output')
source = pathlib.Path('/source')
manifest = json.loads(pathlib.Path('/SOURCE.json').read_text())
executables = json.loads(pathlib.Path('/EXECUTABLES.json').read_text())
def save(name, value):
    (out / (name + '.json')).write_text(json.dumps(value, indent=2) + '\n')
def verify(label):
    actual = {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(source.rglob('*')) if p.is_file()}
    binaries = {p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest() for p in executables}
    save(label, {'sourceMatches': actual == manifest['files'], 'executableMatches': binaries == executables, 'commit': manifest['commit'], 'files': len(actual), 'executables': binaries})
    assert actual == manifest['files'] and binaries == executables

def resources(label):
    paths = ['cpu.stat', 'memory.peak', 'memory.events', 'memory.current']
    save(label, {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'cgroup': {p: (pathlib.Path('/sys/fs/cgroup') / p).read_text() for p in paths}})

def run(name, args, timeout):
    start = time.monotonic()
    result = {'command': args, 'budgetSeconds': timeout, 'timedOut': False}
    with (out / (name + '.stdout')).open('wb') as stdout, (out / (name + '.stderr')).open('wb') as stderr:
        proc = subprocess.Popen(args, stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            result['exit'] = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            result['timedOut'] = True
            os.killpg(proc.pid, signal.SIGTERM)
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
            result['exit'] = proc.returncode
    result['seconds'] = time.monotonic() - start
    save(name, result)
    return result['exit'] == 0 and not result['timedOut']

verify('source-executables-before')
resources('resources-before')
try:
    ok = run('full', ['cargo', 'test', '--offline', '--all-targets'], 1800)
    resources('resources-after-full')
    verify('source-executables-after-full')
    if ok:
        checks_deadline = time.monotonic() + 600
        ok = run('fmt', ['cargo', 'fmt', '--all', '--', '--check'], max(1, checks_deadline - time.monotonic()))
        if ok:
            ok = run('clippy', ['cargo', 'clippy', '--offline', '--all-targets'], max(1, checks_deadline - time.monotonic()))
    save('result', {'passed': ok, 'sourceCommit': manifest['commit'], 'focusedRerun': False, 'packageClean': False})
finally:
    resources('resources-final')
    verify('source-executables-final')
