"""Named native sweep admission and current-step fixture boundaries."""
import copy
from pathlib import Path
import runpy
import unittest

H = runpy.run_path(str(Path(__file__).with_name("public-mcp-evaluation.py")))
N = H["NATIVE_SWEEP"]


class NativeSweepTests(unittest.TestCase):
    def test_explicit_manual_origin_is_independent_of_native_schema(self):
        versions, caps = H["track_profile"](N["TRACK"])
        self.assertEqual(versions["canonicalSchemaVersion"], 2)
        self.assertEqual(versions["publicContractVersion"], 1)
        self.assertEqual(versions["sweepOriginalContractVersion"], 1)
        self.assertTrue(caps["nativeSweep"])
        for key in ("nativeWrites", "nativePhysicalCompaction", "automaticSweep", "fullA6", "ordinaryNativeStartup", "independentNativeGenesis"):
            self.assertFalse(caps[key])
        self.assertFalse(H["track_profile"](H["NATIVE"]["TRACK"])[1]["nativeSweep"])

    def test_future_schedule_and_controls_never_enter_actor_projection(self):
        a, l, c, schedule = N["fixtures"](H["fixtures"], H["NATIVE"])
        a2, l2, c2, changed = N["fixtures"](H["fixtures"], H["NATIVE"], True)
        self.assertEqual(a, a2); self.assertEqual(l, l2)
        self.assertEqual([s["expectedPhase"] for s in schedule["steps"] if s.get("control") == "sweep"], ["summary", "ghost", "deleted"])
        first = next(i for i, s in enumerate(schedule["steps"]) if schedule["futureQueryCanary"] in str(s))
        self.assertEqual(schedule["steps"][:first], changed["steps"][:first])
        self.assertNotEqual(schedule["steps"][first], changed["steps"][first])
        mappings = {f"turn-{i}": f"id-{i}" for i in range(4)}
        for step in schedule["steps"]:
            if "control" in step:
                continue
            projection = H["CONFORMANCE"]["projection"](step, mappings, 100, 1)
            self.assertEqual(set(projection), {"phase", "startId", "deliveryOrdinal", "currentAction"})
            self.assertNotIn("expectedPhase", projection["currentAction"])
            self.assertNotIn(l["labelCanary"], str(projection))
            self.assertEqual(projection["currentAction"]["arguments"].get("recordAccess"), step["arguments"].get("recordAccess"))

    def test_preflight_rejects_false_compaction_writer_and_origin_claims(self):
        base = runpy.run_path(str(Path(__file__).with_name("test_public_mcp_evaluation.py")))["Preflight"]("runTest")
        base.setUp(); self.addCleanup(base.doCleanups)
        base.manifest["track"] = N["TRACK"]
        base.manifest["versions"], base.manifest["capabilities"] = H["track_profile"](N["TRACK"])
        base.manifest["executables"]["ownedEvaluator"] = base.manifest["executables"].pop("recalld")
        fixture = base.binary.with_name("native_prefix_fixture"); fixture.write_bytes(b"unit-fixture-never-executed")
        base.manifest["executables"]["nativePrefixFixture"] = H["digest"](fixture.read_bytes())
        a, l, c, schedule = N["fixtures"](H["fixtures"], H["NATIVE"])
        for key, name, value in (("actorInputs", "actor-inputs.json", a), ("evaluatorLabels", "evaluator-labels.json", l), ("replayCache", "evaluator-cache.json", c), ("evaluatorSchedule", "evaluator-schedule.json", schedule)):
            H["save"](base.root/name, value); base.manifest[key]["sha256"] = H["digest"]((base.root/name).read_bytes())
        base.manifest["dataset"]["actorArtifactSha256"] = base.manifest["actorInputs"]["sha256"]
        base.validate()
        for change in (lambda v: v["capabilities"].update(nativePhysicalCompaction=True),
                       lambda v: v["capabilities"].update(nativeWrites=True),
                       lambda v: v["capabilities"].update(sweepDecayRateMultiplier=0),
                       lambda v: v["capabilities"].update(automaticSweep=True),
                       lambda v: v["versions"].update(sweepOriginalContractVersion=2),
                       lambda v: v["versions"].update(canonicalSchemaVersion=1)):
            candidate = copy.deepcopy(base.manifest); change(candidate)
            with self.assertRaises(ValueError):
                base.validate(candidate)


if __name__ == "__main__":
    unittest.main()
