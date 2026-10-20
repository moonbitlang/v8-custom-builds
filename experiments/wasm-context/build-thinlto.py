#!/usr/bin/env python3
"""Recompile only the existing wee8 archive inputs as LLVM ThinLTO bitcode."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import threading
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--baseline", type=Path, required=True,
                    help="directory containing matching snapshot.cc and embedded.o")
parser.add_argument("--candidate", type=Path, required=True,
                    help="build.py output containing the allowlist snapshot")
parser.add_argument("--work", type=Path, required=True)
parser.add_argument("--ar", default="llvm-ar")
parser.add_argument("--jobs", type=int, default=4)
args = parser.parse_args()
out, baseline, candidate, work = (
    p.resolve() for p in (args.out, args.baseline, args.candidate, args.work)
)
if work == out or out in work.parents:
    raise SystemExit("Use a separate work directory, outside the existing GN output")
work.mkdir(parents=True, exist_ok=True)
assets = Path(__file__).resolve().parent
commands = json.loads(subprocess.check_output(
    ["ninja", "-C", str(out), "-t", "compdb", "custom_wee8_cxx", "custom_wee8_cc"],
    text=True))
by_output = {c["output"]: c for c in commands}
rule = next(line for line in (out / "custom_wee8/obj/v8/wee8.ninja").read_text().splitlines()
            if line.startswith("build custom_wee8/obj/v8/libwee8.a:"))
inputs = shlex.split(rule.split("custom_wee8_alink ", 1)[1].split(" |", 1)[0])
bootstrap = "custom_wee8/obj/v8/v8_base_without_compiler/bootstrapper.o"
snapshot = "custom_wee8/obj/v8/v8_snapshot/snapshot.o"
embedded = "custom_wee8/obj/v8/v8_snapshot/embedded.o"
if set(inputs) - by_output.keys() != {embedded}:
    raise SystemExit("Unexpected non-C/C++ archive inputs")

env = {key: value for key, value in os.environ.items()
       if key not in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH",
                      "OBJC_INCLUDE_PATH")}
failed = threading.Event()
started = time.monotonic()

def compile_input(item):
    output_name, variant = item
    if failed.is_set():
        return "cancelled"
    entry = by_output[output_name]
    command = shlex.split(entry["command"])
    obj = work / variant / output_name
    obj.parent.mkdir(parents=True, exist_ok=True)
    # Preserve the GN compilation settings, changing only output locations,
    # the explicit prototype sources, and the ThinLTO mode.
    command[command.index("-o") + 1] = str(obj)
    if "-MF" in command:
        command[command.index("-MF") + 1] = str(obj.with_suffix(".d"))
    if output_name == snapshot:
        src = (candidate if variant == "candidate" else baseline) / "snapshot.cc"
    elif output_name == bootstrap:
        src = candidate / "bootstrapper.cc"
        if variant == "candidate":
            command += ["-DV8_WASM_CONTEXT_PROTOTYPE", "-I" + str(assets)]
    else:
        src = out / entry["file"]
    command[command.index("-c") + 1] = str(src)
    command += ["-flto=thin", "-fno-color-diagnostics"]
    # Resume only within an unchanged build: headers and compiler binaries are
    # not fingerprinted. Use a fresh work directory after changing either.
    digest = hashlib.sha256(
        json.dumps(command).encode() + src.read_bytes()
    ).hexdigest()
    stamp = obj.with_suffix(".sha256")
    if obj.exists() and stamp.exists() and stamp.read_text() == digest:
        return "cached"
    with obj.with_suffix(".log").open("w") as log:
        result = subprocess.run(command, cwd=out, env=env, stdout=log,
                                stderr=subprocess.STDOUT)
    if result.returncode:
        failed.set()
        raise RuntimeError(f"Compilation failed: {variant}/{output_name}")
    # A ThinLTO archive must contain bitcode, not a native-object relink.
    if obj.read_bytes()[:4] not in (b"BC\xc0\xde", b"\xde\xc0\x17\x0b"):
        failed.set()
        raise RuntimeError(f"Expected LLVM bitcode: {output_name}")
    stamp.write_text(digest)
    return "compiled"

jobs = [(p, "common") for p in inputs if p != embedded]
jobs += [(bootstrap, "candidate"), (snapshot, "candidate")]
counts = {"compiled": 0, "cached": 0, "cancelled": 0}
with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
    futures = [pool.submit(compile_input, job) for job in jobs]
    for number, future in enumerate(concurrent.futures.as_completed(futures), 1):
        counts[future.result()] += 1
        if number % 25 == 0 or number == len(jobs):
            print(f"{number}/{len(jobs)} inputs; {time.monotonic() - started:.0f}s",
                  flush=True)

# Assembly is already machine code; use the matching regenerated object.
# All C/C++ archive inputs, including both snapshot payloads, are bitcode.
for variant in ("baseline", "candidate"):
    folder = work / variant
    folder.mkdir(parents=True, exist_ok=True)
    embedded_obj = folder / "embedded.o"
    shutil.copyfile((candidate if variant == "candidate" else baseline) / "embedded.o",
                    embedded_obj)
    objects = [
        embedded_obj if p == embedded else
        work / ("candidate" if variant == "candidate" and p in (bootstrap, snapshot)
                else "common") / p
        for p in inputs
    ]
    archive = folder / "libwee8.a"
    archive.unlink(missing_ok=True)
    # Preserve duplicate basenames and their ordering in the original archive.
    subprocess.run([args.ar, "qc", str(archive), *map(str, objects)], check=True)
    subprocess.run([args.ar, "s", str(archive)], check=True)
metadata = {
    "archive_input_count": len(inputs),
    "bitcode_inputs_per_archive": len(inputs) - 1,
    "native_assembly_inputs_per_archive": 1,
    "compilation": counts,
    "elapsed_seconds": round(time.monotonic() - started, 1),
}
(work / "build-results.json").write_text(json.dumps(metadata, indent=2) + "\n")
print(json.dumps(metadata, indent=2))
