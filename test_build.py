"""Check the build command with real Git fixtures and simulated Cargo outputs."""

import contextlib
import gzip
import hashlib
import io
import os
from pathlib import Path
import shutil
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
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        self.environment = patch.dict(os.environ, {"GIT_ALLOW_PROTOCOL": "file"})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.upstream = self.root.parent / "upstream"
        self.upstream.mkdir()
        git(self.upstream, "init", "-b", "main")
        (self.upstream / "v8").mkdir()
        (self.upstream / "v8/LICENSE").write_text("V8 notice\n")
        (self.upstream / "LICENSE").write_text("Binding notice\n")
        (self.upstream / "binding.cc").write_text("int intl = 1;\n")
        git(self.upstream, "add", ".")
        git(self.upstream, "commit", "-m", "fixture")
        git(self.root, "init", "-b", "main")
        git(self.root, "submodule", "add", self.upstream.as_uri(), "rusty_v8")
        git(self.root, "commit", "-am", "pin upstream")
        (self.root / "patches").mkdir()
        (self.upstream / "binding.cc").write_text("int intl = 0;\n")
        (self.root / "patches/no-icu.patch").write_text(git(self.upstream, "diff") + "\n")
        git(self.upstream, "restore", "binding.cc")
        self.target = "aarch64-apple-darwin"
        self.native = b"!<arch>\nfixture payload\n"
        self.cargo_calls = []
        self.real_run = subprocess.run

    def run_command(self, command, **kwargs):
        if command[0] == "rustc":
            return subprocess.CompletedProcess(command, 0, f"host: {self.target}\n")
        if command[0] != "cargo":
            return self.real_run(command, **kwargs)
        self.cargo_calls.append((command, kwargs))
        output = self.root / "build" / self.target / "release/gn_out"
        (output / "obj").mkdir(parents=True, exist_ok=True)
        native = "rusty_v8.lib" if self.target.endswith("windows-msvc") else "librusty_v8.a"
        (output / "obj" / native).write_bytes(self.native)
        (output / "src_binding.rs").write_text("// generated bindings\n")
        return subprocess.CompletedProcess(command, 0)

    def invoke(self):
        with patch.object(build, "ROOT", self.root), \
             patch("subprocess.run", side_effect=self.run_command), \
             contextlib.redirect_stdout(io.StringIO()):
            build.main(["--jobs", "2"])

    def test_build_and_package_with_upstream_conventions(self):
        # Unix and Windows use different upstream archive names. The compiler
        # is simulated; source initialization, patching, and packaging are real.
        for target, native in [("aarch64-apple-darwin", "librusty_v8.a"),
                               ("x86_64-pc-windows-msvc", "rusty_v8.lib")]:
            with self.subTest(target=target):
                self.target = target
                self.invoke()
                self.assertEqual((self.root / "rusty_v8/binding.cc").read_text(), "int intl = 0;\n")
                command, kwargs = self.cargo_calls[-1]
                self.assertEqual(command, ("cargo", "build", "--release", "--locked", "--target", target,
                                          "--target-dir", str(self.root / "build"), "-p", "v8", "--jobs", "2"))
                self.assertEqual(kwargs["cwd"], self.root / "rusty_v8")
                self.assertEqual(kwargs["env"]["V8_FROM_SOURCE"], "1")
                self.assertEqual(kwargs["env"]["GN_ARGS"], "v8_enable_i18n_support=false icu_use_data_file=false")
                destination = self.root / "dist" / target
                stem, suffix = native.rsplit(".", 1)
                archive = destination / f"{stem}_release_{target}.{suffix}.gz"
                self.assertEqual(gzip.decompress(archive.read_bytes()), self.native)
                self.assertEqual((destination / f"src_binding_release_{target}.rs").read_text(), "// generated bindings\n")
                checksums = (destination / f"SHA256SUMS_{target}").read_text().splitlines()
                self.assertEqual(len(checksums), 3)
                for line in checksums:
                    digest, filename = line.split("  ", 1)
                    self.assertEqual(hashlib.sha256((destination / filename).read_bytes()).hexdigest(), digest)
                with tarfile.open(destination / f"licenses_{target}.tar.gz") as notices:
                    self.assertEqual(notices.getnames(), ["LICENSE", "v8/LICENSE"])
                    for entry in notices:
                        self.assertEqual((entry.uid, entry.gid, entry.uname, entry.gname, entry.mtime), (0, 0, "", "", 0))
                with self.assertRaisesRegex(ValueError, "output already exists"):
                    self.invoke()
                # The same patched checkout can be built again after outputs
                # are moved aside; there is no separate preparation state.
                shutil.rmtree(destination)
                self.invoke()

    def test_incompatible_patch_stops_before_compilation(self):
        (self.root / "rusty_v8/binding.cc").write_text("int intl = 2;\n")
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertFalse(self.cargo_calls)
        self.assertFalse((self.root / "dist").exists())

    def test_failed_build_does_not_package_stale_outputs(self):
        self.invoke()
        shutil.rmtree(self.root / "dist")
        original_run = self.run_command

        def fail_cargo(command, **kwargs):
            if command[0] == "cargo":
                raise subprocess.CalledProcessError(1, command)
            return original_run(command, **kwargs)

        self.run_command = fail_cargo
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertFalse((self.root / "dist").exists())


if __name__ == "__main__":
    unittest.main()
