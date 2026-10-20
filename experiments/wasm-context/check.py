#!/usr/bin/env python3
"""Compare matching baseline/candidate archives using existing build flags."""
import argparse
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--baseline", type=Path, required=True)
parser.add_argument("--candidate", type=Path, required=True)
parser.add_argument("--work", type=Path, required=True)
parser.add_argument("--cxx", default="clang++")
parser.add_argument("--linker", help="explicit linker executable")
parser.add_argument("--thin-lto", action="store_true")
parser.add_argument("--lto-opt-level", type=int, choices=range(4), default=2)
parser.add_argument("--probes", nargs="+",
                    choices=("empty", "initialize", "isolate", "context",
                             "c-api", "c-api-gc"),
                    default=("empty", "initialize", "isolate", "context",
                             "c-api", "c-api-gc"))
args = parser.parse_args()
source, out, baseline, candidate, work = (
    path.resolve() for path in
    (args.source, args.out, args.baseline, args.candidate, args.work)
)
assets = Path(__file__).resolve().parent
config = {}
for line in (out / "custom_wee8/obj/wee8-runtime-check.ninja").read_text().splitlines():
    if " = " in line:
        key, value = line.split(" = ", 1)
        config[key.strip()] = shlex.split(value)
flags = sum((config[key] for key in
             ("defines", "include_dirs", "cflags", "cflags_cc")), [])
flags += ["-I" + str(source / "v8/third_party/wasm-api")]
link_flags = config["ldflags"] + config["frameworks"] + config["libs"]
if args.linker:
    # Apple's warning-suppression option is not implemented by ld64.lld 18.
    link_flags = [f for f in link_flags if f != "-Wl,-no_warn_duplicate_libraries"]
    link_flags += ["--ld-path=" + args.linker]
if args.thin_lto:
    if not args.linker:
        raise SystemExit("Specify the LLVM linker matching the bitcode compiler")
    flags += ["-flto=thin"]
    cache = work / "thinlto-cache"
    cache.mkdir(parents=True, exist_ok=True)
    link_flags += ["-flto=thin", "-Wl,-cache_path_lto," + str(cache),
                   "-Wl,--thinlto-jobs=6",
                   "-Wl,--lto-O" + str(args.lto_opt_level)]
env = {key: value for key, value in os.environ.items()
       if key not in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH",
                      "OBJC_INCLUDE_PATH")}
results = {}
for variant, archive in (("baseline", baseline), ("candidate", candidate)):
    output = work / variant
    output.mkdir(parents=True, exist_ok=True)
    results[variant] = {}
    for number, name in enumerate(
            ("empty", "initialize", "isolate", "context", "c-api", "c-api-gc")):
        if name not in args.probes:
            continue
        obj, exe = output / (name + ".o"), output / name
        with (output / (name + ".log")).open("w") as log:
            subprocess.run([args.cxx, *flags, f"-DPROBE={number}", "-c",
                            str(assets / "probe.cc"), "-o", str(obj)],
                           cwd=out, env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
            subprocess.run([args.cxx, *link_flags, str(obj), str(archive),
                            "-Wl,-map," + str(output / (name + ".map")),
                            "-o", str(exe)],
                           cwd=out, env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
        subprocess.run([str(exe)], check=True, timeout=60)
        stripped = output / (name + "-stripped")
        shutil.copyfile(exe, stripped)
        subprocess.run(["strip", "-x", str(stripped)], check=True)
        results[variant][name] = {"stripped_bytes": stripped.stat().st_size}
        (work / "results.json").write_text(json.dumps(results, indent=2) + "\n")

    with (output / "contract.log").open("w") as log:
        obj, exe = output / "contract.o", output / "contract"
        subprocess.run([args.cxx, *flags, "-c",
                        str(assets / "context-contract.cc"), "-o", str(obj)],
                       cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
        subprocess.run([args.cxx, *link_flags, str(obj), str(archive),
                        "-o", str(exe)],
                       cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
        results[variant]["heap"] = [
            subprocess.run(
                [str(exe)] + (["allowlist"] if variant == "candidate" else []),
                capture_output=True, text=True, check=True, timeout=60
            ).stdout.strip()
            for _ in range(3)
        ]
    for mode in ("--liftoff-only --no-wasm-lazy-compilation",
                 "--no-liftoff --no-wasm-lazy-compilation",
                 "--wasm-lazy-compilation",
                 "--wasm-sync-tier-up --wasm-tiering-budget=1"):
        for name in ("c-api", "c-api-gc"):
            if name in args.probes:
                subprocess.run([str(output / name), mode], check=True, timeout=60)
    results[variant]["wasm_modes_passed"] = (
        4 if any(name in args.probes for name in ("c-api", "c-api-gc")) else 0
    )
    (work / "results.json").write_text(json.dumps(results, indent=2) + "\n")
print(json.dumps(results, indent=2))
