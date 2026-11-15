"""Narrow synthetic native consumer fixture/manifest contracts; runtime proof is separate."""
import copy
from pathlib import Path
import runpy
import unittest

H=runpy.run_path(str(Path(__file__).with_name("public-mcp-evaluation.py")))
N=H["NATIVE"]
class NativePrefixTests(unittest.TestCase):
    def test_named_track_requires_real_schema_and_separate_version_domains(self):
        versions, caps=H["track_profile"](N["TRACK"])
        self.assertEqual(versions["publicContractVersion"],1)
        self.assertEqual(versions["canonicalSchemaVersion"],2)
        self.assertEqual(versions["logicalHashContract"],"native-logical-export-v1")
        self.assertFalse(caps["nativeWrites"])
        self.assertFalse(caps["nativeSweep"])
        self.assertFalse(caps["fullA6"])
        self.assertFalse(caps["ordinaryNativeStartup"])
        self.assertFalse(caps["independentNativeGenesis"])
    def test_future_change_preserves_prior_actor_inputs_and_source_vectors(self):
        a,l,c,s=N["fixtures"](H["fixtures"])
        a2,l2,c2,s2=N["fixtures"](H["fixtures"],True)
        self.assertEqual(a,a2);self.assertEqual(l,l2)
        first=next(i for i,v in enumerate(s["steps"]) if s["futureQueryCanary"] in str(v))
        self.assertEqual(s["steps"][:first],s2["steps"][:first])
        self.assertNotEqual(s["steps"][first],s2["steps"][first])
        self.assertEqual(c["entries"]["a6-query:"+s["queries"][-1]["text"]],c2["entries"]["a6-query:"+s2["queries"][-1]["text"]])
        actor_code=Path(__file__).with_name("public-mcp-actor.py").read_text()
        self.assertNotIn(s["futureQueryCanary"],actor_code)
        self.assertNotIn(l["labelCanary"],actor_code)
        self.assertNotIn("fixtures(",actor_code)
    def test_native_manifest_rejects_unsupported_guarantees(self):
        # Reuse the existing preflight fixture; executable identity is mocked only
        # in this unit test. Actual binaries/profiles are checked by the episode.
        base=runpy.run_path(str(Path(__file__).with_name("test_public_mcp_evaluation.py")))["Preflight"]("runTest")
        base.setUp();self.addCleanup(base.doCleanups)
        base.manifest["track"]=N["TRACK"]
        base.manifest["versions"],base.manifest["capabilities"]=H["track_profile"](N["TRACK"])
        base.manifest["executables"]["ownedEvaluator"]=base.manifest["executables"].pop("recalld")
        fixture=base.binary.with_name("native_prefix_fixture");fixture.write_bytes(b"unit-fixture-never-executed")
        base.manifest["executables"]["nativePrefixFixture"]=H["digest"](fixture.read_bytes())
        a,l,c,sc=N["fixtures"](H["fixtures"])
        for key,name,value in (("actorInputs","actor-inputs.json",a),("evaluatorLabels","evaluator-labels.json",l),("replayCache","evaluator-cache.json",c),("evaluatorSchedule","evaluator-schedule.json",sc)):
            H["save"](base.root/name,value);base.manifest[key]["sha256"]=H["digest"]((base.root/name).read_bytes())
        base.manifest["dataset"]["actorArtifactSha256"]=base.manifest["actorInputs"]["sha256"]
        base.validate()
        for change in (lambda v:v.update(manifestVersion=2),lambda v:v["versions"].update(canonicalSchemaVersion=None),
                       lambda v:v["versions"].update(canonicalSchemaVersion=0),lambda v:v["versions"].update(publicContractVersion=2),
                       lambda v:v["capabilities"].update(fullA6=True),lambda v:v["capabilities"].update(reset=True),
                       lambda v:v["capabilities"].update(clock="frozen"),lambda v:v["capabilities"].update(deterministicRng=True),
                       lambda v:v["capabilities"].update(nativeWrites=True),lambda v:v["capabilities"].update(nativeSweep=True),
                       lambda v:v["capabilities"].update(ordinaryNativeStartup=True),lambda v:v["capabilities"].update(rif="v1"),
                       lambda v:v["executables"].pop("nativePrefixFixture")):
            candidate=copy.deepcopy(base.manifest);change(candidate)
            with self.assertRaises(ValueError):base.validate(candidate)

    def test_observation_negative_requests_are_not_rewritten(self):
        *_,schedule=N["fixtures"](H["fixtures"])
        mappings={f"turn-{i}":f"id-{i}" for i in range(4)}
        for step in schedule["steps"]:
            original=copy.deepcopy(step)
            projection=H["CONFORMANCE"]["projection"](step,mappings,100,1)
            self.assertEqual(step,original)
            self.assertEqual(projection["currentAction"]["arguments"].get("recordAccess"),step["arguments"].get("recordAccess"))
            self.assertEqual(set(projection),{"phase","startId","deliveryOrdinal","currentAction"})

if __name__=="__main__":unittest.main()
