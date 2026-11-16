"""One owned cache/target, read-only frozen source, offline temporary stores."""
import hashlib
import json
import pathlib
import re
import subprocess
import sys

source = pathlib.Path(__file__).resolve().parents[3]
mode, label = sys.argv[1:]
assert mode in ("focused", "full", "before") and label.isalnum()
out = source / ".validation" / label
out.mkdir(parents=True, exist_ok=False)
cache = pathlib.Path("/tmp/recalld-curie-active-build")
assert (cache / "cargo").is_dir() and (cache / "target").is_dir()
names = subprocess.check_output(
    ["git", "ls-files", "-z", "src", "tests", "examples", "Cargo.toml", "Cargo.lock", ".cargo"],
    cwd=source,
).decode().split("\0")

def inputs():
    return {name: hashlib.sha256((source / name).read_bytes()).hexdigest()
            for name in names if name and (source / name).is_file()}

before = inputs()
(out / "source.json").write_text(json.dumps({
    "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source).decode().strip(),
    "files": before,
}, indent=2) + "\n")
steps = []
if mode in ("focused", "before"):
    # Source root changed from previous owned work: no stale BEFORE executable.
    if label in ("before001", "focus002"):
        steps.append(("clean", "cargo clean --offline -p recalld"))
    for label_filter, pattern in (
        (("boundary", "certified_standalone"),) if mode == "before" else
        (("boundary", "certified_standalone"), ("producer", "native_graph_producer"),
         ("coregraph", "native_graph"))
    ):
        steps.append((label_filter, "cargo test --locked --offline --all-features --lib "
                      + pattern + " -- --nocapture"))
else:
    steps.extend([
        ("fmt", "cargo fmt --all -- --check"),
        ("full", "cargo test --locked --offline --all-features --all-targets --no-fail-fast"),
        ("clippy", "cargo clippy --locked --offline --all-features --all-targets"),
    ])
script = """set -u
mkdir -p /tmp/empty-home
trap 'cat /sys/fs/cgroup/memory.events > /out/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /out/memory-peak.txt' EXIT
rustc -Vv > /out/rustc.txt
result=0
"""
for name, command in steps:
    script += f"{command} > /out/{name}.log 2>&1\nstatus=$?\necho $status > /out/{name}.exit\n"
    script += "if [ $status -ne 0 ]; then result=1; fi\n"
    if name == "clean":
        script += "if [ $status -ne 0 ]; then exit $status; fi\n"
    elif mode in ("focused", "before"):
        script += f"if grep -q 'could not compile' /out/{name}.log; then exit 1; fi\n"
script += "exit $result\n"
cmd = ["docker", "run", "--rm", "--name", "recalld-curie-certified-graph-" + label,
       "--network", "none", "--cpus", "2", "--memory", "4g",
       "-e", "RUSTUP_TOOLCHAIN=1.94.1", "-e", "CARGO_BUILD_JOBS=2",
       "-e", "CARGO_HOME=/cargo", "-e", "CARGO_TARGET_DIR=/target",
       "-e", "HOME=/tmp/empty-home", "-e", "CARGO_PROFILE_DEV_DEBUG=0",
       "-e", "CARGO_PROFILE_TEST_DEBUG=0",
       "--mount", f"type=bind,src={source},dst=/work,readonly",
       "--mount", f"type=bind,src={out},dst=/out",
       "--mount", f"type=bind,src={cache / 'cargo'},dst=/cargo",
       "--mount", f"type=bind,src={cache / 'target'},dst=/target", "-w", "/work",
       "sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef",
       "sh", "-c", script]
(out / "command.json").write_text(json.dumps(cmd, indent=2) + "\n")
print(out, flush=True)
status = subprocess.run(cmd).returncode
(out / "driver.exit").write_text(str(status) + "\n")
after = inputs()
(out / "source-unchanged.json").write_text(json.dumps(before == after) + "\n")
executed = set()
for name, _ in steps:
    log = out / (name + ".log")
    if log.exists():
        executed.update(re.findall(r"Running [^\n]*\((/target/[^)]+)\)", log.read_text()))
digests = {}
for path in sorted(executed):
    actual = cache / "target" / path.removeprefix("/target/")
    digests[path] = hashlib.sha256(actual.read_bytes()).hexdigest()
(out / "executed-binaries.json").write_text(json.dumps(digests, indent=2) + "\n")
print("driver", status, "source unchanged", before == after, "test binaries", len(digests), flush=True)
sys.exit(status if before == after else 2)
