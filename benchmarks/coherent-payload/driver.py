"""Explicitly invoked only AFTER the user's A5 measurement release."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
SOURCE = Path('/tmp/recalld-implementation/coherent-payload')
HARNESS = Path('/tmp/recalld-implementation/observed-consumer-evidence/scripts/public-conformance.py')
IMAGE = 'sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef'
spec = importlib.util.spec_from_file_location('public_runner', HARNESS)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
mode = sys.argv[1]
assert mode in ('format', 'test', 'full')
root = Path(tempfile.mkdtemp(prefix=f'recalld-coherent-payload-{mode}-', dir='/tmp'))
out = root / 'output'
out.mkdir()
(out / 'driver.py').write_bytes(Path(__file__).read_bytes())
commit = runner.git(SOURCE, 'rev-parse', 'HEAD').decode().strip()
provenance, patch = runner.snapshot(SOURCE, root / 'source', extra=('src/storage/metadata/native_records/fixtures/phase-clock-legacy.jsonl',), expected_commit=commit if mode in ('test', 'full') else None)
(out / 'source.patch').write_bytes(patch)
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
manifest = {'track': 'coherent-payload-wrapper-readiness', 'status': 'preparing', 'mode': mode,
            **provenance, 'imageId': IMAGE, 'harnessSha256': sha(HARNESS),
            'driverSha256': sha(Path(__file__)), 'publicFixtureVersion': 3, 'publicContract': 'legacy-v1', 'publicClock': 'actual wall clock; no deterministic reset', 'dirtyPatchSha256': sha(out / 'source.patch')}
if mode in ('test', 'full'):
    cache = Path(sys.argv[2])
    assert cache.is_relative_to('/tmp') and cache.name == 'target'
    shutil.copytree('/tmp/recalld-unit-tests-l__ja1yo/cargo/registry', root / 'cargo/registry')
    shutil.copytree(cache, root / 'target')
    script = """set -eu
mkdir -p /tmp/home
trap 'cat /sys/fs/cgroup/memory.events > /work/output/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt' EXIT
rustc -Vv > /work/output/rustc.txt
cat /sys/fs/cgroup/cpu.max > /work/output/cgroup-cpu.txt
cat /sys/fs/cgroup/memory.max > /work/output/cgroup-memory.txt
cargo fmt --all --check > /work/output/fmt.log 2>&1
cargo clean -p recalld > /work/output/package-clean.log 2>&1
cargo test --offline --locked --all-features --lib online_completion_crash_reopen_preserves_receipt_and_deleted_content -j 1 -- --nocapture > /work/output/crash-reopen.log 2>&1
cargo test --offline --locked --all-features --lib coherent_payload -j 1 -- --nocapture > /work/output/payload-tests.log 2>&1
cargo test --offline --locked --all-features --lib coherent_ -j 1 > /work/output/coherent-tests.log 2>&1
cargo test --offline --locked --all-features --lib api::vector_query_tests -j 1 > /work/output/http-tests.log 2>&1
cargo test --offline --locked --all-features --lib observed_join_tests -j 1 > /work/output/fault-tests.log 2>&1
"""
    if mode == 'full':
        script = script[:script.index('cargo test')] + """cargo test --offline --locked --all-features --lib online_completion_crash_reopen_preserves_receipt_and_deleted_content -j 1 -- --nocapture > /work/output/crash-reopen.log 2>&1
