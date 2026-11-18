"""Fail-closed collection checks and a small isolated synthetic Cargo smoke test."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("shards", Path(__file__).with_name("test_shards.py"))
shards = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shards)


def reports(names=None):
    inventory = {
        "lib:recalld": {
            "kind": "lib",
            "names": sorted(names if names is not None else [
                "storage::a", "storage::ab", "api::test", "search::test", "new_module::test", "ignored"]),
            "ignored": ["ignored"] if names is None else [],
        },
        "bin:recalld": {"kind": "bin", "names": [], "ignored": []},
        "test:cli": {"kind": "test", "names": ["cli"], "ignored": []},
    }
    output = []
    for group in shards.GROUPS:
        evidence = {}
        for key, harness in sorted(inventory.items()):
            selected = shards.selection(harness, group)
            evidence[key] = {"selected": selected,
                             "results": {n: "ignored" if n in harness["ignored"] else "ok" for n in selected},
                             "executed": bool(selected),
                             "binary_sha256": "a" * 64 if selected else None}
        output.append({"source": "source", "os": "os", "compiler": "rustc", "cargo": "cargo",
                       "group": group, "groups": list(shards.GROUPS), "success": True,
                       "inventory": copy.deepcopy(inventory), "evidence": evidence,
                       "compiled_examples": ["offline_eval"],
                       "docs": {"names": ["doc"], "ignored": ["doc"], "results": {"doc": "ignored"}}
                       if group == shards.DEFAULT_GROUP else None})
    return output


class PartitionTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("cargo"), "Cargo not installed; collection-only local checks")
    def test_real_cargo_target_inventory_exact_filters_docs_and_compile_only_examples(self):
        runner = Path(shards.__file__).resolve()
        with tempfile.TemporaryDirectory(prefix="ci-shard-smoke-") as temporary:
            root = Path(temporary)
            files = {
                "Cargo.toml": '[package]\nname="ci-shard-smoke"\nversion="0.1.0"\nedition="2024"\n',
                "src/lib.rs": '''//! ```ignore
//! // Ignored docs stay ignored.
//! ```
//! ```
//! assert_eq!(2 + 2, 4);
//! ```
mod storage {
    #[test] fn a() {}
    #[test] fn ab() {}
}
mod api { #[test] fn protocol() {} }
mod search { #[test] fn embedding() {} }
mod new_module { #[test] fn future() {} }
#[test] #[should_panic] fn panic_expected() { panic!("expected"); }
#[test] #[ignore = "fixture generator"] fn fixture() { panic!("must not run"); }
#[test] fn z() {}
''',
                "src/main.rs": 'fn main() {}\n',
                "tests/cli.rs": '#[test] fn cli() { assert!(std::path::Path::new(env!("CARGO_BIN_EXE_ci-shard-smoke")).is_file()); }\n',
                "examples/compile_only.rs": 'fn main() { panic!("must not execute"); }\n#[test] fn example() { panic!("not baseline"); }\n',
            }
            for name, content in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            env = dict(os.environ, CARGO_NET_OFFLINE="true")
            env.pop("CARGO_TARGET_DIR", None)  # Never touch a shared target.
            def invoke(args):
                result = subprocess.run(args, cwd=root, env=env, text=True,
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
                self.assertEqual(result.returncode, 0, result.stdout)
                return result.stdout
            invoke(["git", "init", "-q"])
            invoke(["git", "add", "."])
            invoke(["git", "-c", "user.name=CI fixture", "-c", "user.email=ci@example.invalid",
                    "commit", "-qm", "Synthetic runner fixture"])
            for case in ("semantic", "empty-groups"):
                if case == "empty-groups":
                    (root / "src/lib.rs").write_text("#[test] fn new_root_test() {}\n")
                    invoke(["git", "add", "src/lib.rs"])
                    invoke(["git", "-c", "user.name=CI fixture", "-c", "user.email=ci@example.invalid",
                            "commit", "-qm", "Empty semantic groups fixture"])
                directory = root / "results" / case
                for group in shards.GROUPS:
                    invoke([sys.executable, str(runner), "run", "--group", group,
                            "--os", "smoke", "--directory", str(directory / group)])
                    report = json.loads((directory / group / "report.json").read_text())
                    library = report["evidence"]["lib:ci_shard_smoke"]
                    if case == "semantic" and group == "storage":
                        self.assertEqual(library["selected"], ["storage::a", "storage::ab"])
                    if case == "empty-groups" and group != "runtime":
                        self.assertEqual(library["selected"], [])
                        self.assertFalse(library["executed"])
                        self.assertFalse(list((directory / group).glob("*.run.log")))
                    self.assertFalse(report["evidence"]["bin:ci-shard-smoke"]["executed"])
                invoke([sys.executable, str(runner), "verify", "--os", "smoke",
                        "--source", invoke(["git", "rev-parse", "HEAD"]).strip(),
                        "--directory", str(directory)])

    def test_complete_collection(self):
        shards.verify(reports(), "source", "os")

    def test_rejects_missing_duplicate_foreign_failed_and_drift(self):
        mutations = [
            lambda r: r.pop(),
            lambda r: r.append(r[0]),
            lambda r: r[1].update(group="storage"),
            lambda r: r[0].update(source="foreign"),
            lambda r: r[0].update(os="foreign"),
            lambda r: r[0].update(success=False),
            lambda r: r[0].update(compiler="other"),
            lambda r: r[0].update(compiled_examples=[]),
            lambda r: r[0]["inventory"]["lib:recalld"]["names"].append("new"),
            lambda r: r[0]["evidence"].pop("test:cli"),
            lambda r: r[0]["evidence"]["lib:recalld"]["results"].clear(),
            lambda r: r[0]["evidence"]["lib:recalld"]["selected"].append("ab"),
            lambda r: r[0]["evidence"]["lib:recalld"]["results"].update({"storage::a": "FAILED"}),
            lambda r: r[0]["evidence"]["lib:recalld"].update(binary_sha256=""),
            lambda r: r[3].update(docs=None),
            lambda r: r[1].update(docs=r[3]["docs"]),
            lambda r: r[3]["docs"].update(results={}),
        ]
        for mutate in mutations:
            with self.subTest(mutation=mutations.index(mutate)):
                data = reports()
                mutate(data)
                with self.assertRaises(ValueError):
                    shards.verify(data, "source", "os")

    def test_listing_includes_ignored_and_rejects_partial_duplicate_or_bench(self):
        self.assertEqual(shards.listing("a: test\nab: test\n2 tests, 0 benchmarks\n"), ["a", "ab"])
        self.assertEqual(shards.listing("0 tests, 0 benchmarks\n"), [])
        for text in ["a: test\n", "a: test\na: test\n2 tests, 0 benchmarks\n",
                     "a: test\n2 tests, 0 benchmarks\n", "0 tests, 1 benchmark\n"]:
            with self.assertRaises(ValueError):
                shards.listing(text)

    def test_result_collection_rejects_missing_extra_duplicate_ignored_or_failed(self):
        good = "test a ... ok\ntest ignored ... ignored, fixture helper\n\n" \
               "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out; finished in 0.01s\n"
        self.assertEqual(shards.results(good, ["a", "ignored"], ["ignored"]),
                         {"a": "ok", "ignored": "ignored"})
        self.assertEqual(shards.results(good.replace("test a ...", "test a - should panic ..."),
                                       ["a", "ignored"], ["ignored"]),
                         {"a": "ok", "ignored": "ignored"})
        for text in [good.replace("test a ... ok\n", ""), good + "test a ... ok\n",
                     good + "test extra ... ok\n", good.replace("a ... ok", "a ... FAILED"),
                     good.replace("1 passed", "2 passed"), good.replace("test result:", "no summary:")]:
            with self.assertRaises(ValueError):
                shards.results(text, ["a", "ignored"], ["ignored"])

    def test_semantic_ownership_is_stable_and_nonlib_targets_belong_to_runtime(self):
        expected = {
            "storage": ["backup::restore", "serialization::format", "storage::unix::test"],
            "api": ["api::http", "canonical_list::page", "catalog_page::cursor",
                    "mcp::tool", "native_catalog_adapter::page"],
            "search": ["cache::test", "embedding::test", "graph::test",
                       "rif::test", "search::test"],
            "runtime": ["cli::test", "daemon::test", "decay::test", "new_module::test", "root_test",
                        "storage_similar::test", "system::shutdown"],
        }
        names = sorted(n for group in expected.values() for n in group)
        for extra in ([], ["api::windows::test", "unknown::new_test"]):
            harness = {"kind": "lib", "names": sorted(names + extra)}
            partitions = {g: shards.selection(harness, g) for g in shards.GROUPS}
            self.assertEqual(sorted(n for part in partitions.values() for n in part), harness["names"])
            for group, owned in expected.items():
                self.assertEqual([n for n in partitions[group] if n not in extra], owned)
        for kind in ("bin", "test"):
            harness = {"kind": kind, "names": names}
            for group in shards.GROUPS:
                self.assertEqual(shards.selection(harness, group), names if group == "runtime" else [])

    def test_empty_groups_never_execute_unfiltered_harnesses(self):
        for names in ([], ["new_module::test"]):
            data = reports(names)
            shards.verify(data, "source", "os")
            for report in data:
                evidence = report["evidence"]["lib:recalld"]
                self.assertEqual(evidence["executed"], bool(evidence["selected"]))
            data[0]["evidence"]["lib:recalld"].update(executed=True, binary_sha256="a" * 64)
            with self.assertRaises(ValueError):
                shards.verify(data, "source", "os")

    def test_rejects_reassigned_and_duplicate_coverage(self):
        for duplicate in (False, True):
            data = reports()
            origin = data[0]["evidence"]["lib:recalld"]
            other = data[1]["evidence"]["lib:recalld"]
            other["selected"].append("storage::a")
            other["results"]["storage::a"] = "ok"
            if not duplicate:
                origin["selected"].remove("storage::a")
                del origin["results"]["storage::a"]
            with self.assertRaises(ValueError):
                shards.verify(data, "source", "os")


if __name__ == "__main__":
    unittest.main()
