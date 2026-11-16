"""Isolated offline validation; one retained target, immutable source per run."""
import hashlib, json, pathlib, shutil, subprocess, sys
source = pathlib.Path(__file__).resolve().parents[3]
mode = sys.argv[1]
assert mode in ('check', 'focused', 'cli', 'full', 'pending-probe', 'before-binary', 'native-consumers', 'pending-capture', 'closure')
label = sys.argv[2] if len(sys.argv) > 2 else mode
assert label.replace('-', '').isalnum()
root = pathlib.Path('/tmp/recalld-vector-certification-validation') / label
root.mkdir(parents=True, exist_ok=False)
(root/'source').mkdir(); (root/'output').mkdir(); (root/'cargo').mkdir()
revision = 'a3c7010c50809bb231e220a7f4000dfd2802979e' if mode == 'before-binary' else 'HEAD'
head = subprocess.check_output(['git', 'rev-parse', revision], cwd=source).decode().strip()
# Uncommitted candidate is identified by every source hash; HEAD is its base, not a validated runtime commit.
paths = subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z','src','examples','tests','scripts','Cargo.toml','Cargo.lock','rustfmt.toml','.cargo'],cwd=source).decode().split('\0')
if mode == 'before-binary':
    paths = subprocess.check_output(['git','ls-tree','-r','--name-only','-z',head,'--','src','examples','tests','scripts','Cargo.toml','Cargo.lock','rustfmt.toml','.cargo'],cwd=source).decode().split('\0')
hashes={}
for name in filter(None,paths):
    p=source/name
    if not p.is_file(): continue
    target=root/'source'/name; target.parent.mkdir(parents=True,exist_ok=True)
    if mode == 'before-binary':
        target.write_bytes(subprocess.check_output(['git','show',head+':'+name],cwd=source))
    else:
        shutil.copyfile(p,target)
    hashes[name]=hashlib.sha256(target.read_bytes()).hexdigest()
(root/'output/source.json').write_text(json.dumps({'head':head,'files':hashes},indent=2))
(root/'output/source.sha256').write_text(''.join(f'{h}  {p}\n' for p,h in hashes.items()))
target=pathlib.Path('/tmp/recalld-owned-native-catalog-ids-full-xk182ybj/target')
cmd=['docker','run','--rm','--name','recalld-feynman-vector-certification','--network','none','--cpus','2','--memory','4g','--memory-swap','4g','--cap-drop','ALL','--security-opt','no-new-privileges','--read-only','--tmpfs','/tmp:rw,nosuid,nodev','-w','/work/source','-e','HOME=/tmp/home','-e','RUSTUP_TOOLCHAIN=1.94.1','-e','CARGO_HOME=/work/cargo','-e','CARGO_TARGET_DIR=/work/target','-e','CARGO_BUILD_JOBS=2','-e','CARGO_NET_OFFLINE=true','-e','RUST_TEST_THREADS=2','-e','CARGO_PROFILE_DEV_DEBUG=0','-e','CARGO_PROFILE_TEST_DEBUG=0']
for host,guest,ro in [(root,'/work',False),(root/'source','/work/source',True),(target,'/work/target',False),(pathlib.Path('/tmp/recalld-unit-tests-l__ja1yo/cargo/registry'),'/work/cargo/registry',True)]:
    cmd+=['--mount',f'type=bind,src={host},dst={guest}'+(',readonly' if ro else '')]
cmd += ['--mount','type=bind,src=/tmp/recalld-vector-certification-fence-validation/before,dst=/before,readonly','-e','RECALLD_CERTIFICATION_BEFORE_BINARY=/before/recalld-test']
context_before = pathlib.Path('/tmp/recalld-vector-certification-validation/before159158/output')
if (context_before/'recalld-test').exists():
    cmd += ['--mount',f'type=bind,src={context_before},dst=/before-context,readonly','-e','RECALLD_CERTIFICATION_CONTEXT_BEFORE_BINARY=/before-context/recalld-test']
