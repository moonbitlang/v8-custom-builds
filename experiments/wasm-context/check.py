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
parser.add_argument("--cpp-entry-contract", action="store_true",
                    help="validate candidate C++ entry selection and named traps")
parser.add_argument("--runtime-checks", action="store_true",
                    help="validate runtime registry and Wasm boundary guests")
parser.add_argument("--wasm-tools", default="wasm-tools")
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
contracts = []
if args.cpp_entry_contract:
    contracts.append((
        "cpp-entry-contract", "cpp_entry_contract", "unsupported_cpp_traps_passed",
        "Unsupported Wasm-context C++ builtin: ",
        ("DateConstructor", "TemporalPlainDateConstructor", "JsonStringify",
         "ArrayBufferConstructor", "StringPrototypeToLowerCase")))
if args.runtime_checks:
    contracts.append((
        "runtime-entry-contract", "runtime_entry_contract", "unsupported_runtime_traps_passed",
        "Unsupported Wasm-context runtime: ",
        ("DateCurrentTime", "IsInvalidTemporalCalendarField", "DynamicImportCall",
         "AsyncFunctionAwait", "DebugBreakOnBytecode", "LoadLookupSlotForCall")))
for name, result_key, trap_key, diagnostic, negative_cases in contracts:
    import resource
    import signal

    output = work / "candidate"
    obj, exe = output / (name + ".o"), output / name
    with (output / (name + ".build.log")).open("w") as log:
        subprocess.run([args.cxx, *flags, "-I" + str(assets), "-c",
                        str(assets / (name + ".cc")), "-o", str(obj)],
                       cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
        subprocess.run([args.cxx, *link_flags, str(obj), str(candidate),
                        "-o", str(exe)], cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    results["candidate"][result_key] = subprocess.run(
        [str(exe)], capture_output=True, text=True, check=True, timeout=60
    ).stdout.strip()
    # Deliberate unsupported-entry death tests must identify their own entry.
    # Disable core dumps for these child processes.
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    for entry_name in negative_cases:
        result = subprocess.run([str(exe), entry_name], capture_output=True,
                                text=True, timeout=60)
        if (result.returncode not in (-signal.SIGABRT, -signal.SIGTRAP,
                                      -signal.SIGILL) or
                diagnostic + entry_name not in result.stderr):
            raise RuntimeError(f"Unexpected unsupported-entry outcome: {entry_name}")
    results["candidate"][trap_key] = list(negative_cases)
    (work / "results.json").write_text(json.dumps(results, indent=2) + "\n")
if args.runtime_checks:
    guests = []
    for name in ("runtime-boundaries", "runtime-shared-memory"):
        guest = work / (name + ".wasm")
        subprocess.run([args.wasm_tools, "parse", str(assets / (name + ".wat")),
                        "-o", str(guest)], check=True)
        guests.append(str(guest))
    for variant, archive in (("baseline", baseline), ("candidate", candidate)):
        output = work / variant
        obj, exe = output / "boundaries.o", output / "boundaries"
        with (output / "boundaries.build.log").open("w") as log:
            subprocess.run([args.cxx, *flags, "-c",
                            str(assets / "runtime-boundaries.cc"), "-o", str(obj)],
                           cwd=out, env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
            subprocess.run([args.cxx, *link_flags, str(obj), str(archive),
                            "-o", str(exe)], cwd=out, env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
        for mode in ("--liftoff-only --no-wasm-lazy-compilation",
                     "--no-liftoff --no-wasm-lazy-compilation",
                     "--wasm-lazy-compilation",
                     "--wasm-sync-tier-up --wasm-tiering-budget=1"):
            subprocess.run([str(exe), mode, *guests], check=True, timeout=60)
        results[variant]["runtime_boundary_modes_passed"] = 4
        (work / "results.json").write_text(json.dumps(results, indent=2) + "\n")
print(json.dumps(results, indent=2))
