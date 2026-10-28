"""A completed base survives later job failures, without trusting unchecked bytes."""

import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import base_artifacts
from base_artifacts import base_cache, native


class BaseArtifactsTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.addCleanup(patch.stopall)
        patch.object(base_cache, "ROOT", self.root).start()
        patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/wee8", "ImageVersion": "fixture"}).start()
        source = self.root / "rusty_v8"
        source.mkdir()
        (source / "BUILD.gn").write_text("fixture")
        subprocess.run(["git", "init", "-q", self.root], check=True)
        subprocess.run(["git", "-C", self.root, "add", "."], check=True)
        subprocess.run(["git", "-C", self.root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "fixture"], check=True)
        head = subprocess.check_output(["git", "-C", self.root, "rev-parse", "HEAD"], text=True).strip()
        # Use the same gitlink boundary as the producer, not a mocked identity.
        subprocess.run(["git", "-C", self.root, "rm", "-qr", "--cached", "rusty_v8"], check=True)
        subprocess.run(["git", "-C", self.root, "update-index", "--add", "--cacheinfo", f"160000,{head},rusty_v8"], check=True)
        subprocess.run(["git", "-C", self.root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "pin"], check=True)
        head = subprocess.check_output(["git", "-C", self.root, "rev-parse", "HEAD"], text=True).strip()
        self.identity = base_cache.identity()
        self.run = {"id": 123, "head_sha": head, "event": "workflow_dispatch", "conclusion": "failure",
                    "path": ".github/workflows/build.yml", "head_repository": {"full_name": "owner/wee8"}}
        self.files = {"libwee8_base.a": b"base archive", "gen/layout.h": b"generated layout"}
        self.manifest = {"schema": 1, "identity": self.identity, "target": "fixture", "runner_image": "fixture",
                         "archive": "libwee8_base.a", "configuration": {}, "producer_revision": head, "producer_run": "123",
                         "files": {name: hashlib.sha256(data).hexdigest() for name, data in self.files.items()}}
        patch.object(native, "api", return_value={"workflow_runs": [self.run]}).start()
        patch.object(native, "commit", side_effect=lambda revision: revision).start()

    def restore(self, extra=None, bad_digest=False):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr("base.json", json.dumps(self.manifest))
            for name, contents in {**self.files, **(extra or {})}.items():
                # ZipInfo normalizes backslashes on Windows. Keep deliberately
                # malformed entry names literal so every runner tests the boundary.
                entry = zipfile.ZipInfo()
                entry.filename = name
                archive.writestr(entry, contents)
        payload = data.getvalue()
        artifact = {"id": 4, "digest": "sha256:" + ("0" * 64 if bad_digest else hashlib.sha256(payload).hexdigest())}
        with patch.object(native, "artifact_map", return_value={f"base-fixture-{self.identity}": artifact}), \
             patch.object(native, "command", return_value=payload):
            base_artifacts.restore("fixture")

    def test_base_from_failed_later_stage_is_verified_and_reused(self):
        self.restore()
        base = self.root / "target/wee8-base/fixture"
        self.assertEqual(base_cache.verify(base, "fixture"), self.manifest)
        self.assertEqual((base / "libwee8_base.a").read_bytes(), b"base archive")

    def test_corrupt_zip_and_files_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "ZIP checksum"):
            self.restore(bad_digest=True)
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            self.restore(extra={"gen/layout.h": b"tampered"})
        self.assertFalse((self.root / "target/wee8-base/fixture").exists())

    def test_wrong_identity_and_producer_are_rejected(self):
        self.manifest["identity"] = "wrong"
        with self.assertRaisesRegex(ValueError, "base inputs differ"):
            self.restore()
        self.manifest["identity"] = self.identity
        self.manifest["producer_run"] = "999"
        with self.assertRaisesRegex(ValueError, "producer differs"):
            self.restore()

    def test_zip_reader_cannot_hide_nonportable_entry_names(self):
        # Exercise Windows' ZIP name normalization even on Unix recipe runners.
        with patch.object(zipfile.os, "sep", "\\"), self.assertRaisesRegex(ValueError, "unexpected base artifact entry"):
            self.restore(extra={"gen\\escape": b"unexpected"})

    def test_unexpected_paths_are_rejected_and_other_runner_is_skipped(self):
        for name in ("../escape", "/escape", "gen/../../escape", "gen\\escape", "C:/escape", "tools/clang"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "unexpected base artifact entry"):
                self.restore(extra={name: b"unexpected"})
        self.manifest["runner_image"] = "different"
        self.restore()
        self.assertFalse((self.root / "target/wee8-base/fixture").exists())


if __name__ == "__main__":
    unittest.main()