cargo test --offline --locked --all-features --lib coherent_payload -j 1 -- --nocapture > /work/output/payload-tests.log 2>&1
cargo test --offline --locked --all-features --lib purge_releases_storage_before_graph -j 1 -- --nocapture > /work/output/feedback-lock-order.log 2>&1
cargo test --offline --locked --all-features --lib real_rif_changes_legacy_scores -j 1 > /work/output/focused-rif.log 2>&1
cargo test --offline --locked --all-features --lib by_id_actual_rest_and_mcp -j 1 > /work/output/focused-offline-byid.log 2>&1
cargo test --offline --locked --all-features --lib synthetic_canonical_labels_survive_actual_http -j 1 > /work/output/focused-offline-labels.log 2>&1
cargo test --offline --locked --all-features --lib accepted_query_ -j 1 > /work/output/focused-transport-roots.log 2>&1
cargo build --offline --locked --bin recalld > /work/output/build.log 2>&1
python3 scripts/public-conformance.py --inside --fixture-version 3 > /work/output/public-runner.log 2>&1
cargo test --offline --locked --all-features --lib --bins --tests --examples -j 1 > /work/output/full-suite.log 2>&1
cargo clippy --offline --locked --all-features --all-targets > /work/output/clippy.log 2>&1
python3 scripts/test_public_conformance.py > /work/output/runner-tests.log 2>&1
"""
else:
    script = 'cargo fmt --all > /work/output/format.log 2>&1' 
command = ['docker', 'run', '--rm', '--network', 'none', '--cpus', '2', '--memory', '4g',
           '--memory-swap', '4g', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
           '--read-only', '--tmpfs', '/tmp:rw,nosuid,nodev',
           '--mount', f'type=bind,src={root},dst=/work',
           '-w', '/work/source', '-e', 'HOME=/tmp/home', '-e', 'RUSTUP_TOOLCHAIN=1.94.1',
           '-e', 'CARGO_HOME=/work/cargo', '-e', 'CARGO_TARGET_DIR=/work/target',
           '-e', 'CARGO_BUILD_JOBS=2', '-e', 'CARGO_NET_OFFLINE=true']
if mode in ('test', 'full'):
    command += ['--mount', f'type=bind,src={root}/source,dst=/work/source,readonly']
command += [IMAGE, 'sh', '-c', script]
manifest['command'] = command
runner.save(out / 'manifest.json', manifest)
print(root, flush=True)
result = subprocess.run(command)
if mode in ('test', 'full'):
    executables = [p for p in (root / 'target/debug/deps').glob('recalld-*')
                   if p.is_file() and p.stat().st_mode & 0o111 and p.suffix == '']
    for name in ('recalld',):
        candidate = root / 'target/debug' / name
        if candidate.is_file():
            executables.append(candidate)
    runner.save(out / 'source-binaries.json', {
        'sourceCommit': commit,
        'sourceTree': runner.git(SOURCE, 'rev-parse', commit + '^{tree}').decode().strip(),
        'packageCleanLogSha256': sha(out / 'package-clean.log') if (out / 'package-clean.log').exists() else None,
        'executablesSha256': {str(p.relative_to(root)): sha(p) for p in executables},
    })
manifest['exitCode'] = result.returncode
manifest['status'] = 'passed' if result.returncode == 0 else 'failed'
manifest['outputsSha256'] = {str(p.relative_to(out)): sha(p) for p in out.rglob('*') if p.is_file() and p.name != 'manifest.json' and 'probe-store' not in p.parts}
runner.save(out / 'manifest.json', manifest)
if mode == 'format' and result.returncode == 0:
    changed = set(runner.git(SOURCE, 'diff', 'b641d1e', '--name-only').decode().splitlines()) | {'src/storage/metadata/work.rs'}
    for name, before in provenance['sourceFilesSha256'].items():
        if name.endswith('.rs') and sha(root / 'source' / name) != before:
            assert name in changed, f'formatter changed unrelated source: {name}'
            assert sha(SOURCE / name) == before, f'source changed during format: {name}'
            shutil.copyfile(root / 'source' / name, SOURCE / name)
elif mode in ('test', 'full'):
    for name, expected in provenance['sourceFilesSha256'].items():
        assert sha(root / 'source' / name) == expected, name
print(json.dumps({'root': str(root), 'status': manifest['status'], 'exitCode': result.returncode}), flush=True)
sys.exit(result.returncode)
