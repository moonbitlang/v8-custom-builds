"""Exercise promotion boundaries with real Git trees and synthetic packages."""

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

import native_artifacts as native


class ArtifactsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.previous = Path.cwd()
        os.chdir(self.root)
        self.env = patch.dict(os.environ, {"GITHUB_REPOSITORY": "owner/wee8"})
        self.env.start()
        subprocess.run(["git", "init", "-q", "-b", "main"], check=True)
        self.matrix = [{"target": target, "runner": "test", "python": "x64"} for target in (
            "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-apple-darwin",
            "aarch64-apple-darwin", "x86_64-pc-windows-msvc")]
        self.write("ci/targets.json", json.dumps(self.matrix))
        self.write(".github/workflows/build.yml", "native build configuration\n")
        self.write("custom_wee8/src/c-api.cc", "native source\n")
        self.write("crates/wee8/build.rs", 'const RELEASE: &str = "v0.106.0-moonbit.6";\n')
        self.producer = self.save()
        self.write("crates/wee8/src/lib.rs", "new Rust wrapper\n")
        self.write("releases/v0.106.0-moonbit.6.md", "# v0.106.0-moonbit.6\n\nTested changes.\n")
        self.revision = self.save()
        subprocess.run(["git", "update-ref", "refs/remotes/origin/main", self.revision], check=True)
        subprocess.run(["git", "tag", "v0.106.0-moonbit.6"], check=True)
        self.run = {"id": 123, "head_sha": self.producer, "head_branch": "codex/candidate",
                    "head_repository": {"full_name": "owner/wee8"}, "event": "workflow_dispatch",
                    "path": ".github/workflows/build.yml", "conclusion": "success"}
        self.artifacts = {f"native-{row['target']}": {"id": i + 1, "digest": "sha256:" + "a" * 64}
                          for i, row in enumerate(self.matrix)}
        with patch.object(native, "artifact_map", return_value=self.artifacts):
            self.selected = native.candidate(self.run, self.revision)

    def tearDown(self):
        self.env.stop()
        os.chdir(self.previous)
        self.temp.cleanup()

    def write(self, name, data):
        path = Path(name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(data)

    def save(self):
        subprocess.run(["git", "add", "."], check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "fixture"], check=True)
        return native.command("git", "rev-parse", "HEAD").decode().strip()

    def package(self, target, directory):
        directory.mkdir(parents=True)
        for name in native.package_files(target):
            (directory / name).write_bytes(b"tested payload\n")
        (directory / f"validation_{target}.json").write_text(json.dumps({
            "target": target, "run": "123", "recipe": self.producer}))
        sums = {name: hashlib.sha256((directory / name).read_bytes()).hexdigest()
                for name in native.package_files(target)}
        (directory / f"SHA256SUMS_{target}").write_text(
            "".join(f"{digest}  ./{name}\n" for name, digest in sorted(sums.items())))
        return sums

    def proof(self):
        receipts = self.root / "receipts"
        receipts.mkdir()
        for target in self.selected["artifacts"]:
            files = self.package(target, self.root / "packages" / target)
            (receipts / f"{target}.json").write_text(json.dumps({
                "candidate": self.selected, "consumer_revision": self.revision, "files": files}))
        return native.qualify(self.selected, receipts, self.revision)

    def test_fingerprint_allows_wrapper_and_docs_changes_but_tracks_native_inputs(self):
        self.assertEqual(native.fingerprint(self.producer), native.fingerprint(self.revision))
        self.write("README.md", "documentation\n")
        self.assertEqual(native.fingerprint(self.revision), native.fingerprint(self.save()))
        for name in ("custom_wee8/src/c-api.cc", ".github/workflows/build.yml", "ci/targets.json",
                     "custom_wee8/upstream.json", "custom_wee8/tests/check.py", "ci/check-glibc.sh",
                     ".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".gitattributes"):
            with self.subTest(path=name):
                previous = native.fingerprint(native.command("git", "rev-parse", "HEAD").decode().strip())
                self.write(name, "changed native input\n")
                self.assertNotEqual(previous, native.fingerprint(self.save()))
        previous = native.fingerprint(native.command("git", "rev-parse", "HEAD").decode().strip())
        Path("custom_wee8/src/c-api.cc").unlink()
        self.assertNotEqual(previous, native.fingerprint(self.save()))

    def test_candidate_accepts_different_commit_with_identical_native_inputs(self):
        self.assertNotEqual(self.producer, self.revision)
        self.assertEqual(self.selected["producer_revision"], self.producer)
        self.assertEqual(len(self.selected["artifacts"]), 5)

    def test_candidate_rejects_untrusted_failed_or_partial_runs(self):
        for changes in ({"conclusion": "failure"}, {"path": ".github/workflows/other.yml"},
                        {"event": "pull_request"}, {"head_repository": {"full_name": "fork/wee8"}}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                native.candidate(dict(self.run, **changes), self.revision)
        with patch.object(native, "artifact_map", return_value={}), self.assertRaises(ValueError):
            native.candidate(self.run, self.revision)
        self.write("custom_wee8/src/c-api.cc", "different\n")
        with self.assertRaisesRegex(ValueError, "inputs differ"):
            native.candidate(self.run, self.save())

    def test_artifact_map_excludes_expired_and_rejects_ambiguous_or_digestless_entries(self):
        item = {"name": "native-test", "id": 1, "expired": True, "digest": "sha256:" + "a" * 64}
        with patch.object(native, "pages", return_value=[item]):
            self.assertEqual(native.artifact_map(123), {})
        for items in ([dict(item, expired=False)] * 2, [dict(item, expired=False, digest=None)]):
            with patch.object(native, "pages", return_value=items), self.assertRaises(ValueError):
                native.artifact_map(123)

    def test_auto_selection_reuses_the_first_matching_complete_producer(self):
        bad = dict(self.run, id=999, event="pull_request")
        original = native.command
        def commands(*args):
            return b"[[]]" if args[0] == "gh" else original(*args)
        with patch.object(native, "api", return_value={"workflow_runs": [bad, self.run]}), \
                patch.object(native, "command", side_effect=commands), \
                patch.object(native, "artifact_map", return_value=self.artifacts):
            self.assertEqual(native.select("", self.revision)["producer_run"], 123)

    def test_unpublished_pin_without_candidate_is_actionable_failure_not_skipped_success(self):
        original = native.command
        def commands(*args):
            return b"[[]]" if args[0] == "gh" else original(*args)
        with patch.object(native, "api", return_value={"workflow_runs": []}), \
                patch.object(native, "command", side_effect=commands), \
                self.assertRaisesRegex(ValueError, "dispatch Build and test once"):
            native.select("", self.revision)

    def test_matching_published_release_preserves_normal_download_coverage(self):
        original = native.command
        def commands(*args):
            if args[0] == "gh":
                return json.dumps([[{"tag_name": "v0.106.0-moonbit.6", "draft": False}]]).encode()
            return original(*args)
        with patch.object(native, "api", return_value={"workflow_runs": []}), \
                patch.object(native, "command", side_effect=commands):
            self.assertEqual(native.select("", self.revision)["source"], "release")

    def test_zip_digest_and_flat_file_boundary(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr("candidate.json", "candidate")
        payload = data.getvalue()
        artifact = {"id": 1, "digest": "sha256:" + hashlib.sha256(payload).hexdigest()}
        with patch.object(native, "command", return_value=payload):
            native.download_artifact(artifact, self.root / "download")
            self.assertEqual((self.root / "download/candidate.json").read_text(), "candidate")
            with self.assertRaisesRegex(ValueError, "digest mismatch"):
                native.download_artifact(dict(artifact, digest="sha256:" + "0" * 64), self.root / "bad")
        for name in ("../escape", "dir/file", "dir\\file", "C:escape", "name:stream"):
            data = io.BytesIO()
            with zipfile.ZipFile(data, "w") as archive:
                archive.writestr(name, "bad")
            payload = data.getvalue()
            artifact["digest"] = "sha256:" + hashlib.sha256(payload).hexdigest()
            with patch.object(native, "command", return_value=payload), self.assertRaisesRegex(ValueError, "flat"):
                native.download_artifact(artifact, self.root / "bad")

    def test_package_checks_bytes_inventory_and_producer_provenance(self):
        target = self.matrix[0]["target"]
        directory = self.root / "package"
        files = self.package(target, directory)
        self.assertEqual(native.verify_package(self.selected, target, directory), files)
        wrong = dict(self.selected, producer_run=456)
        with self.assertRaisesRegex(ValueError, "provenance"):
            native.verify_package(wrong, target, directory)
        (directory / "unexpected").write_text("extra")
        with self.assertRaisesRegex(ValueError, "unexpected package"):
            native.verify_package(self.selected, target, directory)
        (directory / "unexpected").unlink()
        (directory / next(iter(files))).write_text("changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            native.verify_package(self.selected, target, directory)

    def test_qualification_requires_matching_receipts_for_all_five_targets(self):
        proof = self.proof()
        self.assertEqual(len(proof["targets"]), 5)
        path = self.root / "receipts" / f"{self.matrix[0]['target']}.json"
        receipt = json.loads(path.read_text())
        receipt["consumer_revision"] = self.producer
        path.write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, "receipt"):
            native.qualify(self.selected, path.parent, self.revision)
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            native.qualify(self.selected, path.parent, self.revision)

    def test_promotion_requires_nonempty_matching_tagged_notes(self):
        for content in ("# v0.106.0-moonbit.6\n", "# wrong-version\n\nDescription.\n"):
            self.write("releases/v0.106.0-moonbit.6.md", content)
            revision = self.save()
            subprocess.run(["git", "update-ref", "refs/remotes/origin/main", revision], check=True)
            subprocess.run(["git", "tag", "-f", "v0.106.0-moonbit.6"], check=True, stdout=subprocess.DEVNULL)
            with self.subTest(content=content), patch.object(native, "api") as api:
                with self.assertRaisesRegex(ValueError, "release notes"):
                    native.prepare_release(456, "v0.106.0-moonbit.6", self.root / "bad-notes")
                api.assert_not_called()
        Path("releases/v0.106.0-moonbit.6.md").unlink()
        revision = self.save()
        subprocess.run(["git", "update-ref", "refs/remotes/origin/main", revision], check=True)
        subprocess.run(["git", "tag", "-f", "v0.106.0-moonbit.6"], check=True, stdout=subprocess.DEVNULL)
        with patch.object(native, "api") as api:
            with self.assertRaises(subprocess.CalledProcessError):
                native.prepare_release(456, "v0.106.0-moonbit.6", self.root / "missing-notes")
            api.assert_not_called()

    def test_promotion_preserves_pr_producer_bytes_and_requires_tagged_main_validation(self):
        proof = self.proof()
        validation = dict(self.run, id=456, head_sha=self.revision, head_branch="main",
                          event="push", path=".github/workflows/rust-crate.yml")
        def api(path):
            return validation if path == "actions/runs/456" else self.run
        def artifacts(run):
            return {"wee8-validation": {"id": 99, "digest": "proof"}} if run == 456 else self.artifacts
        def download(artifact, destination):
            destination.mkdir(parents=True)
            if artifact["id"] == 99:
                (destination / "qualification.json").write_text(json.dumps(proof))
            else:
                target = self.matrix[artifact["id"] - 1]["target"]
                for source in (self.root / "packages" / target).iterdir():
                    (destination / source.name).write_bytes(source.read_bytes())
        with patch.object(native, "api", side_effect=api), \
                patch.object(native, "artifact_map", side_effect=artifacts), \
                patch.object(native, "download_artifact", side_effect=download):
            # Promotion must ignore edited checkout notes and use the tag's text.
            self.write("releases/v0.106.0-moonbit.6.md", "unreviewed checkout notes")
            output = self.root / "release"
            native.prepare_release(456, "v0.106.0-moonbit.6", output)
            self.assertTrue((output / "release-notes.md").read_text().startswith(
                "# v0.106.0-moonbit.6\n\nTested changes."))
            self.assertIn("/actions/runs/456", (output / "release-notes.md").read_text())
            self.assertEqual(json.loads((output / "promotion.json").read_text())["producer_revision"], self.producer)
            for target in proof["targets"]:
                for original in (self.root / "packages" / target).iterdir():
                    self.assertEqual((output / f"native-{target}" / original.name).read_bytes(), original.read_bytes())
            validation["head_sha"] = self.producer
            with self.assertRaisesRegex(ValueError, "tagged main commit"):
                native.prepare_release(456, "v0.106.0-moonbit.6", self.root / "stale")
            validation["head_sha"] = self.revision
            for changes in ({"head_branch": "codex/unmerged"}, {"event": "pull_request"},
                            {"conclusion": "failure"}, {"head_repository": {"full_name": "fork/wee8"}}):
                old = dict(validation)
                validation.update(changes)
                with self.subTest(changes=changes), self.assertRaises(ValueError):
                    native.prepare_release(456, "v0.106.0-moonbit.6", self.root / "untrusted")
                validation.clear()
                validation.update(old)
            subprocess.run(["git", "tag", "v0.106.0-moonbit.7"], check=True)
            with self.assertRaisesRegex(ValueError, "release pin"):
                native.prepare_release(456, "v0.106.0-moonbit.7", self.root / "wrong-pin")
            proof["targets"][self.matrix[0]["target"]][next(iter(proof["targets"][self.matrix[0]["target"]]))] = "0" * 64
            with self.assertRaisesRegex(ValueError, "bytes differ"):
                native.prepare_release(456, "v0.106.0-moonbit.6", self.root / "changed")


if __name__ == "__main__":
    unittest.main()
