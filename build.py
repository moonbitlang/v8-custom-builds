#!/usr/bin/env python3
"""Build and package the pinned native library; publishing is a separate step."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parent
TARGETS = {
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
}


def run(*args, cwd=None, env=None, capture=False):
    print("+", subprocess.list2cmdline([str(arg) for arg in args]), flush=True)
    result = subprocess.run(
        [str(arg) for arg in args], cwd=cwd, env=env, check=True,
        stdout=subprocess.PIPE if capture else None, text=True,
    )
    return result.stdout.strip() if capture else None


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def load_recipe(root):
    recipe = json.loads((root / "versions.json").read_text())
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", recipe["release"]):
        raise ValueError("release must be a simple tag without path separators")
    for dependency in ("rusty_v8", "v8"):
        if not re.fullmatch(r"[0-9a-f]{40}", recipe[dependency]["commit"]):
            raise ValueError(f"{dependency} must pin a full Git commit")
    inputs = {"versions.json": sha256(root / "versions.json"),
              "build.py": sha256(root / "build.py")}
    for name in recipe["patches"]:
        patch = (root / name).resolve()
        if not patch.is_relative_to(root.resolve() / "patches"):
            raise ValueError("patches must be files inside patches/")
        inputs[name] = sha256(patch)
    # Separate source trees and Cargo/GN outputs when any build input changes.
    # In particular, upstream build.rs does not regenerate an existing args.gn.
    digest = hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()
    return recipe, inputs, digest


def prepare_source(root, recipe, work):
    source = work / "source"
    stamp = work / "prepared.json"
    pin = recipe["rusty_v8"]
    if not source.exists():
        source.mkdir(parents=True)
        run("git", "init", source)
        run("git", "remote", "add", "origin", pin["repository"], cwd=source)
        run("git", "fetch", "--depth=1", "origin", pin["commit"], cwd=source)
        run("git", "checkout", "--detach", "FETCH_HEAD", cwd=source)
    if run("git", "rev-parse", "HEAD", cwd=source, capture=True) != pin["commit"]:
        raise ValueError("cached source has a different revision; use a fresh build directory")

    if not stamp.exists():
        run("git", "diff", "--exit-code", cwd=source)
        run("git", "diff", "--cached", "--exit-code", cwd=source)
        run("git", "submodule", "update", "--init", "--recursive", "--depth=1", cwd=source)
        for name in recipe["patches"]:
            run("git", "apply", "--check", "--index", root / name, cwd=source)
            run("git", "apply", "--index", root / name, cwd=source)
        staged = run("git", "diff", "--cached", "--binary", cwd=source, capture=True)
        stamp.write_text(json.dumps({
            "staged_sha256": hashlib.sha256(staged.encode()).hexdigest(),
        }) + "\n")

    # Never silently build edits made in the cached checkout. Patches are staged
    # against the upstream commit, making the expected source tree checkable.
    run("git", "diff", "--exit-code", "--ignore-submodules=untracked", cwd=source)
    staged = run("git", "diff", "--cached", "--binary", cwd=source, capture=True)
    if hashlib.sha256(staged.encode()).hexdigest() != json.loads(stamp.read_text())["staged_sha256"]:
        raise ValueError("cached patches changed; use a fresh build directory")
    if run("git", "ls-files", "--others", "--exclude-standard", cwd=source, capture=True):
        raise ValueError("cached source contains unexpected untracked files")
    modules = run("git", "submodule", "status", "--recursive", cwd=source, capture=True)
    if any(line.startswith(("-", "+", "U")) for line in modules.splitlines()):
        raise ValueError("submodules do not match the pinned source tree")
    if run("git", "rev-parse", "HEAD", cwd=source / "v8", capture=True) != recipe["v8"]["commit"]:
        raise ValueError("V8 submodule does not match versions.json")
    return source


def package(recipe, inputs, digest, target, source, gn_out, destination):
    native = "rusty_v8.lib" if target.endswith("windows-msvc") else "librusty_v8.a"
    archive = gn_out / "obj" / native
    bindings = gn_out / "src_binding.rs"
    with archive.open("rb") as file:
        if file.read(8) != b"!<arch>\n":
            raise ValueError("expected a complete static archive, not a thin archive")
    if not bindings.is_file() or not bindings.stat().st_size:
        raise ValueError("generated Rust bindings are missing")
    if destination.exists():
        raise ValueError(f"artifact directory already exists: {destination}; existing artifacts are never overwritten")
    destination.parent.mkdir(parents=True, exist_ok=True)

    # Publish the local package directory only when every artifact is complete.
    with tempfile.TemporaryDirectory(prefix=".packaging-", dir=destination.parent) as temp:
        stage = Path(temp)
        stem = "rusty_v8" if target.endswith("windows-msvc") else "librusty_v8"
        suffix = ".lib" if target.endswith("windows-msvc") else ".a"
        filename = f"{stem}_release_{target}{suffix}.gz"
        with (stage / filename).open("wb") as output:
            with gzip.GzipFile(filename="", fileobj=output, mode="wb", mtime=0) as compressed:
                with archive.open("rb") as file:
                    shutil.copyfileobj(file, compressed)
        shutil.copyfile(bindings, stage / f"src_binding_release_{target}.rs")

        # Preserve upstream notices without distributing the full source tree.
        tracked = subprocess.check_output(
            ["git", "ls-files", "--recurse-submodules", "-z"], cwd=source,
        ).decode().split("\0")
        notices = sorted(name for name in tracked if name and
                         Path(name).name.upper().startswith(("LICENSE", "COPYING", "NOTICE")))
        if "LICENSE" not in notices or "v8/LICENSE" not in notices:
            raise ValueError("upstream license notices are missing")
        with (stage / f"licenses_{target}.tar.gz").open("wb") as output:
            with gzip.GzipFile(filename="", fileobj=output, mode="wb", mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as bundle:
                    for name in notices:
                        path = source / name
                        if not path.is_file():
                            continue
                        info = tarfile.TarInfo(name)
                        info.size = path.stat().st_size
                        info.mode = 0o644
                        with path.open("rb") as file:
                            bundle.addfile(info, file)

        artifacts = {path.name: sha256(path) for path in sorted(stage.iterdir())}
        manifest = {
            "schema": 1, "release": recipe["release"], "target": target,
            "profile": "release", "recipe_sha256": digest, "inputs": inputs,
            "sources": {name: recipe[name] for name in ("rusty_v8", "v8")},
            "gn_args": recipe["gn_args"], "artifacts": artifacts,
            "native_archive_sha256": sha256(archive),
        }
        metadata = stage / f"manifest_{target}.json"
        metadata.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        checksums = "".join(f"{sha256(path)}  {path.name}\n" for path in sorted(stage.iterdir()))
        (stage / f"SHA256SUMS_{target}").write_text(checksums)
        stage.rename(destination)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare-only", action="store_true", help="fetch and patch sources without compiling")
    parser.add_argument("--dry-run", action="store_true", help="show the recipe without fetching or building")
    parser.add_argument("--jobs", type=int, help="maximum parallel Cargo/Ninja jobs")
    args = parser.parse_args()
    if args.jobs is not None and args.jobs < 1:
        parser.error("--jobs must be positive")
    recipe, inputs, digest = load_recipe(ROOT)
    rustc = run("rustc", "-vV", capture=True)
    target = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    if target not in TARGETS:
        raise ValueError(f"unsupported native target: {target}")
    work = ROOT / "build" / digest[:16] / target
    destination = ROOT / "dist" / recipe["release"] / target
    print(json.dumps({"release": recipe["release"], "target": target,
                      "gn_args": recipe["gn_args"], "recipe_sha256": digest}, indent=2))
    if args.dry_run:
        return

    # Build inputs come from the recipe, not an ambient download override or a
    # previous source build's GN flags. Compiler overrides need a recipe change.
    conflicting = [name for name in (
        "GN_ARGS", "EXTRA_GN_ARGS", "CLANG_BASE_PATH", "DISABLE_CLANG",
        "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
    ) if os.environ.get(name)]
    if conflicting:
        raise ValueError("unset build overrides before building: " + ", ".join(conflicting))
    source = prepare_source(ROOT, recipe, work)
    if args.prepare_only:
        print("Pinned, patched sources are ready.")
        return
    if destination.exists():
        raise ValueError(f"artifact directory already exists: {destination}; choose a new release before rebuilding")
    env = os.environ.copy()
    for name in ("RUSTY_V8_ARCHIVE", "RUSTY_V8_MIRROR", "RUSTY_V8_SRC_BINDING_PATH",
                 "V8_FORCE_DEBUG", "DOCS_RS", "DENO_TRYBUILD"):
        env.pop(name, None)
    env.update(V8_FROM_SOURCE="1", GN_ARGS=" ".join(recipe["gn_args"]))
    command = ["cargo", "build", "--release", "--locked", "--target", target,
               "--target-dir", str(work / "cargo-target"), "-p", "v8"]
    if args.jobs:
        command += ["--jobs", str(args.jobs)]
    run(*command, cwd=source, env=env)
    gn_out = work / "cargo-target" / target / "release" / "gn_out"
    package(recipe, inputs, digest, target, source, gn_out, destination)
    print(f"Packaged {recipe['release']} for {target}. Publishing remains manual.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
