"""Small fail-closed tests for CI collection, independent of Rust compilation."""
import copy
import importlib.util
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


def reports():
    inventory = {
        "lib:recalld": {"kind": "lib", "names": ["a", "ab", "c", "d", "ignored"], "ignored": ["ignored"]},
        "bin:recalld": {"kind": "bin", "names": [], "ignored": []},
        "test:cli": {"kind": "test", "names": ["cli"], "ignored": []},
    }
    output = []
    for index in range(shards.SHARDS):
        evidence = {}
        for ordinal, (key, harness) in enumerate(sorted(inventory.items())):
            selected = shards.selection(harness, index, ordinal)
            evidence[key] = {"selected": selected,
                             "results": {n: "ignored" if n in harness["ignored"] else "ok" for n in selected},
                             "executed": harness["kind"] == "lib" or ordinal % shards.SHARDS == index,
                             "binary_sha256": "a" * 64}
        output.append({"source": "source", "os": "os", "compiler": "rustc", "cargo": "cargo",
                       "shard": index, "shards": shards.SHARDS, "success": True,
                       "inventory": copy.deepcopy(inventory), "evidence": evidence,
                       "compiled_examples": ["offline_eval"],
                       "docs": {"names": ["doc"], "ignored": ["doc"], "results": {"doc": "ignored"}}
                       if index == 0 else None})
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
#[test] fn a() {}
#[test] fn ab() {}
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
            for index in range(shards.SHARDS):
                invoke([sys.executable, str(runner), "run", "--shard", str(index),
                        "--os", "smoke", "--directory", str(root / "results" / str(index))])
            invoke([sys.executable, str(runner), "verify", "--os", "smoke",
                    "--source", invoke(["git", "rev-parse", "HEAD"]).strip(),
                    "--directory", str(root / "results")])

    def test_complete_collection(self):
        shards.verify(reports(), "source", "os")

    def test_rejects_missing_duplicate_foreign_failed_and_drift(self):
        mutations = [
            lambda r: r.pop(),
            lambda r: r.append(r[0]),
            lambda r: r[1].update(shard=0),
            lambda r: r[0].update(source="foreign"),
            lambda r: r[0].update(os="foreign"),
            lambda r: r[0].update(success=False),
            lambda r: r[0].update(compiler="other"),
            lambda r: r[0].update(compiled_examples=[]),
            lambda r: r[0]["inventory"]["lib:recalld"]["names"].append("new"),
            lambda r: r[0]["evidence"].pop("test:cli"),
            lambda r: r[0]["evidence"]["lib:recalld"]["results"].clear(),
            lambda r: r[0]["evidence"]["lib:recalld"]["selected"].append("ab"),
            lambda r: r[0]["evidence"]["lib:recalld"]["results"].update(a="FAILED"),
            lambda r: r[0]["evidence"]["lib:recalld"].update(binary_sha256=""),
            lambda r: r[0].update(docs=None),
            lambda r: r[1].update(docs=r[0]["docs"]),
            lambda r: r[0]["docs"].update(results={}),
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

    def test_partition_exactness_for_platform_specific_inventory_and_new_tests(self):
        for size in [4, 5, 1768, 1769, 1809]:
            names = [f"module::test_{i:05}" for i in range(size)]
            harness = {"kind": "lib", "names": names, "ignored": []}
            partitions = [shards.selection(harness, i, 0) for i in range(4)]
            self.assertEqual(sorted(n for part in partitions for n in part), names)
            self.assertLessEqual(max(map(len, partitions)) - min(map(len, partitions)), 1)


if __name__ == "__main__":
    unittest.main()
