import hashlib, importlib.util, json, subprocess, sys, tempfile
from pathlib import Path
sys.dont_write_bytecode = True
name, mode = sys.argv[1:3]
assert name in ('owned-native-catalog-ids',) and mode in ('format', 'red', 'focused', 'full')
source = Path('/tmp/recalld-implementation') / name
harness = Path('/tmp/recalld-implementation/observed-consumer-evidence/scripts/public-conformance.py')
spec = importlib.util.spec_from_file_location('runner', harness)
r = importlib.util.module_from_spec(spec); spec.loader.exec_module(r)
root = Path(tempfile.mkdtemp(prefix=f'recalld-{name}-{mode}-', dir='/tmp')); out=root/'output'; out.mkdir()
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
commit=r.git(source,'rev-parse','HEAD').decode().strip()
extra=['src/storage/metadata/native_records/fixtures/phase-clock-legacy.jsonl']
extra+=['scripts/test_owned_evaluator.py']
if name == 'owned-reset': extra+=['scripts/test_owned_reset.py']
provenance,patch=r.snapshot(source,root/'source',extra=extra,expected_commit=commit if mode!='format' else None)
(out/'source.patch').write_bytes(patch); (out/'driver.py').write_bytes(Path(__file__).read_bytes())
image='sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
command=['docker','run','--rm','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev','--mount',f'type=bind,src={root},dst=/work','-w','/work/source','-e','HOME=/tmp/home','-e','RUSTUP_TOOLCHAIN=1.94.1','-e','CARGO_HOME=/work/cargo','-e','CARGO_TARGET_DIR=/work/target','-e','CARGO_BUILD_JOBS=2','-e','CARGO_NET_OFFLINE=true','-e','RUST_TEST_THREADS=2','-e','CARGO_PROFILE_DEV_DEBUG=0','-e','CARGO_PROFILE_TEST_DEBUG=0','-e','RECALLD_CONTEXT_BEFORE_FIXTURES=/before','-e','RECALLD_CONTEXT_OLD_TEST_BINARY=/old/recalld-test','--mount','type=bind,src=/tmp/recalld-native-context-validation/before/fixtures,dst=/before,readonly','--mount','type=bind,src=/tmp/recalld-native-context-validation/before/target/debug/deps/recalld-b2844bb1c4a63074,dst=/old/recalld-test,readonly']
script='cargo fmt --all > /work/output/fmt.log 2>&1'
if mode!='format':
    cache=Path(sys.argv[3]); assert cache.is_relative_to('/tmp') and cache.name=='target'
    (root/'cargo').mkdir()
    command+=['--mount','type=bind,src=/tmp/recalld-unit-tests-l__ja1yo/cargo/registry,dst=/work/cargo/registry,readonly']
    subprocess.run(['cp','-cR',str(cache),str(root/'target')],check=True)
    command+=['--mount',f'type=bind,src={root}/source,dst=/work/source,readonly']
    (out/'source-files.sha256').write_text(''.join(f'{v}  {k}\n' for k,v in provenance['sourceFilesSha256'].items()))
    script='''set -eu
mkdir -p /tmp/home
sha256sum -c /work/output/source-files.sha256 --quiet
trap 'sha256sum -c /work/output/source-files.sha256 --quiet > /work/output/source-after.txt; cat /sys/fs/cgroup/memory.events > /work/output/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt' EXIT
rustc -Vv > /work/output/rustc.txt
cargo fmt --all --check > /work/output/fmt.log 2>&1
cargo clean -p recalld > /work/output/package-clean.log 2>&1
'''
    script+='cargo test --offline --locked --all-features --lib storage::native_catalog::tests::owned_ids -j 2 -- --nocapture > /work/output/focused.log 2>&1\n'
    if mode!='red':
        script+='''cargo test --offline --locked --all-features --lib native_catalog -j 2 > /work/output/catalog-regressions.log 2>&1
cargo test --offline --locked --all-features --lib native_policy -j 2 > /work/output/policy-regressions.log 2>&1
cargo test --offline --locked --all-features --lib native_purge -j 2 > /work/output/purge-regressions.log 2>&1
cargo test --offline --locked --all-features --lib owned_native_ -j 2 > /work/output/producer-regressions.log 2>&1
cargo test --offline --locked --all-features --lib storage::ids::tests -j 2 > /work/output/ids.log 2>&1
cargo test --offline --locked --all-features --lib bench::frozen::tests -j 2 > /work/output/freeze-regressions.log 2>&1
cargo build --offline --locked --example owned_evaluator --features bench -j 2 > /work/output/example-build.log 2>&1
python3 scripts/test_owned_evaluator.py --binary /work/target/debug/examples/owned_evaluator --output /work/output/controls > /work/output/controls.log 2>&1
'''
    if mode=='full':
        script+='''cargo test --offline --locked --all-features --lib --bins --tests --examples -j 2 > /work/output/full-suite.log 2>&1
cargo clippy --offline --locked --all-features --all-targets -j 2 > /work/output/clippy.log 2>&1
'''
        if name == 'owned-native-catalog-ids':
            script+='''cargo build --offline --locked --bin recalld -j 2 > /work/output/bin-build.log 2>&1
python3 scripts/public-conformance.py --inside --fixture-version 3 > /work/output/public.log 2>&1
python3 scripts/test_public_conformance.py > /work/output/runner-tests.log 2>&1
'''
command += [image,'sh','-c',script]
manifest={**provenance,'mode':mode,'sourceCommit':commit,'command':command,'imageId':image,'driverSha256':sha(Path(__file__)),'dirtyPatchSha256':sha(out/'source.patch'),'status':'running'}
r.save(out/'manifest.json',manifest); print(root,flush=True)
result=subprocess.run(command)
if mode=='format' and result.returncode==0:
    changed=set(r.git(source,'diff','HEAD','--name-only').decode().splitlines())
    for f,before in provenance['sourceFilesSha256'].items():
        if f.endswith('.rs') and sha(root/'source'/f)!=before:
            assert f in changed and sha(source/f)==before,f
            (source/f).write_bytes((root/'source'/f).read_bytes())
if mode!='format':
    binaries=[p for p in (root/'target/debug').rglob('*') if p.is_file() and p.stat().st_mode & 0o111 and (p.name.startswith('recalld-') or p.name=='owned_evaluator')]
    r.save(out/'source-binaries.json',{'sourceCommit':commit,'sha256':{str(p.relative_to(root)):sha(p) for p in binaries}})
manifest['exitCode']=result.returncode;manifest['status']='passed' if result.returncode==0 else 'failed'
manifest['outputsSha256']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file() and p.name!='manifest.json' and 'data' not in p.parts}
r.save(out/'manifest.json',manifest)
print(json.dumps({'root':str(root),'status':manifest['status']}),flush=True);sys.exit(result.returncode)
