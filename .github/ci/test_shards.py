#!/usr/bin/env python3
"""Partition ordinary Cargo/libtest runs; missing coverage is a failed check."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

SHARDS = 4
CARGO = ["cargo", "test", "--all-features"]


def check(condition, message):
    if not condition:
        raise ValueError(message)


def command(args, log):
    print("+ " + " ".join(args), flush=True)
    with log.open("w") as output:
        process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   text=True, encoding="utf-8")
        for line in process.stdout:
            output.write(line)
            print(line, end="", flush=True)
        code = process.wait()
    check(code == 0, f"command exited {code}: {args[0]}; see {log}")
    return log.read_text()


def listing(text):
    names = []
    totals = []
    for line in text.splitlines():
        match = re.fullmatch(r"(.+): test", line)
        if match:
            names.append(match[1])
        match = re.fullmatch(r"(\d+) tests?, (\d+) benchmarks?", line)
        if match:
            check(int(match[2]) == 0, "unexpected benchmark harness")
            totals.append(int(match[1]))
    check(len(totals) == 1 and totals[0] == len(names), "incomplete test listing")
    check(len(names) == len(set(names)), "duplicate test name")
    return sorted(names)


def results(text, selected, ignored):
    found = {}
    summaries = []
    for line in text.splitlines():
        match = re.fullmatch(r"test (.+?)(?: - should panic)? \.\.\. (ok|ignored|FAILED)(?:, .*)?", line)
        if match:
            name, status = match.groups()
            check(name not in found, "duplicate test result: " + name)
            found[name] = status
        match = re.fullmatch(
            r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
            r"(\d+) measured; (\d+) filtered out; finished in .+", line)
        if match:
            summaries.append(tuple(map(int, match.groups())))
    check(set(found) == set(selected), "executed names differ from assigned names")
    check(len(summaries) == 1, "missing/duplicate successful harness summary")
    expected_ignored = set(selected) & set(ignored)
    check(summaries[0][:4] == (len(selected) - len(expected_ignored), 0,
                                len(expected_ignored), 0), "result totals mismatch")
    check(all(status == ("ignored" if name in expected_ignored else "ok")
              for name, status in found.items()), "failed or unexpectedly ignored test")
    return found


def selection(harness, shard, ordinal):
    if harness["kind"] == "lib":
        return harness["names"][shard::SHARDS]
    return harness["names"] if ordinal % SHARDS == shard else []


def fingerprint(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(shard, directory, os_name):
    directory.mkdir(parents=True, exist_ok=False)
    check(not os.environ.get("RUST_MIN_STACK"), "CI must retain the default test stack")
    provenance = {
        "source": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "os": os_name,
        "compiler": subprocess.check_output(["rustc", "-vV"], text=True),
        "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
    }
    print(json.dumps(provenance, indent=2), flush=True)
    build = command(CARGO + ["--no-run", "--message-format=json"], directory / "build.log")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version=1"], text=True))
    root = Path.cwd().resolve()
    packages = [p for p in metadata["packages"]
                if Path(p["manifest_path"]).parent.resolve() == root]
    check(len(packages) == 1, "expected one root Cargo package")
    package = packages[0]
    artifacts = {}
    examples = set()
    for line in build.splitlines():
        if not line.startswith("{"):
            continue
        event = json.loads(line)
        if event.get("reason") != "compiler-artifact" or event["package_id"] != package["id"]:
            continue
        target = event["target"]
        if target["kind"] == ["example"]:
            examples.add(target["name"])
        if not event.get("executable") or not event["profile"]["test"]:
            continue
        check(target["kind"] in (["lib"], ["bin"], ["test"]), "unexpected test target kind")
        key = target["kind"][0] + ":" + target["name"]
        check(key not in artifacts, "duplicate executable: " + key)
        artifacts[key] = event
    expected = {t["kind"][0] + ":" + t["name"] for t in package["targets"]
                if t["test"] and t["kind"] in (["lib"], ["bin"], ["test"])}
    check(set(artifacts) == expected and any(k.startswith("lib:") for k in expected),
          "Cargo test artifact inventory incomplete")
    expected_examples = {t["name"] for t in package["targets"] if t["kind"] == ["example"]}
    check(examples == expected_examples, "baseline example compilation incomplete")
    inventory = {}
    evidence = {}
    for ordinal, (key, artifact) in enumerate(sorted(artifacts.items())):
        kind, name = key.split(":", 1)
        selector = ["--lib"] if kind == "lib" else ["--" + kind, name]
        prefix = directory / key.replace(":", "-")
        executable = artifact["executable"]
        # Listing executes no test body. Execution below remains through Cargo,
        # preserving its runtime environment, working directory and libtest stack.
        names = listing(command([executable, "--list", "--format", "pretty"],
                                prefix.with_suffix(".list.log")))
        ignored = listing(command([executable, "--ignored", "--list", "--format", "pretty"],
                                  prefix.with_suffix(".ignored.log")))
        check(set(ignored) <= set(names), "ignored tests missing from inventory")
        harness = {"kind": kind, "names": names, "ignored": ignored}
        inventory[key] = harness
        selected = selection(harness, shard, ordinal)
        assigned = kind == "lib" or ordinal % SHARDS == shard
        status = {}
        binary_hash = None
        if assigned:
            binary_hash = fingerprint(executable)
            check(selected or not names, "empty shard would execute an unfiltered harness")
            args = CARGO + selector + ["--", "--color", "never", "--format", "pretty", "--exact"] + selected
            check(sum(len(a.encode()) + 1 for a in args) < 120_000, "exact filters exceed bounded argv")
            status = results(command(args, prefix.with_suffix(".run.log")), selected, ignored)
            check(fingerprint(executable) == binary_hash, "test executable changed during execution")
        evidence[key] = {"selected": selected, "results": status,
                         "executed": assigned, "binary_sha256": binary_hash}
    docs = None
    if shard == 0:
        names = listing(command(CARGO + ["--doc", "--", "--list", "--format", "pretty"],
                                directory / "docs.list.log"))
        ignored = listing(command(CARGO + ["--doc", "--", "--ignored", "--list", "--format", "pretty"],
                                  directory / "docs.ignored.log"))
        status = results(command(CARGO + ["--doc", "--", "--color", "never", "--format", "pretty"],
                                 directory / "docs.run.log"), names, ignored)
        docs = {"names": names, "ignored": ignored, "results": status}
    report = dict(provenance, shard=shard, shards=SHARDS, inventory=inventory,
                  evidence=evidence, compiled_examples=sorted(examples), docs=docs, success=True)
    subprocess.run(["git", "diff", "--exit-code", "HEAD", "--"], check=True)
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")


def verify(reports, source, os_name):
    check(len(reports) == SHARDS, "missing or extra shard reports")
    check(sorted(r["shard"] for r in reports) == list(range(SHARDS)), "duplicate/wrong shard")
    baseline = reports[0]
    for report in reports:
        check(report["success"] is True and report["shards"] == SHARDS, "unsuccessful shard")
        check(report["source"] == source and report["os"] == os_name, "foreign source/OS report")
        for field in ["compiler", "cargo", "inventory", "compiled_examples"]:
            check(report[field] == baseline[field], "shard inventory/provenance drift: " + field)
        check(set(report["evidence"]) == set(baseline["inventory"]), "missing harness evidence")
    for ordinal, (key, harness) in enumerate(sorted(baseline["inventory"].items())):
        found = []
        for report in reports:
            shard = report["shard"]
            evidence = report["evidence"][key]
            selected = selection(harness, shard, ordinal)
            check(evidence["selected"] == selected, "wrong shard partition")
            check(evidence["executed"] == (harness["kind"] == "lib" or ordinal % SHARDS == shard),
                  "harness execution missing")
            check(bool(re.fullmatch(r"[0-9a-f]{64}", evidence["binary_sha256"] or ""))
                  if evidence["executed"] else evidence["binary_sha256"] is None,
                  "missing or unexpected executed binary identity")
            expected_results = {n: "ignored" if n in harness["ignored"] else "ok" for n in selected}
            check(evidence["results"] == expected_results, "missing/failed/extra test result")
            found.extend(selected)
        check(sorted(found) == harness["names"], "partition union differs from inventory")
        check(len(found) == len(set(found)), "duplicate test execution")
    for report in reports:
        doc = report["docs"]
        if report["shard"] != 0:
            check(doc is None, "duplicate docs run")
        else:
            check(doc is not None and len(doc["names"]) == len(set(doc["names"])), "missing docs inventory")
            check(set(doc["ignored"]) <= set(doc["names"]), "invalid doc ignored inventory")
            check(doc["results"] == {n: "ignored" if n in doc["ignored"] else "ok" for n in doc["names"]},
                  "incomplete docs results")
    print(f"Verified {os_name}: {sum(len(h['names']) for h in baseline['inventory'].values())} "
          f"tests inventoried across {SHARDS} shards; docs and example compilation retained.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["run", "verify"])
    parser.add_argument("--shard", type=int, choices=range(SHARDS))
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--os", required=True)
    parser.add_argument("--source")
    args = parser.parse_args()
    try:
        if args.mode == "run":
            check(args.shard is not None, "shard required")
            run(args.shard, args.directory, args.os)
        else:
            check(args.source, "source required")
            verify([json.loads(p.read_text()) for p in args.directory.rglob("report.json")],
                   args.source, args.os)
    except (ValueError, KeyError, OSError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
