#!/usr/bin/env python3
"""Keep the built base library and generated headers for extension-only builds."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
EXT_SOURCES = {"c-api.cc", "c-api.h", "wasm.hh", "c-api-strings.inc"}
BASE_INPUTS = [".gitmodules", ".gitattributes", ".cargo", "Cargo.toml", "Cargo.lock",
               "rust-toolchain.toml", "tools/native", "tools/prepare-source.py",
               "custom_wee8/BUILD.gn", "patches"]


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def identity(root=None, revision=None):
    root = ROOT if root is None else root
    if revision:
        tree = subprocess.check_output(["git", "-C", str(root), "ls-tree", "-rz", revision, "--",
                                        *BASE_INPUTS, "custom_wee8/src", "rusty_v8"])
        inputs = {}
        for entry in tree.split(b"\0"):
            if not entry:
                continue
            metadata, name = entry.split(b"\t", 1)
            _, kind, object_id = metadata.decode().split()
            name = name.decode()
            if name.removeprefix("custom_wee8/src/") in EXT_SOURCES:
                continue
            data = subprocess.check_output(["git", "-C", str(root), "cat-file", "blob", object_id]) if kind != "commit" else None
            inputs[name] = hashlib.sha256(data).hexdigest() if data is not None else object_id
        return hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()
    files = []
    for name in BASE_INPUTS:
        path = root / name
        if path.is_dir():
            files.extend(p for p in path.rglob("*") if p.is_file())
        else:
            files.append(path)
    sources = root / "custom_wee8/src"
    files += [p for p in sources.rglob("*")
              if p.is_file() and p.relative_to(sources).as_posix() not in EXT_SOURCES]
    inputs = {p.relative_to(root).as_posix(): digest(p) for p in files if p.exists()}
    inputs["rusty_v8"] = subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD:rusty_v8"], text=True).strip()
    return hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()


def portable_path(path):
    # Rust canonicalize supplies verbatim paths that GN cannot rebase.
    text = str(path)
    return "\\\\" + text[8:] if text.startswith("\\\\?\\UNC\\") else text.removeprefix("\\\\?\\")


def gn_output(args, *query):
    result = subprocess.run(
        [portable_path(args.gn), *map(portable_path, query), f"--script-executable={portable_path(sys.executable)}"],
        cwd=portable_path(ROOT / "rusty_v8"), stdout=subprocess.PIPE, text=True,
        env={**os.environ, "DEPOT_TOOLS_WIN_TOOLCHAIN": "0", "PYTHONDONTWRITEBYTECODE": "1"})
    if result.returncode:
        print(result.stdout, file=sys.stderr)
        result.check_returncode()
    return result.stdout


def configuration(args):
    source = ROOT / "rusty_v8"
    helper = runpy.run_path(str(source / "build/gn_helpers.py"))
    values = helper["FromGNArgs"]((args.out / "args.gn").read_text())
    clang = Path(values.pop("clang_base_path"))
    compiler = clang / "bin" / ("clang-cl.exe" if os.name == "nt" else "clang++")
    # Paths and the compiler cache wrapper can change without changing code.
    # The compiler and explicitly selected compiler-rt library are compared by bytes.
    values.pop("cc_wrapper", None)
    values.pop("wee8_base_dir")
    builtins = values.pop("clang_builtin_library", None)
    label = "//v8:wee8_base(//custom_wee8:custom_wee8)"
    defines = json.loads(gn_output(args, "desc", args.out, label, "defines", "--format=json"))[label]["defines"]
    return {"arguments": values, "compiler": digest(compiler),
            "compiler_rt": digest(builtins) if builtins else None, "defines": sorted(defines)}


def verify(base, target, config=None):
    manifest = json.loads((base / "base.json").read_text())
    if (manifest["schema"] != 1 or manifest["identity"] != identity()
            or manifest["target"] != target or manifest["runner_image"] != os.environ.get("ImageVersion", "")):
        raise ValueError("base inputs differ; build a matching base")
    if config is not None and manifest["configuration"] != config:
        raise ValueError("base compiler or GN configuration differs")
    files = {p.relative_to(base).as_posix() for p in base.rglob("*") if p.is_file()}
    if files != set(manifest["files"]) | {"base.json"}:
        raise ValueError("base file inventory differs")
    for name, expected in manifest["files"].items():
        if digest(base / name) != expected:
            raise ValueError(f"base checksum mismatch: {name}")
    return manifest


def prepare_base(args, base):
    receipt = args.out / "wee8-base-config.json"
    # Read/validate toolchain metadata before invoking any engine compiler.
    config = configuration(args)
    if base.exists():
        verify(base, args.target, config)
    receipt.write_text(json.dumps(config, indent=2) + "\n")

def save_base(args, base):
    archive = "wee8_base.lib" if os.name == "nt" else "libwee8_base.a"
    receipt = args.out / "wee8-base-config.json"
    if base.exists():
        raise ValueError("base already exists; preserve it before rebuilding")
    staging = base.with_name(base.name + ".partial")
    staging.mkdir(parents=True)
    shutil.copy2(args.out / "custom_wee8/obj/v8" / archive, staging / archive)
    shutil.copytree(args.out / "custom_wee8/gen", staging / "gen")
    files = {p.relative_to(staging).as_posix(): digest(p) for p in staging.rglob("*") if p.is_file()}
    manifest = {"schema": 1, "identity": identity(), "target": args.target, "archive": archive,
                "runner_image": os.environ.get("ImageVersion", ""), "files": files,
                "configuration": json.loads(receipt.read_text()),
                "producer_revision": subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip(),
                "producer_run": os.environ.get("GITHUB_RUN_ID", "")}
    (staging / "base.json").write_text(json.dumps(manifest, indent=2) + "\n")
    staging.rename(base)

def verify_extension(args, base):
    manifest = verify(base, args.target, configuration(args))
    label = "//custom_wee8/ext:wee8_ext(//custom_wee8:custom_wee8)"
    defines = json.loads(gn_output(args, "desc", args.out, label, "defines", "--format=json"))[label]["defines"]
    if sorted(defines) != manifest["configuration"]["defines"]:
        raise ValueError("extension ABI definitions differ from the base")
    label = "//:wee8-ext-build"
    deps = json.loads(gn_output(args, "desc", args.out, label, "deps", "--all", "--format=json"))[label]["deps"]
    if not deps or any(not dep.startswith("//custom_wee8/ext:") for dep in deps):
        raise ValueError(f"extension unexpectedly depends on engine compilation: {deps}")
    print(f"Reusing base {manifest['identity']}; {len(deps)} extension/probe targets, no engine targets", flush=True)

def merge_archives(args, base):
    archive = "wee8_base.lib" if os.name == "nt" else "libwee8_base.a"
    manifest = verify(base, args.target)
    destination = ROOT / "build/wee8" / args.target
    destination.mkdir(parents=True, exist_ok=True)
    combined = destination / ("wee8.lib" if os.name == "nt" else "libwee8.a")
    ext = args.out / "custom_wee8/obj/custom_wee8/ext" / ("wee8_ext.lib" if os.name == "nt" else "libwee8_ext.a")
    helper = runpy.run_path(str(ROOT / "rusty_v8/build/gn_helpers.py"))
    clang = Path(helper["FromGNArgs"]((args.out / "args.gn").read_text())["clang_base_path"])
    if os.name == "nt":
        subprocess.run([str(clang / "bin/lld-link.exe"), "/lib", f"/out:{combined}", str(base / archive), str(ext)], check=True)
    elif args.target.endswith("apple-darwin"):
        subprocess.run(["libtool", "-static", "-o", str(combined), str(base / archive), str(ext)], check=True)
    else:
        # MRI filenames are relative so checkout spaces require no quoting convention.
        shutil.copy2(base / archive, destination / "base.a")
        shutil.copy2(ext, destination / "ext.a")
        subprocess.run([str(clang / "bin/llvm-ar"), "-M"], cwd=destination,
                       input="CREATE libwee8.a\nADDLIB base.a\nADDLIB ext.a\nSAVE\nEND\n", text=True, check=True)
        (destination / "base.a").unlink()
        (destination / "ext.a").unlink()
    if digest(base / archive) != manifest["files"][archive]:
        raise ValueError("base changed during extension build")
    (destination / "build.json").write_text(json.dumps({
        "base_identity": manifest["identity"], "base_sha256": digest(base / archive),
        "base_producer_revision": manifest["producer_revision"], "base_producer_run": manifest["producer_run"],
        "ext_sha256": digest(ext), "combined_sha256": digest(combined)}, indent=2) + "\n")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["identity", "prepare", "save", "verify", "merge"])
    parser.add_argument("--target")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--gn")
    args = parser.parse_args()
    if args.command == "identity":
        print(identity())
        return
    if not args.target or not args.out or not args.gn:
        parser.error("--target, --out and --gn are required for build stages")
    args.out = args.out.resolve()
    base = ROOT / "target/wee8-base" / args.target
    if args.command == "prepare":
        prepare_base(args, base)
    elif args.command == "save":
        save_base(args, base)
    elif args.command == "verify":
        verify_extension(args, base)
    else:
        merge_archives(args, base)


if __name__ == "__main__":
    main()
