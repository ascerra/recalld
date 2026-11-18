#!/bin/sh
cd /work
rustc -Vv > /evidence/rustc.txt
cargo -Vv > /evidence/cargo.txt
cargo fmt --all --check > /evidence/full-fmt.log 2>&1
fmt=$?
printf 'fmt %s\n' "$fmt" > /evidence/full-status.txt
cargo clippy --locked --offline --all-features --all-targets > /evidence/clippy.log 2>&1
clippy=$?
printf 'clippy %s\n' "$clippy" >> /evidence/full-status.txt
cargo test --locked --offline --all-features --no-run --message-format=json > /evidence/artifacts.jsonl 2> /evidence/compile.log
compile=$?
printf 'compile %s\n' "$compile" >> /evidence/full-status.txt
if [ "$compile" -eq 0 ]; then
 python3 - <<'PYHASH'
import json,hashlib,pathlib
items={}
for line in pathlib.Path('/evidence/artifacts.jsonl').read_text().splitlines():
 try: a=json.loads(line)
 except ValueError: continue
 if a.get('executable'):
  p=pathlib.Path(a['executable']);items[str(p)]={'sha256':hashlib.file_digest(p.open('rb'),'sha256').hexdigest(),'bytes':p.stat().st_size}
pathlib.Path('/evidence/EXECUTABLES-BEFORE.json').write_text(json.dumps(items,indent=2)+'\n')
PYHASH
 cargo test --locked --offline --all-features > /evidence/full.log 2>&1
 full=$?
 python3 - <<'PYHASH'
import json,hashlib,pathlib
items=json.loads(pathlib.Path('/evidence/EXECUTABLES-BEFORE.json').read_text())
for name,item in items.items():
 p=pathlib.Path(name);assert hashlib.file_digest(p.open('rb'),'sha256').hexdigest()==item['sha256'],name
pathlib.Path('/evidence/EXECUTABLES-AFTER.json').write_text(json.dumps({'verified':True,'files':items},indent=2)+'\n')
PYHASH
 hashes=$?
else
 full=99
 hashes=99
fi
printf 'full %s\nhashes %s\n' "$full" "$hashes" >> /evidence/full-status.txt
cat /sys/fs/cgroup/memory.events > /evidence/full-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/full-memory.peak
if [ "$fmt" -eq 0 ] && [ "$clippy" -eq 0 ] && [ "$full" -eq 0 ] && [ "$hashes" -eq 0 ]; then
 echo 0 > /evidence/full-exit-code
 exit 0
fi
echo 1 > /evidence/full-exit-code
exit 1
