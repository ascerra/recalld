"""Runner boundary regression tests; execute in the isolated Rust container."""
from pathlib import Path
import json
import runpy
import subprocess
import tempfile
import unittest
import sys
from types import SimpleNamespace

runner = runpy.run_path(str(Path(__file__).with_name("offline-evaluation.py")))


class SourceSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "checkout"
        self.root.mkdir()
        self.output = Path(self.temp.name) / "snapshot"
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        for name in (*runner["FIXED_INPUTS"], "src/lib.rs", ".validation/tracked.txt", "notes.txt"):
            self.write(name, "fixture\n")
        self.write("Cargo.toml", '[package]\nname="fixture"\nversion="0.1.0"\nedition="2024"\n')
        self.git("add", ".")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "commit", "--no-gpg-sign", "-qm", "fixture")

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True)

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value)

    def test_only_allowlisted_inputs_and_patch_with_exact_hashes(self):
        for name in (".env", ".validation/tracked.txt", ".validation/secret", ".test-scratch/secret",
                     "target/secret", "notes.txt", "src/untracked.rs", ".cargo/config.toml"):
            self.write(name, "DO_NOT_COPY_CANARY\n")
        self.write("src/lib.rs", "allowed source edit\n")
        hashes = runner["snapshot_source"](self.root, self.output)
        self.assertEqual(set(hashes), set(runner["FIXED_INPUTS"]) | {"src/lib.rs"})
        for name, digest in hashes.items():
            self.assertEqual(digest, runner["sha"](self.output / name))
        patch = runner["source_patch"](self.root, hashes)
        self.assertIn(b"allowed source edit", patch)
        self.assertNotIn(b"DO_NOT_COPY_CANARY", patch)
        self.assertEqual({str(p.relative_to(self.output)) for p in self.output.rglob("*") if p.is_file()}, set(hashes))

    def test_actual_extra_example_modules_and_cli_targets_are_included(self):
        for name in ("examples/performance_probe.rs", "examples/support/mod.rs", "tests/text_migration_cli.rs"):
            self.write(name, "// tracked canonical consumer\n")
        self.git("add", "examples", "tests")
        hashes = runner["snapshot_source"](self.root, self.output)
        self.assertIn("examples/support/mod.rs", hashes)
        self.assertIn("examples/performance_probe.rs", hashes)
        self.assertIn("tests/text_migration_cli.rs", hashes)

    def test_unknown_target_asset_or_explicit_cargo_target_fails_closed(self):
        self.write("tests/fixture.secret", "must not silently omit or copy\n")
        self.git("add", "tests")
        with self.assertRaises(RuntimeError):
            runner["snapshot_source"](self.root, self.output)
        self.git("rm", "-f", "tests/fixture.secret")
        self.write("Cargo.toml", '[package]\nname="fixture"\nversion="0.1.0"\n[[test]]\nname="custom"\npath="other/check.rs"\n')
        with self.assertRaises(RuntimeError):
            runner["snapshot_source"](self.root, self.output)

    def test_missing_required_input_fails_closed(self):
        (self.root / "Cargo.lock").unlink()
        with self.assertRaises(RuntimeError):
            runner["snapshot_source"](self.root, self.output)

    def test_symlink_input_fails_closed(self):
        path = self.root / "src/lib.rs"
        path.unlink()
        path.symlink_to(self.root / "notes.txt")
        with self.assertRaises(RuntimeError):
            runner["snapshot_source"](self.root, self.output)

    def test_symlink_parent_even_inside_checkout_fails_closed(self):
        (self.root / "src").rename(self.root / "elsewhere")
        (self.root / "src").symlink_to(self.root / "elsewhere", target_is_directory=True)
        with self.assertRaises(RuntimeError):
            runner["snapshot_source"](self.root, self.output)

    def test_replay_checks_stable_results_and_each_control_not_random_receipts(self):
        memory = "00000000-0000-0000-0000-000000000001"
        for run in range(3):
            root = self.output / f"run-{run}"
            root.mkdir(parents=True)
            store = f"00000000-0000-0000-0001-{run+1:012d}"
            operation = f"00000000-0000-0000-0002-{run+1:012d}"
            identity = {"sourceId": 0, "memoryId": memory}
            runner["save"](root / "fixture.json", {"sources": [identity]})
            runner["save"](root / "replay.json", [])
            for name in ("state-before.json", "state-after.json"):
                runner["save"](root / name, {memory: []})
            for name in ("control-before.json", "control-after.json"):
                runner["save"](root / name, {"storeId": store, "committedGeneration": "5"})
            receipt = {"storeId": store, "operationId": operation, "committedGeneration": "4",
                       "receiptVersion": 1, "replayed": False,
                       "outcome": {"kind": "memoryInserted", "memoryId": memory}}
            ledger = [{**identity, "status": "attempted"},
                      {**identity, "status": "accepted", "receipt": receipt}]
            (root / "turn-ledger.jsonl").write_text("\n".join(map(json.dumps, ledger)) + "\n")
        stable = runner["verify_replays"](self.output, 3)
        self.assertNotIn("turn-ledger.jsonl", stable)
        (self.output / "run-1/replay.json").write_text('{}\n')
        with self.assertRaisesRegex(RuntimeError, "replay mismatch"):
            runner["verify_replays"](self.output, 3)
        runner["save"](self.output / "run-1/replay.json", [])
        ledger[-1]["receipt"]["outcome"]["memoryId"] = store
        (root / "turn-ledger.jsonl").write_text("\n".join(map(json.dumps, ledger)) + "\n")
        with self.assertRaisesRegex(RuntimeError, "receipt outcome/write count"):
            runner["verify_replays"](self.output, 3)
        ledger[-1]["receipt"]["outcome"]["memoryId"] = memory
        ledger[-1]["receipt"]["outcome"]["extraSemanticChange"] = True
        (root / "turn-ledger.jsonl").write_text("\n".join(map(json.dumps, ledger)) + "\n")
        with self.assertRaisesRegex(RuntimeError, "semantics differ"):
            runner["verify_replays"](self.output, 3)
        (self.output / "run-2/control-after.json").write_text('{}\n')
        with self.assertRaisesRegex(RuntimeError, "false query changed control"):
            runner["verify_replays"](self.output, 3)


