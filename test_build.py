"""Exercise source pinning and artifact packaging without compiling V8."""

import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import build


def git(directory, *args):
    return subprocess.check_output(
        ["git", "-c", "user.name=Build test", "-c", "user.email=build@example.invalid", *args],
        cwd=directory, text=True, stderr=subprocess.STDOUT,
    ).strip()


class BuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.environment = patch.dict(os.environ, {"GIT_ALLOW_PROTOCOL": "file"})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        v8 = self.root / "upstream-v8"
        v8.mkdir()
        git(v8, "init", "-b", "main")
        (v8 / "LICENSE").write_text("V8 notice\n")
        git(v8, "add", ".")
        git(v8, "commit", "-m", "fixture")
        v8_commit = git(v8, "rev-parse", "HEAD")
        upstream = self.root / "upstream"
        upstream.mkdir()
        git(upstream, "init", "-b", "main")
        (upstream / "LICENSE").write_text("Binding notice\n")
        (upstream / "binding.cc").write_text("int intl = 1;\n")
        git(upstream, "submodule", "add", v8.as_uri(), "v8")
        git(upstream, "add", ".")
        git(upstream, "commit", "-m", "fixture")
        upstream_commit = git(upstream, "rev-parse", "HEAD")
        (self.root / "patches").mkdir()
        (upstream / "binding.cc").write_text("int intl = 0;\n")
        (self.root / "patches/no-icu.patch").write_text(git(upstream, "diff") + "\n")
        git(upstream, "restore", "binding.cc")
        (self.root / "build.py").write_text("test build entry point\n")
        self.recipe = {
            "release": "v0.106.0-no-icu.1",
            "rusty_v8": {"repository": upstream.as_uri(), "commit": upstream_commit},
            "v8": {"commit": v8_commit},
            "gn_args": ["v8_enable_i18n_support=false", "icu_use_data_file=false"],
            "patches": ["patches/no-icu.patch"],
        }
        (self.root / "versions.json").write_text(json.dumps(self.recipe))
        self.target = "aarch64-apple-darwin"

    def test_prepare_reuses_pinned_patched_source_and_rejects_edits(self):
        source = build.prepare_source(self.root, self.recipe, self.root / "work")
        self.assertEqual((source / "binding.cc").read_text(), "int intl = 0;\n")
        self.assertEqual(build.prepare_source(self.root, self.recipe, self.root / "work"), source)
        (source / "binding.cc").write_text("int intl = 2;\n")
        with self.assertRaises(subprocess.CalledProcessError):
            build.prepare_source(self.root, self.recipe, self.root / "work")

    def test_prepare_rejects_staged_edits(self):
        source = build.prepare_source(self.root, self.recipe, self.root / "work")
        (source / "binding.cc").write_text("int intl = 2;\n")
        git(source, "add", "binding.cc")
        with self.assertRaisesRegex(ValueError, "cached patches changed"):
            build.prepare_source(self.root, self.recipe, self.root / "work")

    def test_prepare_rejects_wrong_v8_revision(self):
        self.recipe["v8"]["commit"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "V8 submodule"):
            build.prepare_source(self.root, self.recipe, self.root / "work")

    def test_recipe_changes_invalidate_cache(self):
        _, _, before = build.load_recipe(self.root)
        patch_file = self.root / self.recipe["patches"][0]
        patch_file.write_text(patch_file.read_text() + "\n")
        _, _, after = build.load_recipe(self.root)
        self.assertNotEqual(before, after)
        (self.root / "build.py").write_text("updated build entry point\n")
        _, _, updated = build.load_recipe(self.root)
        self.assertNotEqual(after, updated)

    def test_recipe_rejects_patch_outside_patch_directory(self):
        self.recipe["patches"] = ["../outside.patch"]
        (self.root / "versions.json").write_text(json.dumps(self.recipe))
        with self.assertRaisesRegex(ValueError, "inside patches"):
            build.load_recipe(self.root)

    def test_package_checksums_notices_and_deterministic_compression(self):
        recipe, inputs, digest = build.load_recipe(self.root)
        output = self.root / "gn_out"
        (output / "obj").mkdir(parents=True)
        # The packaging contract requires an ordinary ar header. Compilation
        # and engine behavior are checked separately with an actual V8 build.
        native = b"!<arch>\nfixture payload\n"
        (output / "obj/librusty_v8.a").write_bytes(native)
        (output / "src_binding.rs").write_text("// generated bindings\n")
        destinations = [self.root / "dist/first", self.root / "dist/second"]
        for destination in destinations:
            build.package(recipe, inputs, digest, self.target,
                          self.root / "upstream", output, destination)
        first, second = destinations
        for path in first.iterdir():
            self.assertEqual(path.read_bytes(), (second / path.name).read_bytes())
        archive = first / f"librusty_v8_release_{self.target}.a.gz"
        self.assertEqual(gzip.decompress(archive.read_bytes()), native)
        for line in (first / f"SHA256SUMS_{self.target}").read_text().splitlines():
            expected, filename = line.split("  ", 1)
            self.assertEqual(hashlib.sha256((first / filename).read_bytes()).hexdigest(), expected)
        with tarfile.open(first / f"licenses_{self.target}.tar.gz") as bundle:
            self.assertEqual(bundle.getnames(), ["LICENSE", "v8/LICENSE"])
            self.assertEqual(bundle.extractfile("v8/LICENSE").read(), b"V8 notice\n")
        with self.assertRaisesRegex(ValueError, "never overwritten"):
            build.package(recipe, inputs, digest, self.target,
                          self.root / "upstream", output, first)

    def test_package_rejects_thin_archive(self):
        recipe, inputs, digest = build.load_recipe(self.root)
        output = self.root / "gn_out"
        (output / "obj").mkdir(parents=True)
        (output / "obj/librusty_v8.a").write_bytes(b"!<thin>\n")
        with self.assertRaisesRegex(ValueError, "complete static archive"):
            build.package(recipe, inputs, digest, self.target,
                          self.root / "upstream", output, self.root / "dist/package")


if __name__ == "__main__":
    unittest.main()
