#!/usr/bin/env python3
"""Reuse tested native archives and promote their bytes, never rebuild V8.

The native fingerprint identifies inputs, not a reproducible-binary claim. Archive
digests and per-file checksums bind consumer validation to the bytes published.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile


# Keep the producer workflow itself in this boundary: it sets compilers, SDKs,
# flags and validation. Rust consumers and documentation can evolve independently.
# Target membership is checked separately: retiring a platform must not invalidate
# other platforms, but every retained target's configuration must still match.
# Include the native probes and their fixtures so old validation is not silently
# reused after a contract change. The producer's existing cache key is separate.
NATIVE_INPUTS = [
    ".gitmodules", ".gitattributes", ".cargo", "Cargo.toml", "Cargo.lock",
    "rust-toolchain", "rust-toolchain.toml", ".github/workflows/build.yml",
    "rusty_v8", "build.sh", "tools/prepare-source.py", "tools/native",
    "custom_wee8/BUILD.gn", "custom_wee8/upstream.json",
    "custom_wee8/src", "custom_wee8/include", "custom_wee8/tests",
    "patches", "licenses", "ci/check-glibc.sh",
]


def command(*args):
    return subprocess.check_output(args)


def api(path):
    repo = os.environ["GITHUB_REPOSITORY"]
    return json.loads(command("gh", "api", f"repos/{repo}/{path}"))


def pages(path, field):
    repo = os.environ["GITHUB_REPOSITORY"]
    result = json.loads(command("gh", "api", "--paginate", "--slurp",
                                f"repos/{repo}/{path}"))
    return [item for page in result for item in page[field]]


def commit(revision):
    # API revisions are immutable SHA-1 object IDs, never shell expressions/refs.
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("expected a full commit ID")
    if subprocess.run(["git", "cat-file", "-e", f"{revision}^{{commit}}"],
                      stderr=subprocess.DEVNULL).returncode:
        subprocess.run(["git", "fetch", "--no-tags", "origin", revision], check=True)
    return revision


def fingerprint(revision):
    tree = command("git", "ls-tree", "-rz", "--full-tree", commit(revision),
                   "--", *NATIVE_INPUTS)
    return hashlib.sha256(tree).hexdigest()


def targets(revision):
    matrix = json.loads(command("git", "show", f"{revision}:ci/targets.json"))
    names = [row["target"] for row in matrix]
    if not names or len(set(names)) != len(names):
        raise ValueError("expected a nonempty list of distinct native targets")
    return matrix


def matching_inputs(producer, revision):
    producer_targets = targets(producer)
    return (fingerprint(producer) == fingerprint(revision)
            and all(row in producer_targets for row in targets(revision)))


def checked_run(run, workflow):
    if (run["conclusion"] != "success" or run["path"] != workflow
            or run["head_repository"]["full_name"] != os.environ["GITHUB_REPOSITORY"]):
        raise ValueError("run must succeed in this repository's expected workflow")
    commit(run["head_sha"])


def artifact_map(run_id):
    artifacts = pages(f"actions/runs/{run_id}/artifacts?per_page=100", "artifacts")
    result = {}
    for artifact in artifacts:
        if artifact["expired"]:
            continue
        if artifact["name"] in result:
            raise ValueError("ambiguous artifact name")
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", artifact.get("digest") or ""):
            raise ValueError("artifact has no SHA-256 digest")
        result[artifact["name"]] = {"id": artifact["id"], "digest": artifact["digest"]}
    return result


def candidate(run, revision):
    checked_run(run, ".github/workflows/build.yml")
    if run["event"] != "workflow_dispatch":
        raise ValueError("native producer must be explicitly dispatched")
    identity = fingerprint(revision)
    if not matching_inputs(run["head_sha"], revision):
        raise ValueError("producer native inputs differ from this revision")
    matrix = targets(revision)
    names = [row["target"] for row in matrix]
    artifacts = artifact_map(run["id"])
    if any(f"native-{target}" not in artifacts for target in names):
        raise ValueError("producer must retain every required native artifact")
    return {
        "schema": 1, "source": "run", "producer_run": run["id"],
        "producer_revision": run["head_sha"], "native_fingerprint": identity,
        "matrix": matrix,
        "artifacts": {target: artifacts[f"native-{target}"] for target in names},
    }


def select(native_run, revision):
    if native_run:
        return candidate(api(f"actions/runs/{native_run}"), revision)
    release = re.search(rb'const RELEASE: &str = "([^"]+)";',
                        command("git", "show", f"{revision}:crates/wee8/build.rs"))[1].decode()
    repo = os.environ["GITHUB_REPOSITORY"]
    releases = json.loads(command("gh", "api", "--paginate", "--slurp",
                                  f"repos/{repo}/releases?per_page=100"))
    if any(r["tag_name"] == release and not r["draft"] for page in releases for r in page):
        published = command("git", "rev-parse", f"refs/tags/{release}^{{commit}}").decode().strip()
        if matching_inputs(published, revision):
            return {"schema": 1, "source": "release", "release": release,
                    "native_fingerprint": fingerprint(revision), "matrix": targets(revision)}
    # Search completed producers automatically after Rust-only edits or squash
    # merges. An expired or different candidate is not a reason to rebuild here.
    runs = api("actions/workflows/build.yml/runs?status=success&per_page=100")["workflow_runs"]
    for run in runs:
        try:
            return candidate(run, revision)
        except ValueError:
            continue
    raise ValueError("no matching native candidate or published release; dispatch Build and test once, "
                     "then rerun Rust crate (or select an older retained run with native-run)")


def download_artifact(artifact, destination):
    repo = os.environ["GITHUB_REPOSITORY"]
    data = command("gh", "api", f"repos/{repo}/actions/artifacts/{artifact['id']}/zip")
    if "sha256:" + hashlib.sha256(data).hexdigest() != artifact["digest"]:
        raise ValueError("artifact ZIP digest mismatch")
    destination.mkdir(parents=True, exist_ok=True)
    # These artifacts contain flat files. Do not extract arbitrary paths supplied
    # by a producer into the consumer/release checkout.
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise ValueError("duplicate artifact entry")
        for name in names:
            if not name or name in (".", "..") or any(c in name for c in "/\\:"):
                raise ValueError("artifact entries must be flat file names")
            (destination / name).write_bytes(archive.read(name))


def package_files(target):
    archive = (f"wee8_release_{target}.lib.gz" if target.endswith("-windows-msvc")
               else f"libwee8_release_{target}.a.gz")
    return {archive, f"wasm_c_api_{target}.h", f"wee8_c_api_{target}.h",
            f"NOTICES_{target}.txt", f"WASM_API_LICENSE_{target}.txt", f"validation_{target}.json"}


def verify_package(selected, target, directory):
    if selected["source"] != "run":
        raise ValueError("package verification requires a produced candidate")
    if target not in {row["target"] for row in selected["matrix"]}:
        raise ValueError("unexpected target")
    checksums = {}
    for line in (directory / f"SHA256SUMS_{target}").read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64}) [ *](?:\./)?([^/\\]+)", line)
        if not match or match[2] in checksums:
            raise ValueError("invalid or duplicate checksum entry")
        checksums[match[2]] = match[1]
    if set(checksums) != package_files(target):
        raise ValueError("package must contain exactly the six expected payload files")
    if {p.name for p in directory.iterdir()} != set(checksums) | {f"SHA256SUMS_{target}"}:
        raise ValueError("unexpected package file")
    for name, digest in checksums.items():
        if hashlib.sha256((directory / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"checksum mismatch: {name}")
    provenance = json.loads((directory / f"validation_{target}.json").read_text())
    if provenance["target"] != target:
        raise ValueError("package target mismatch")
    if (provenance["run"] != str(selected["producer_run"])
            or provenance["recipe"] != selected["producer_revision"]):
        raise ValueError("package producer provenance mismatch")
    return checksums


def download(selected, target, directory):
    if selected["source"] != "run":
        raise ValueError("published releases use the Rust wrapper's normal download path")
    revision = command("git", "rev-parse", "HEAD").decode().strip()
    if (selected["native_fingerprint"] != fingerprint(revision)
            or selected["matrix"] != targets(revision)
            or not matching_inputs(selected["producer_revision"], revision)):
        raise ValueError("candidate native inputs differ from this checkout")
    download_artifact(selected["artifacts"][target], directory)
    return verify_package(selected, target, directory)


def qualify(selected, receipts, revision):
    if (selected["source"] != "run" or selected["native_fingerprint"] != fingerprint(revision)
            or selected["matrix"] != targets(revision)
            or not matching_inputs(selected["producer_revision"], revision)):
        raise ValueError("qualification requires matching produced archives")
    result = dict(selected, consumer_revision=revision, targets={})
    for row in selected["matrix"]:
        target = row["target"]
        receipt = json.loads((receipts / f"{target}.json").read_text())
        if (receipt["consumer_revision"] != revision or receipt["candidate"] != selected
                or set(receipt["files"]) != package_files(target)):
            raise ValueError("consumer receipt does not match this candidate and revision")
        result["targets"][target] = receipt["files"]
    return result


def prepare_release(validation_run, tag, output):
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+-moonbit\.[0-9]+", tag):
        raise ValueError("invalid native release tag")
    revision = command("git", "rev-parse", f"refs/tags/{tag}^{{commit}}").decode().strip()
    subprocess.run(["git", "merge-base", "--is-ancestor", revision, "origin/main"], check=True)
    build_rs = command("git", "show", f"{revision}:crates/wee8/build.rs")
    if re.search(rb'const RELEASE: &str = "([^"]+)";', build_rs)[1].decode() != tag:
        raise ValueError("tag does not match the wrapper's release pin")
    changelog = command("git", "show", f"{revision}:CHANGELOG.md").decode()
    version = re.escape(tag.removeprefix("v"))
    sections = re.findall(rf"^## \[{version}\] - \d{{4}}-\d{{2}}-\d{{2}}\n"
                          r"(.*?)(?=^## |\Z)", changelog, re.MULTILINE | re.DOTALL)
    if len(sections) != 1 or not re.sub(r"^### .*", "", sections[0], flags=re.MULTILINE).strip():
        raise ValueError("release notes require one dated changelog section with a description")
    notes = f"# {tag}\n\n" + sections[0].strip() + "\n"
    run = api(f"actions/runs/{validation_run}")
    checked_run(run, ".github/workflows/rust-crate.yml")
    if (run["head_sha"] != revision or run["head_branch"] != "main"
            or run["event"] not in ("push", "workflow_dispatch", "workflow_run")):
        raise ValueError("release requires consumer validation of the tagged main commit")
    download_artifact(artifact_map(validation_run)["wee8-validation"], output / "validation")
    proof = json.loads((output / "validation/qualification.json").read_text())
    if proof["schema"] != 1 or proof["consumer_revision"] != revision:
        raise ValueError("qualification revision mismatch")
    selected = candidate(api(f"actions/runs/{proof['producer_run']}"), revision)
    if any(proof[key] != value for key, value in selected.items()):
        raise ValueError("producer identity or artifact digests changed after consumer validation")
    if set(proof["targets"]) != set(selected["artifacts"]):
        raise ValueError("qualification must cover every required target")
    for target, artifact in selected["artifacts"].items():
        directory = output / f"native-{target}"
        download_artifact(artifact, directory)
        if verify_package(selected, target, directory) != proof["targets"][target]:
            raise ValueError("release bytes differ from the consumer-tested package")
    (output / "release-notes.md").write_text(notes.rstrip() +
        f"\n\nConsumer qualification: https://github.com/{os.environ['GITHUB_REPOSITORY']}"
        f"/actions/runs/{validation_run}. All {len(selected['matrix'])} release targets passed native and consumer "
        "validation. Original producer provenance is preserved in the assets and promotion.json.\n")
    # Preserve each producer's validation file; add promotion metadata separately.
    (output / "promotion.json").write_text(json.dumps(dict(proof, tag=tag,
        validation_run=validation_run), indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    select_parser = commands.add_parser("select")
    select_parser.add_argument("--native-run", default="")
    select_parser.add_argument("--output", type=Path, required=True)
    download_parser = commands.add_parser("download")
    download_parser.add_argument("candidate", type=Path)
    download_parser.add_argument("--target", required=True)
    download_parser.add_argument("--output", type=Path, required=True)
    receipt_parser = commands.add_parser("receipt")
    receipt_parser.add_argument("candidate", type=Path)
    receipt_parser.add_argument("--target", required=True)
    receipt_parser.add_argument("--package", type=Path, required=True)
    receipt_parser.add_argument("--output", type=Path, required=True)
    qualify_parser = commands.add_parser("qualify")
    qualify_parser.add_argument("candidate", type=Path)
    qualify_parser.add_argument("--receipts", type=Path, required=True)
    qualify_parser.add_argument("--output", type=Path, required=True)
    release_parser = commands.add_parser("prepare-release")
    release_parser.add_argument("--validation-run", type=int, required=True)
    release_parser.add_argument("--tag", required=True)
    release_parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    revision = command("git", "rev-parse", "HEAD").decode().strip()
    if args.action == "select":
        if args.native_run and not re.fullmatch(r"[1-9][0-9]*", args.native_run):
            raise ValueError("native-run must be a positive run ID")
        selected = select(args.native_run, revision)
        args.output.write_text(json.dumps(selected, indent=2) + "\n")
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"source={selected['source']}\n")
            output.write("matrix=" + json.dumps({"include": selected["matrix"]}) + "\n")
        print(f"Selected {selected['source']}: {selected.get('producer_run', selected.get('release'))}")
    elif args.action == "prepare-release":
        prepare_release(args.validation_run, args.tag, args.output)
    else:
        selected = json.loads(args.candidate.read_text())
        if args.action == "download":
            download(selected, args.target, args.output)
            print(f"Verified {args.target}: artifact digest, package checksums and provenance")
        elif args.action == "receipt":
            files = verify_package(selected, args.target, args.package)
            args.output.write_text(json.dumps({"candidate": selected, "consumer_revision": revision,
                                               "files": files}, indent=2) + "\n")
        else:
            args.output.write_text(json.dumps(qualify(selected, args.receipts, revision), indent=2) + "\n")


if __name__ == "__main__":
    main()
