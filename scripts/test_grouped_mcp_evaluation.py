#!/usr/bin/env python3
"""Synthetic grouping integrity; actual public episodes are validated separately."""
import copy
import json
from types import SimpleNamespace
import unittest
from unittest import mock

from test_public_mcp_evaluation import Preflight, m as h

g = h["GROUPED"]


class Grouping(unittest.TestCase):
    def test_permutations_transitive_relations_and_exact_history_boundary(self):
        source = g["registry"](h)
        groups, splits = g["maps"](source, h)
        self.assertEqual(len(groups["groups"]), 5)
        self.assertEqual(splits["counts"], dict(train=3, development=1, test=1))
        self.assertEqual(groups["episodes"]["episode-0"], groups["episodes"]["episode-1"])
        self.assertEqual(groups["episodes"]["episode-2"], groups["episodes"]["episode-3"])
        shuffled = copy.deepcopy(source); shuffled["episodes"].reverse()
        for episode in shuffled["episodes"]:
            episode["sources"].reverse()
        self.assertEqual(g["maps"](shuffled, h), (groups, splits))
        aliased = copy.deepcopy(source)
        for unit in aliased["episodes"][1]["sources"]:
            unit["sourceId"] += "-alias"; unit["sourceTimestamp"] += 99
        self.assertEqual(g["maps"](aliased, h), (groups, splits))
        chained = copy.deepcopy(source)
        chained["episodes"][4]["aliases"] = ["declared-shared-history"]
        chained["episodes"][4]["userIds"] += chained["episodes"][5]["userIds"]
        linked, _ = g["maps"](chained, h)
        self.assertEqual(set(linked["groups"][linked["episodes"]["episode-2"]]), {f"episode-{i}" for i in (2, 3, 4, 5)})
        chained["episodes"].reverse()
        self.assertEqual(g["maps"](chained, h)[0], linked)
        changed = copy.deepcopy(source)
        changed["episodes"][1]["sources"][0]["namespace"] = "different"
        self.assertNotEqual(g["maps"](changed, h)[0]["episodes"]["episode-0"], g["maps"](changed, h)[0]["episodes"]["episode-1"])

    def test_unknown_fields_duplicates_and_content_hashes_reject(self):
        for change in (lambda v: v.update(registryVersion=True),
                       lambda v: v["episodes"].append(v["episodes"][0]),
                       lambda v: v["episodes"][0].update(answer="label"),
                       lambda v: v["episodes"][0]["sources"][0].update(contentSha256="0" * 64),
                       lambda v: v["episodes"][0]["sources"].append(v["episodes"][0]["sources"][0])):
            value = g["registry"](h); change(value)
            with self.assertRaises(ValueError): g["maps"](value, h)


