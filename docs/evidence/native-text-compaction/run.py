"""Isolated offline validation; one retained target, immutable source per run."""
import hashlib, json, pathlib, shutil, subprocess, sys
source = pathlib.Path(__file__).resolve().parents[3]
mode = sys.argv[1]
assert mode in ('before', 'focused', 'full')
label = sys.argv[2] if len(sys.argv) > 2 else mode
assert label.replace('-', '').isalnum()
root = pathlib.Path('/tmp/recalld-native-text-compaction-validation') / label
root.mkdir(parents=True, exist_ok=False)
(root/'source').mkdir(); (root/'output').mkdir(); (root/'cargo').mkdir()
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source).decode().strip()
assert not subprocess.check_output(['git','diff','HEAD','--','src','Cargo.toml','Cargo.lock'],cwd=source)
paths = subprocess.check_output(['git','ls-files','-z','src','examples','tests','scripts','Cargo.toml','Cargo.lock','rustfmt.toml','.cargo'],cwd=source).decode().split('\0')
hashes={}
for name in filter(None,paths):
    p=source/name
    if not p.is_file(): continue
    target=root/'source'/name; target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(p,target)
    hashes[name]=hashlib.sha256(target.read_bytes()).hexdigest()
(root/'output/source.json').write_text(json.dumps({'head':head,'files':hashes},indent=2))
(root/'output/source.sha256').write_text(''.join(f'{h}  {p}\n' for p,h in hashes.items()))
target=pathlib.Path('/tmp/recalld-owned-native-catalog-ids-full-xk182ybj/target')
cmd=['docker','run','--rm','--name','recalld-feynman-native-compaction','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev','-w','/work/source','-e','HOME=/tmp/home','-e','RUSTUP_TOOLCHAIN=1.94.1','-e','CARGO_HOME=/work/cargo','-e','CARGO_TARGET_DIR=/work/target','-e','CARGO_BUILD_JOBS=2','-e','CARGO_NET_OFFLINE=true','-e','RUST_TEST_THREADS=2','-e','CARGO_PROFILE_DEV_DEBUG=0','-e','CARGO_PROFILE_TEST_DEBUG=0']
for host,guest,ro in [(root,'/work',False),(root/'source','/work/source',True),(target,'/work/target',False),(pathlib.Path('/tmp/recalld-unit-tests-l__ja1yo/cargo/registry'),'/work/cargo/registry',True)]:
    cmd+=['--mount',f'type=bind,src={host},dst={guest}'+(',readonly' if ro else '')]
if mode != 'before':
    cmd += ['--mount','type=bind,src=/tmp/recalld-native-text-compaction-validation/before-corrected/output/legacy-before,dst=/before-compaction,readonly','-e','RECALLD_COMPACTION_BEFORE_INPUT=/before-compaction']
script='''set -eu
mkdir -p /tmp/home
sha256sum -c /work/output/source.sha256 --quiet
rustc -Vv > /work/output/rustc.txt
cargo clean -p recalld > /work/output/clean.log 2>&1
cargo test --locked --offline --all-features --lib --no-run --message-format=json > /work/output/build.jsonl 2> /work/output/build.log
python3 - <<'INNER'
import json,hashlib,shutil,pathlib
for line in pathlib.Path('/work/output/build.jsonl').read_text().splitlines():
    r=json.loads(line)
    if r.get('reason')=='compiler-artifact' and r.get('executable') and r['target']['name']=='recalld':
        src=pathlib.Path(r['executable']); dst=pathlib.Path('/work/output/recalld-test')
        shutil.copyfile(src,dst); dst.chmod(0o755)
        pathlib.Path('/work/output/binary.sha256').write_text(hashlib.sha256(dst.read_bytes()).hexdigest())
INNER
set +e
/work/output/recalld-test compaction --nocapture > /work/output/focused.log 2>&1
result=$?
printf '%s\\n' "$result" > /work/output/focused.exit
set -e
RECALLD_COMPACTION_BEFORE_OUTPUT=/work/output/legacy-before /work/output/recalld-test --exact storage::engine::text_compaction::tests::compaction_genuine_before_fixture --nocapture > /work/output/fixture.log 2>&1
sha256sum -c /work/output/source.sha256 --quiet > /work/output/source-after.txt
cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt
'''
cmd+=['sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef','sh','-c',script]
(root/'output/command.json').write_text(json.dumps(cmd,indent=2))
print(root,flush=True)
result=subprocess.run(cmd)
(root/'output/driver.exit').write_text(str(result.returncode))
print('driver exit',result.returncode,flush=True)
sys.exit(result.returncode)
