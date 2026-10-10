#!/usr/bin/env python3
"""Apply our patches, build through rusty_v8, and package its native outputs."""

import argparse
import gzip
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parent


def run(*args, **kwargs):
    print("+", subprocess.list2cmdline([str(arg) for arg in args]), flush=True)
    return subprocess.run(args, check=True, **kwargs)


def package(source, gn_out, target, destination):
    native = "rusty_v8.lib" if target.endswith("windows-msvc") else "librusty_v8.a"
    archive = gn_out / "obj" / native
    bindings = gn_out / "src_binding.rs"
    with archive.open("rb") as file:
        if file.read(8) != b"!<arch>\n":
            raise ValueError("expected a complete static archive, not a thin archive")
    if not bindings.is_file() or not bindings.stat().st_size:
        raise ValueError("generated Rust bindings are missing")
    # Keep upstream notices, including those in the pinned submodules.
    tracked = subprocess.check_output(
        ["git", "ls-files", "--recurse-submodules", "-z"], cwd=source,
    ).decode().split("\0")
    notices = sorted(name for name in tracked if name and
                     Path(name).name.upper().startswith(("LICENSE", "COPYING", "NOTICE")))
    if "LICENSE" not in notices or "v8/LICENSE" not in notices:
        raise ValueError("upstream license notices are missing")
    destination.mkdir(parents=True, exist_ok=False)
    # Match rusty_v8's release asset names and compression.
    stem, suffix = native.rsplit(".", 1)
    with (destination / f"{stem}_release_{target}.{suffix}.gz").open("wb") as output:
        with gzip.GzipFile(filename="", fileobj=output, mode="wb", mtime=0) as compressed:
            with archive.open("rb") as file:
                shutil.copyfileobj(file, compressed)
    shutil.copyfile(bindings, destination / f"src_binding_release_{target}.rs")
    with (destination / f"licenses_{target}.tar.gz").open("wb") as output:
        with gzip.GzipFile(filename="", fileobj=output, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as bundle:
                for name in notices:
                    path = source / name
                    if path.is_file():
                        # Exclude build-machine ownership, paths, and timestamps.
                        info = tarfile.TarInfo(name)
                        info.size, info.mode = path.stat().st_size, 0o644
                        with path.open("rb") as file:
                            bundle.addfile(info, file)
    checksums = []
    for path in sorted(destination.iterdir()):
        with path.open("rb") as file:
            digest = hashlib.file_digest(file, "sha256").hexdigest()
        checksums.append(f"{digest}  {path.name}\n")
    (destination / f"SHA256SUMS_{target}").write_text("".join(checksums))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs", type=int, help="maximum parallel Cargo/Ninja jobs")
    args = parser.parse_args(argv)
    if args.jobs is not None and args.jobs < 1:
        parser.error("--jobs must be positive")
    rustc = subprocess.check_output(["rustc", "-vV"], text=True)
    target = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    destination = ROOT / "dist" / target
    if destination.exists():
        raise ValueError("output already exists; move dist/<target> aside before rebuilding")
    source = ROOT / "rusty_v8"
    run("git", "submodule", "update", "--init", "--recursive", "--depth=1", cwd=ROOT)
    for patch in sorted((ROOT / "patches").glob("*.patch")):
        # Repeated builds may reuse an already-patched checkout. An incompatible
        # patch still fails through git apply rather than being silently skipped.
        applied = subprocess.run(
            ["git", "apply", "--reverse", "--check", str(patch)],
            cwd=source, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        ).returncode == 0
        if not applied:
            run("git", "apply", patch, cwd=source)
    env = os.environ.copy()
    for name in ("DOCS_RS", "DENO_TRYBUILD", "V8_FORCE_DEBUG", "EXTRA_GN_ARGS"):
        env.pop(name, None)
    env.update(V8_FROM_SOURCE="1", GN_ARGS="v8_enable_i18n_support=false icu_use_data_file=false")
    command = ["cargo", "build", "--release", "--locked", "--target", target,
               "--target-dir", str(ROOT / "build"), "-p", "v8"]
    if args.jobs:
        command += ["--jobs", str(args.jobs)]
    run(*command, cwd=source, env=env)
    package(source, ROOT / "build" / target / "release" / "gn_out", target, destination)
    print(f"Packaged native artifacts in dist/{target}. Publishing remains manual.")


if __name__ == "__main__":
    main()
