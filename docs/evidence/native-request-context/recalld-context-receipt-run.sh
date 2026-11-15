#!/bin/sh
set -u
name=$1
mode=$2
log=/tmp/recalld-native-context-validation/after/$name.log
git -C /tmp/recalld-implementation/native-request-context rev-parse HEAD > /tmp/recalld-native-context-validation/after/$name-source.txt
python3 - "$name" <<'HASHES'
from pathlib import Path
import hashlib,subprocess,sys
root=Path('/tmp/recalld-implementation/native-request-context')
paths=subprocess.check_output(['git','ls-files','-z'],cwd=root).decode().split('\0')
lines=[hashlib.sha256((root/p).read_bytes()).hexdigest()+'  '+p for p in paths if p and (root/p).is_file()]
Path('/tmp/recalld-native-context-validation/after/'+sys.argv[1]+'-source-files.sha256').write_text('\n'.join(lines)+'\n')
HASHES
if [ "$mode" = full ]; then
  task='cargo fmt --check && cargo test --offline && cargo clippy --offline --all-targets'
else
  task='cargo fmt --check && cargo test --offline --lib native_purge_accepted_unknown_child_preserves_reserved_id_and_reopens -- --nocapture && cargo test --offline --lib native_context_purge_replay_validation_preserves_exact_known_receipt -- --nocapture'
fi
docker run --rm --name recalld-context-validation --network none --cpus 2 --memory 4g -e RUSTUP_TOOLCHAIN=1.94.1 -e CARGO_BUILD_JOBS=2 -e RUST_TEST_THREADS=2 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e RECALLD_CONTEXT_BEFORE_FIXTURES=/before -e RECALLD_CONTEXT_OLD_TEST_BINARY=/old/recalld-test -v /tmp/recalld-implementation/native-request-context:/work:ro -v /tmp/recalld-native-context-validation/after/$name-source-files.sha256:/source-files.sha256:ro -v /tmp/recalld-native-context-validation/after/target:/target -v /tmp/recalld-native-context-validation/before/fixtures:/before:ro -v /tmp/recalld-native-context-validation/before/target/debug/deps/recalld-b2844bb1c4a63074:/old/recalld-test:ro -v /tmp/recalld-unit-tests-l__ja1yo/cargo/registry:/cargo/registry:ro -w /work recalld-ci:1.94 sh -c "sha256sum -c /source-files.sha256 --quiet && { $task; code=\$?; sha256sum -c /source-files.sha256 --quiet || exit 125; exit \$code; }" > "$log" 2>&1
rc=$?
echo "$rc" > /tmp/recalld-native-context-validation/after/$name-exit.txt
exit "$rc"
