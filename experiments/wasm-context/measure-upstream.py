#!/usr/bin/env python3
"""Link the same context probe against a published upstream rusty_v8 archive."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=Path, required=True,
                    help="matching upstream rusty_v8 source package")
parser.add_argument("--archive", type=Path, required=True)
parser.add_argument("--work", type=Path, required=True)
parser.add_argument("--cxx", default="clang++")
parser.add_argument("--linker", required=True)
parser.add_argument("--sdk", type=Path, required=True)
parser.add_argument("--pointer-compression", action="store_true",
                    help="match the enabled ABI in upstream 0.102.0")
args = parser.parse_args()
source, archive, work, sdk = (
    p.resolve() for p in (args.source, args.archive, args.work, args.sdk)
)
work.mkdir(parents=True, exist_ok=True)
assets = Path(__file__).resolve().parent
libcxx = source / "buildtools/third_party/libc++/trunk/include"
libcxxabi = source / "buildtools/third_party/libc++abi/trunk/include"
if not (libcxx / "cstdio").exists():
    libcxx = source / "third_party/libc++/src/include"
    libcxxabi = source / "third_party/libc++abi/src/include"
# Upstream archives contain their own libc++ with a private ABI namespace.
# Compile the public headers with that exact configuration, not the system ABI.
flags = [
    "-O2", "-DNDEBUG", "-std=c++20", "-fno-exceptions", "-fno-rtti",
    "--target=arm64-apple-macos", "-mmacos-version-min=11.0",
    "-isysroot", str(sdk), "-nostdinc++",
    "-I" + str(source / "v8"),
    "-I" + str(source / "v8/include"),
    "-I" + str(source / "buildtools/third_party/libc++"),
    "-isystem" + str(libcxx),
    "-isystem" + str(libcxxabi),
    "-DV8_ARRAY_BUFFER_INTERNAL_FIELD_COUNT=2",
    "-DV8_ARRAY_BUFFER_VIEW_INTERNAL_FIELD_COUNT=2",
    "-DV8_PROMISE_INTERNAL_FIELD_COUNT=1",
    "-D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_NONE",
]
if args.pointer_compression:
    flags += ["-DV8_COMPRESS_POINTERS", "-DV8_COMPRESS_POINTERS_IN_SHARED_CAGE"]
env = {key: value for key, value in os.environ.items()
       if key not in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH",
                      "OBJC_INCLUDE_PATH")}
obj, exe = work / "context.o", work / "context"
with (work / "build.log").open("w") as log:
    subprocess.run([args.cxx, *flags, "-DPROBE=3", "-c", str(assets / "probe.cc"),
                    "-o", str(obj)], env=env, stdout=log,
                   stderr=subprocess.STDOUT, check=True)
    subprocess.run([
        args.cxx, "--ld-path=" + args.linker,
        "--target=arm64-apple-macos", "-mmacos-version-min=11.0",
        "-isysroot", str(sdk), "-nostdlib++", "-Wl,-dead_strip",
        "-Wl,-ObjC", "-Wl,-mllvm,-enable-machine-outliner=never",
        str(obj), str(archive), "-framework", "Foundation",
        "-Wl,-map," + str(work / "context.map"), "-o", str(exe),
    ], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
subprocess.run([str(exe)], check=True, timeout=60)
stripped = work / "context-stripped"
shutil.copyfile(exe, stripped)
subprocess.run(["strip", "-x", str(stripped)], check=True)
result = {
    "archive_bytes": archive.stat().st_size,
    "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
    "context_probe_stripped_bytes": stripped.stat().st_size,
}
(work / "results.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