class ManifestInputTests(unittest.TestCase):
    def test_synthetic_actor_evaluator_separation_and_nonrecursive_hashes(self):
        args = SimpleNamespace(records=100, dimensions=16, k=5, warmup=1, samples=3, seed=7)
        # Metadata values here are not executed; full validation is through the compiled consumer.
        diagnostic = dict.fromkeys(("sourceCommit", "dirtyPatchSha256", "sourceFilesSha256",
                                    "lockfileSha256", "containerImageId", "limits"))
        manifest, actor, evaluator = runner["evaluation_inputs"](args, diagnostic)
        labels = json.loads(evaluator)
        self.assertNotIn(labels["labelCanary"].encode(), actor)
        self.assertEqual(manifest["actorInputs"]["sha256"], runner["hashlib"].sha256(actor).hexdigest())
        self.assertEqual(manifest["evaluatorLabels"]["sha256"], runner["hashlib"].sha256(evaluator).hexdigest())
        self.assertEqual(manifest["dataset"]["datasetRevision"], manifest["actorInputs"]["sha256"])
        self.assertEqual(len(json.loads(actor)["sources"]), 100)
        self.assertEqual(manifest["track"], "manifest-internal-diagnostic-v2")
        self.assertEqual(manifest["manifestVersion"], 1)
        self.assertEqual(manifest["scoring"]["profile"], "synthetic-unique-source-v1")
        self.assertEqual(labels["relevant"]["query10"], [])
        self.assertNotIn("query20", labels["relevant"])
        self.assertEqual(labels["relevant"]["query30"], ["unit-000030", "unit-000099"])
        self.assertNotIn("manifestSha256", manifest)
        self.assertFalse(manifest["capabilities"]["reset"])
        self.assertIsNone(manifest["versions"]["canonicalSchemaVersion"])
        self.assertEqual(manifest["versions"]["canonicalRepresentation"], "legacy-schema-none")
        self.assertIsNone(manifest["dataset"]["thirdPartyLicense"])

    def test_old_supervisor_invocation_cannot_silently_select_new_track(self):
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("offline-evaluation.py")),
                                 "--cache", "/does-not-exist"], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--track", result.stderr)
        self.assertNotIn("Artifacts:", result.stdout)

    def test_old_explicit_track_is_rejected_before_scratch(self):
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("offline-evaluation.py")),
                                 "--track", "manifest-internal-diagnostic-v1", "--cache", "/does-not-exist"],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("Artifacts:", result.stdout)

    def test_bad_plan_rejected_before_any_runner_side_effect(self):
        args = SimpleNamespace(records=99, dimensions=16, k=5, warmup=1, samples=3, seed=7)
        with self.assertRaises(ValueError):
            runner["evaluation_inputs"](args, {})


if __name__ == "__main__":
    unittest.main()