script='''set -eu
mkdir -p /tmp/home
trap 'find /work/target/debug/deps -maxdepth 1 -type f -perm /111 ! -name \"*.so\" ! -name \"*.dylib\" -exec sha256sum {} \\; > /work/output/executables.sha256; if [ -f /work/target/debug/recalld-cli ]; then sha256sum /work/target/debug/recalld-cli >> /work/output/executables.sha256; fi; sha256sum -c /work/output/source.sha256 --quiet > /work/output/source-after.txt; cat /sys/fs/cgroup/memory.events > /work/output/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt' EXIT
sha256sum -c /work/output/source.sha256 --quiet
rustc -Vv > /work/output/rustc.txt
cargo clean -p recalld > /work/output/clean.log 2>&1
cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1
'''
if mode == 'before-binary':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo test --locked --offline --lib --no-run > /work/output/before-build.log 2>&1\nfind /work/target/debug/deps -maxdepth 1 -type f -name "recalld-*" -perm /111 -exec cp {} /work/output/recalld-test \\;\nsha256sum /work/output/recalld-test > /work/output/before-executable.sha256')
if mode == 'focused':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo test --locked --offline --lib vector_certification -- --nocapture > /work/output/focused.log 2>&1\nfind /work/target/debug/deps -maxdepth 1 -type f -name \"recalld-*\" -perm /111 -exec sha256sum {} \\; > /work/output/executables.sha256')
if mode == 'pending-probe':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo test --locked --offline --lib vector_certification_fresh_batch_rejects_without_stranding_offline_remedy -- --nocapture > /work/output/focused.log 2>&1')
if mode == 'native-consumers':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo test --locked --offline --lib storage::metadata::native_records::tests -- --nocapture > /work/output/native.log 2>&1')
if mode == 'pending-capture':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo test --locked --offline --lib vector_certification_old_binary_pending_aba -- --nocapture > /work/output/focused.log 2>&1')
if mode == 'closure':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo fmt --all -- --check > /work/output/fmt.log 2>&1\ncargo test --locked --offline --lib vector_certification -- --nocapture > /work/output/focused.log 2>&1\ncargo test --locked --offline --lib native_lifecycle_wire -- --nocapture > /work/output/wire.log 2>&1\ncargo test --locked --offline --test vector_certification_cli -- --nocapture > /work/output/cli.log 2>&1')
if mode == 'cli':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', 'cargo test --locked --offline --test vector_certification_cli -- --nocapture > /work/output/cli.log 2>&1\ncargo test --locked --offline --lib generated_document_seal_binds_actual_input_profile_and_raw_output -- --nocapture > /work/output/seal.log 2>&1')
if mode == 'full':
    script = script.replace('cargo check --locked --offline --all-features --all-targets > /work/output/check.log 2>&1', """cargo fmt --all -- --check > /work/output/fmt.log 2>&1
cargo test --locked --offline --lib vector_certification -- --nocapture > /work/output/focused.log 2>&1
set +e
cargo test --locked --offline --all-targets --no-fail-fast > /work/output/full.log 2>&1
full_status=$?
echo "$full_status" > /work/output/full.exit
cargo clippy --locked --offline --all-features --all-targets > /work/output/clippy.log 2>&1
lint_status=$?
echo "$lint_status" > /work/output/clippy.exit
[ "$full_status" -eq 0 ] && [ "$lint_status" -eq 0 ]""")
if mode == 'pending-capture':
    cmd += ['-e','RECALLD_CERTIFICATION_MULTI_BEFORE_CAPTURE=/work/output/pending-multi-before']
if mode == 'pending-probe':
    cmd += ['-e', 'RECALLD_CERTIFICATION_PENDING_BEFORE=/work/output/pending-before']
cmd+=['sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef','sh','-c',script]
(root/'output/command.json').write_text(json.dumps(cmd,indent=2))
print(root,flush=True)
result=subprocess.run(cmd)
(root/'output/driver.exit').write_text(str(result.returncode))
print('driver exit',result.returncode,flush=True)
sys.exit(result.returncode)
