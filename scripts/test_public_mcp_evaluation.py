#!/usr/bin/env python3
"""Contract/isolation checks; actual executable checks are retained separately."""
import copy
import json
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest import mock

m = runpy.run_path(str(Path(__file__).with_name("public-mcp-evaluation.py")))


class Preflight(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.binary, self.scorer = self.root / "binary", self.root / "scorer"
        self.binary.write_bytes(b"unit-test-binary-never-executed")
        self.scorer.write_bytes(b"unit-test-scorer-never-executed")
        names = ["src/lib.rs", "Cargo.toml", "Cargo.lock", *m["EXTRA"]]
        for name in names:
            p = self.root / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_text("unit test source")
        self.profile = dict(identity="configuration", dimension=4, profileId="ep1:" + "a" * 64)
        self.manifest = dict(manifestVersion=1, sourceMappingVersion=1, traceSchemaVersion=1, track=m["TRACK"],
            versions=m["VERSIONS"], capabilities=m["CAPABILITIES"], sourceCommit="b" * 40, containerImageId="sha256:" + "c" * 64,
            sourceFilesSha256={name: m["digest"]((self.root / name).read_bytes()) for name in names},
            limits=dict(cpus=2, memoryBytes=4294967296), executables=dict(recalld=m["digest"](self.binary.read_bytes()), scorer=m["digest"](self.scorer.read_bytes())),
            activeEmbedding=self.profile, scoringProfile="synthetic-unique-source-v1")
        actor, labels, cache, schedule = m["fixtures"]()
        for key, name, value in (("actorInputs", "actor-inputs.json", actor), ("evaluatorLabels", "evaluator-labels.json", labels), ("replayCache", "evaluator-cache.json", cache), ("evaluatorSchedule", "evaluator-schedule.json", schedule)):
            m["save"](self.root / name, value)
            self.manifest[key] = dict(file=name, sha256=m["digest"]((self.root / name).read_bytes()))
        self.manifest["dataset"] = dict(kind="generated-synthetic", upstreamRevision=None, upstreamUrl=None, thirdPartyLicense=None,
            actorArtifactSha256=self.manifest["actorInputs"]["sha256"], intendedUse="contract-diagnostic-only")

    def validate(self, manifest=None):
        m["save"](self.root / "manifest.json", manifest or self.manifest)
        identity = dict(activeEmbedding=self.profile, sourceCommit=self.manifest["sourceCommit"], containerImageId=self.manifest["containerImageId"],
                        sourceTreeSha256=m["digest"](m["encoded"](self.manifest["sourceFilesSha256"])))
        with mock.patch("subprocess.check_output", return_value=json.dumps(identity).encode()):
            return m["validate"](self.root / "manifest.json", self.binary, self.scorer, self.root)

    def test_actual_version_absence_and_projection(self):
        validated, actor, labels, cache, schedule, env = self.validate()
        self.assertIsNone(validated["versions"]["canonicalSchemaVersion"])
        self.assertEqual(validated["versions"]["canonicalRepresentation"], "unattested-via-public-v1")
        self.assertNotIn(labels["labelCanary"].encode(), m["encoded"](actor) + m["encoded"](cache))
        self.assertNotIn("queries", actor)
        self.assertNotIn(schedule["futureQueryCanary"], m["encoded"](actor).decode())
        self.assertNotIn(schedule["futureQueryCanary"], Path(__file__).with_name("public-mcp-actor.py").read_text())
        self.assertIn("a6-document:amber", cache["entries"])
        self.assertIn("a6-query:amber", cache["entries"])

    def test_unknown_versions_and_controls_fail_closed(self):
        changes = [lambda v: v.update(manifestVersion=2), lambda v: v.update(manifestVersion=True), lambda v: v["versions"].update(publicContractVersion=True),
            lambda v: v.update(sourceCommit="e" * 40),
            lambda v: v["versions"].update(publicContractVersion=2), lambda v: v["versions"].update(canonicalSchemaVersion=2),
            lambda v: v["capabilities"].update(reset=True), lambda v: v["capabilities"].update(barrier=True),
            lambda v: v["capabilities"].update(clock="manual"), lambda v: v.update(unknown=True),
            lambda v: v["sourceFilesSha256"].pop("src/lib.rs"), lambda v: v["executables"].update(recalld="0" * 64)]
        for change in changes:
            with self.subTest(change=change):
                value = copy.deepcopy(self.manifest); change(value)
                with self.assertRaises(ValueError): self.validate(value)
        self.assertFalse((self.root / "data").exists())

    def test_label_perturbation_changes_environment_not_actor_projection(self):
        _, actor, labels, cache, schedule, environment = self.validate()
        labels["labelCanary"] = "DIFFERENT_EVALUATOR_CANARY"
        labels["relevant"] = {f"q{i}": ["turn-3"] for i in range(4)}
        m["save"](self.root / "evaluator-labels.json", labels)
        self.manifest["evaluatorLabels"]["sha256"] = m["digest"]((self.root / "evaluator-labels.json").read_bytes())
        _, actor2, _, cache2, schedule2, environment2 = self.validate()
        self.assertEqual(actor, actor2); self.assertEqual(cache, cache2); self.assertNotEqual(environment, environment2)

    def test_actor_label_injection_rejected_even_with_valid_outer_hash(self):
        actor = json.loads((self.root / "actor-inputs.json").read_text()); actor["answer"] = "LABEL_CANARY"
        m["save"](self.root / "actor-inputs.json", actor)
        self.manifest["actorInputs"]["sha256"] = m["digest"]((self.root / "actor-inputs.json").read_bytes())
        with self.assertRaises(ValueError): self.validate()

    def test_unrecorded_cache_and_foreign_artifact_reference_rejected(self):
        cache = json.loads((self.root / "evaluator-cache.json").read_text()); cache["entries"]["a6-query:unrecorded-query"] = [1,0,0,0]
        m["save"](self.root / "evaluator-cache.json", cache)
        self.manifest["replayCache"]["sha256"] = m["digest"]((self.root / "evaluator-cache.json").read_bytes())
        with self.assertRaises(ValueError): self.validate()
        self.manifest["replayCache"]["file"] = "../evaluator-cache.json"
        with self.assertRaises(ValueError): self.validate()

    def test_actor_code_has_no_evaluator_factory_or_project_import(self):
        import ast
        tree = ast.parse(Path(__file__).with_name("public-mcp-actor.py").read_text())
        imported = {alias.name for node in ast.walk(tree) if isinstance(node, ast.Import) for alias in node.names}
        self.assertEqual(imported, {"argparse", "json", "os", "select", "sys", "time", "uuid"})
        self.assertFalse(any(isinstance(node, ast.ImportFrom) for node in ast.walk(tree)))
        self.assertFalse(any(isinstance(node, ast.Name) and node.id == "fixtures" for node in ast.walk(tree)))

    def test_actor_rejects_future_schedule_and_labels_before_mcp_io(self):
        import subprocess, sys
        for extra in ("futureQueries", "evaluatorLabels"):
            projection = dict(phase="query", startId=0, deliveryOrdinal=1,
                currentQuestion=dict(queryId="current", text="current-only"), expectedMiss=False)
            projection[extra] = "MUST_NOT_REACH_ACTOR"
            run = subprocess.run([sys.executable, str(Path(__file__).with_name("public-mcp-actor.py")),
                "--read-fd", "0", "--write-fd", "1", "--deny-fd", "9"], input=json.dumps(projection), capture_output=True, text=True)
            self.assertNotEqual(run.returncode, 0)
            self.assertEqual(run.stdout, "")
            self.assertIn("unexpected actor projection fields", run.stderr)


if __name__ == "__main__":
    unittest.main()
