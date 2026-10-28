"""Base reuse follows engine inputs, independently of extension and packaging work."""

import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import base_cache


class BaseIdentityTest(unittest.TestCase):
    def test_extension_and_packaging_changes_reuse_base_but_engine_changes_do_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", root], check=True)
            source = root / "custom_wee8/src"
            source.mkdir(parents=True)
            (source / "wasm-feature-policy.h").write_text("policy")
            subprocess.run(["git", "-C", root, "add", "."], check=True)
            subprocess.run(["git", "-C", root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "-qm", "fixture"], check=True)
            head = subprocess.check_output(["git", "-C", root, "rev-parse", "HEAD"], text=True).strip()
            subprocess.run(["git", "-C", root, "update-index", "--add", "--cacheinfo", f"160000,{head},rusty_v8"], check=True)
            subprocess.run(["git", "-C", root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "-qm", "pin"], check=True)
            expected = base_cache.identity(root)
            self.assertEqual(expected, base_cache.identity(root, "HEAD"))
            for name in ("custom_wee8/src/c-api.cc", "custom_wee8/src/c-api.h", "custom_wee8/src/wasm.hh",
                         "custom_wee8/include/wee8.h", "custom_wee8/ext/BUILD.gn", "crates/wee8/src/lib.rs",
                         "tools/base_cache.py", "build.sh", "ci/base_artifacts.py"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("changed extension or packaging")
                self.assertEqual(expected, base_cache.identity(root), name)
            for name in ("custom_wee8/src/wasm-feature-policy.h", "custom_wee8/src/new-engine.h",
                         "custom_wee8/src/nested/c-api.cc", "custom_wee8/BUILD.gn", "tools/native/build.rs", "patches/series"):
                before = base_cache.identity(root)
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("changed engine")
                self.assertNotEqual(before, base_cache.identity(root), name)


@unittest.skipUnless(os.environ.get("WEE8_TEST_GN"), "set WEE8_TEST_GN for real GN integration")
class BaseBuildTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.addCleanup(patch.stopall)
        patch.object(base_cache, "ROOT", self.root).start()
        patch.dict(os.environ, {"ImageVersion": "fixture-image"}).start()
        self.source, self.out = self.root / "rusty_v8", self.root / "out"
        self.base = self.root / "target/wee8-base/fixture"
        self.archive = "wee8_base.lib" if os.name == "nt" else "libwee8_base.a"
        helper = Path(__file__).resolve().parents[1] / "rusty_v8/build/gn_helpers.py"
        gn_helpers = runpy.run_path(str(helper))
        compiler = self.root / r"clang\new\tools $cache"
        inputs = {
            "rusty_v8/.gn": 'buildconfig = "//BUILDCONFIG.gn"\n',
            "rusty_v8/BUILDCONFIG.gn": 'declare_args() { clang_base_path = "" wee8_base_dir = "" is_debug = false }\nset_default_toolchain("//:default")\n',
            "rusty_v8/BUILD.gn": 'print("common/icudtl.dat")\ntoolchain("default") { tool("stamp") { command = "unused {{output}}" } }\ngroup("wee8-base-build") { deps = [ "//v8:wee8_base(//custom_wee8:custom_wee8)" ] }\ngroup("wee8-ext-build") { deps = [ "//custom_wee8/ext:build(//custom_wee8:custom_wee8)" ] }\n',
            "rusty_v8/custom_wee8/BUILD.gn": 'toolchain("custom_wee8") { tool("stamp") { command = "unused {{output}}" } }\n',
            "rusty_v8/custom_wee8/ext/BUILD.gn": 'source_set("wee8_ext") { defines = [ "V8_FIXTURE" ] }\ngroup("build") { deps = [ ":wee8_ext" ] }\n',
            "rusty_v8/v8/BUILD.gn": 'source_set("wee8_base") { defines = [ "V8_FIXTURE" ] }\n',
            "out/args.gn": gn_helpers["ToGNString"]({"clang_base_path": str(compiler), "wee8_base_dir": str(self.base), "is_debug": False}),
            f"out/custom_wee8/obj/v8/{self.archive}": "base archive fixture",
            "out/custom_wee8/gen/layout.h": "generated layout fixture",
            "custom_wee8/src/wasm-feature-policy.h": "policy",
            str(compiler.relative_to(self.root) / "bin" / ("clang-cl.exe" if os.name == "nt" else "clang++")): "compiler fixture",
        }
        for name, text in inputs.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        (self.source / "build").mkdir()
        shutil.copy2(helper, self.source / "build/gn_helpers.py")
        subprocess.run(["git", "init", "-q", self.root], check=True)
        subprocess.run(["git", "-C", self.root, "add", "."], check=True)
        subprocess.run(["git", "-C", self.root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "fixture"], check=True)
        self.args = SimpleNamespace(target="fixture", out=self.out, gn=os.environ["WEE8_TEST_GN"])
        base_cache.gn_output(self.args, "gen", self.out)

    def test_real_gn_configuration_and_extension_reuse_without_sdk_export(self):
        noisy = base_cache.gn_output(self.args, "args", self.out, "--list=clang_base_path", "--json")
        self.assertTrue(noisy.startswith("common/icudtl.dat\n"), noisy)
        base_cache.prepare_base(self.args, self.base)
        base_cache.save_base(self.args, self.base)
        before = (self.base / self.archive).read_bytes()
        (self.root / "custom_wee8/src/c-api.cc").write_text("changed extension")
        base_cache.verify_extension(self.args, self.base)
        self.assertEqual((self.base / self.archive).read_bytes(), before)
        self.assertEqual({p.name for p in self.base.iterdir()}, {self.archive, "gen", "base.json"})

    def test_mismatched_toolchain_and_tampered_headers_are_rejected(self):
        base_cache.prepare_base(self.args, self.base)
        base_cache.save_base(self.args, self.base)
        compiler = self.root / r"clang\new\tools $cache" / "bin" / ("clang-cl.exe" if os.name == "nt" else "clang++")
        compiler.write_text("different compiler")
        with self.assertRaisesRegex(ValueError, "compiler or GN configuration differs"):
            base_cache.verify_extension(self.args, self.base)
        compiler.write_text("compiler fixture")
        (self.base / "gen/layout.h").write_text("wrong layout")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            base_cache.verify_extension(self.args, self.base)

    def test_engine_dependency_is_rejected_before_extension_compilation(self):
        base_cache.prepare_base(self.args, self.base)
        base_cache.save_base(self.args, self.base)
        p = self.source / "custom_wee8/ext/BUILD.gn"
        p.write_text(p.read_text().replace('deps = [ ":wee8_ext" ]', 'deps = [ ":wee8_ext", "//v8:wee8_base" ]'))
        with self.assertRaisesRegex(ValueError, "depends on engine compilation"):
            base_cache.verify_extension(self.args, self.base)

    def test_failed_archive_merge_preserves_saved_base_for_the_next_attempt(self):
        base_cache.prepare_base(self.args, self.base)
        base_cache.save_base(self.args, self.base)
        ext = self.out / "custom_wee8/obj/custom_wee8/ext" / ("wee8_ext.lib" if os.name == "nt" else "libwee8_ext.a")
        ext.parent.mkdir(parents=True, exist_ok=True)
        ext.write_bytes(b"extension fixture")
        original = base_cache.verify(self.base, "fixture")
        with patch.object(base_cache.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "archiver")):
            with self.assertRaises(subprocess.CalledProcessError):
                base_cache.merge_archives(self.args, self.base)
        self.assertEqual(base_cache.verify(self.base, "fixture"), original)
        base_cache.verify_extension(self.args, self.base)


if __name__ == "__main__":
    unittest.main()