class CohortPreflight(unittest.TestCase):
    def setUp(self):
        self.legacy = Preflight(); self.legacy.setUp(); self.addCleanup(self.legacy.doCleanups)
        self.root = self.legacy.root
        self.identity = dict(activeEmbedding=self.legacy.profile, sourceCommit=self.legacy.manifest["sourceCommit"],
            containerImageId=self.legacy.manifest["containerImageId"], sourceTreeSha256=h["digest"](h["encoded"](self.legacy.manifest["sourceFilesSha256"])))
        build = {key: self.legacy.manifest[key] for key in ("sourceCommit", "sourceFilesSha256", "containerImageId")}
        h["save"](self.root / "build.json", build)
        args = SimpleNamespace(binary=self.legacy.binary, scorer=self.legacy.scorer, build=self.root / "build.json", out=self.root / "cohort")
        with mock.patch("subprocess.check_output", return_value=json.dumps(self.identity).encode()):
            g["prepare"](args, h)
        self.args = SimpleNamespace(binary=args.binary, scorer=args.scorer, source_root=self.root,
                                    manifest=args.out / "manifest.json", out=self.root / "run")
        self.manifest = json.loads(self.args.manifest.read_text())

    def validate(self):
        h["save"](self.args.manifest, self.manifest)
        with mock.patch("subprocess.check_output", return_value=json.dumps(self.identity).encode()):
            return g["validate"](self.args, h)

    def rewrite(self, ref, value):
        path = self.args.manifest.parent / ref["file"]
        h["save"](path, value); ref["sha256"] = h["digest"](path.read_bytes())

    def test_valid_roster_and_labels_do_not_select_groups_or_actor_inputs(self):
        manifest, bundles, roster, environment = self.validate()
        self.assertEqual(len(bundles), 7); self.assertEqual(len(roster), 7)
        self.assertIsNone(manifest["versions"]["canonicalSchemaVersion"])
        maps = {key: copy.deepcopy(manifest[key]) for key in ("sourceRegistry", "groupMap", "splitMap")}
        for entry in self.manifest["episodes"]:
            ref = entry["artifacts"]["evaluatorLabels"]
            labels = json.loads((self.args.manifest.parent / ref["file"]).read_text())
            labels.update(labelCanary="CHANGED_LABEL_CANARY", relevant={"q0": ["turn-3"]})
            self.rewrite(ref, labels)
        changed, other, other_roster, other_environment = self.validate()
        self.assertEqual(roster, other_roster); self.assertNotEqual(environment, other_environment)
        self.assertEqual(maps, {key: changed[key] for key in maps})
        for before, after in zip(bundles, other):
            self.assertEqual(before[1], after[1]); self.assertEqual(before[3:5], after[3:5])
            self.assertNotIn("CHANGED_LABEL_CANARY", h["encoded"](after[1]).decode())
            self.assertNotIn("groupMap", after[1]); self.assertNotIn("split", after[1])

    def test_rehashed_cross_split_duplicate_is_not_a_hash_error(self):
        ref = self.manifest["groupMap"]
        groups = json.loads((self.args.manifest.parent / ref["file"]).read_text())
        prior = groups["episodes"]["episode-1"]
        groups["groups"][prior].remove("episode-1")
        groups["groups"]["f" * 64] = ["episode-1"]; groups["episodes"]["episode-1"] = "f" * 64
        self.rewrite(ref, groups)
        split_ref = self.manifest["splitMap"]
        splits = json.loads((self.args.manifest.parent / split_ref["file"]).read_text())
        splits["groups"]["f" * 64] = "test" if splits["groups"][prior] != "test" else "train"
        self.rewrite(split_ref, splits)
        with self.assertRaisesRegex(ValueError, "group map differs from source histories"):
            self.validate()
        self.assertFalse(self.args.out.exists())

    def test_whole_cohort_rejects_last_cache_before_first_episode(self):
        entry = self.manifest["episodes"][-1]; ref = entry["artifacts"]["replayCache"]
        cache = json.loads((self.args.manifest.parent / ref["file"]).read_text())
        cache["entries"]["a6-query:unrecorded-query"] = [1., 0., 0., 0.]
        self.rewrite(ref, cache); h["save"](self.args.manifest, self.manifest)
        consumer = mock.Mock(side_effect=AssertionError("must not start an episode"))
        with mock.patch.dict(h, run_episode=consumer), mock.patch("subprocess.check_output", return_value=json.dumps(self.identity).encode()):
            with self.assertRaisesRegex(ValueError, "unsupported synthetic actor/cache fixture"):
                g["run"](self.args, h)
        consumer.assert_not_called(); self.assertFalse(self.args.out.exists())

    def test_versions_missing_duplicate_roster_and_old_track_boundary(self):
        original = copy.deepcopy(self.manifest)
        for change in (lambda v: v.update(groupMapVersion=True), lambda v: v.update(splitMapVersion=2),
                       lambda v: v.update(aggregateProfile="unknown"), lambda v: v["episodes"].pop(),
                       lambda v: v["episodes"].__setitem__(-1, v["episodes"][0]),
                       lambda v: v["capabilities"].update(reset=True)):
            self.manifest = copy.deepcopy(original); change(self.manifest)
            with self.assertRaises(ValueError): self.validate()
            self.assertFalse(self.args.out.exists())
        old = copy.deepcopy(self.legacy.manifest); old["groupMapVersion"] = 1
        with self.assertRaises(ValueError): self.legacy.validate(old)
        self.assertEqual(self.legacy.validate()[0]["track"], h["TRACK"])

    def test_failed_episode_retains_roster_and_cannot_produce_aggregate(self):
        consumer = mock.Mock(side_effect=RuntimeError("injected episode failure"))
        with mock.patch.dict(h, run_episode=consumer), mock.patch("subprocess.check_output", return_value=json.dumps(self.identity).encode()):
            with self.assertRaisesRegex(RuntimeError, "injected episode failure"):
                g["run"](self.args, h)
        status = json.loads((self.args.out / "episode-status.json").read_text())
        self.assertEqual((status["planned"], status["attempted"], status["succeeded"], status["failed"], status["unattempted"]), (7, 1, 0, 1, 6))
        self.assertEqual(status["status"], "failed")
        self.assertEqual(status["episodes"][0]["status"], "failed")
        self.assertEqual(len(status["episodes"]), 7)
        self.assertTrue(all(row["status"] == "unattempted" for row in status["episodes"][1:]))
        self.assertFalse((self.args.out / "grouped-scores.json").exists())


if __name__ == "__main__":
    unittest.main()
