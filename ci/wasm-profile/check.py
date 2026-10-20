#!/usr/bin/env python3
"""Verify profile inventories, snapshot identities and native Wasm boundaries."""
import argparse
import difflib
import os
from pathlib import Path
import signal
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("probes", type=Path)
args = parser.parse_args()
assets = Path(__file__).resolve().parent
suffix = ".exe" if os.name == "nt" else ""
if os.name == "nt":
    import ctypes
    ctypes.windll.kernel32.SetErrorMode(0x0002 | 0x8000)
    traps = {0x80000003, -2147483645, 0xC000001D, -1073741795}
    fatal_statuses = traps
else:
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    traps = {-signal.SIGTRAP, -signal.SIGILL}
    fatal_statuses = traps | {-signal.SIGABRT}

# Inventories record names, kinds, signatures and selection. IDs and machine
# code sizes come from this target; never freeze another architecture's IDs.
for program, inventory in [("generated-builtin-catalog", "builtin-inventory.tsv"),
                           ("runtime-catalog", "runtime-inventory.tsv")]:
    executable = (args.probes / (program + suffix)).resolve()
    actual = subprocess.check_output([str(executable)], text=True, timeout=60)
    if program == "generated-builtin-catalog":
        rows = [row.split("\t") for row in actual.splitlines()]
        actual = "".join("\t".join([r[0], r[1], r[3], r[4]]) + "\n"
                         for r in rows if r[0] in ("CPP", "TFJ", "TSJ"))
    expected = (assets / inventory).read_text()
    if actual != expected:
        raise SystemExit("Review the changed profile inventory:\n" + "".join(
            difflib.unified_diff(expected.splitlines(True), actual.splitlines(True),
                                 fromfile=inventory, tofile="compiled catalog")))

# These inspect the archive's actual tables, including inline aliases and
# external references, rather than just re-expanding a selection header.
for program in ["cpp-entry-contract", "runtime-entry-contract"]:
    executable = (args.probes / (program + suffix)).resolve()
    subprocess.run([str(executable)], check=True, timeout=60)

executable = (args.probes / ("generated-builtin-contract" + suffix)).resolve()
rows = subprocess.check_output([str(executable)], text=True, timeout=60).splitlines()
omitted = [row.split("\t")[1] for row in rows if row.split("\t")[-1] == "1"]
expected_omitted = [row.split("\t")[1] for row in (assets / "builtin-inventory.tsv").read_text().splitlines()
                    if row.split("\t")[0] in ("CPP", "TFJ") and row.split("\t")[2] == "omit"]
if set(omitted) != set(expected_omitted):
    raise SystemExit("Snapshot selection differs from the compiled builtin inventory")
for name in omitted:
    result = subprocess.run([str(executable), name], capture_output=True,
                            text=True, timeout=60)
    if ("invoking:" + name not in result.stderr or result.returncode not in traps
            or "Unsupported generated entry returned" in result.stderr):
        raise SystemExit(f"{name}: expected generated trap, got {result.returncode}\n{result.stderr}")
print(f"{len(rows)} distinct builtin identities; {len(omitted)} generated traps passed")

# Both runtime return conventions and representative former library roots must
# terminate at their named typed entry, never at a null pointer or fake result.
for program, prefix, names in [
    ("cpp-entry-contract", "Unsupported Wasm-context C++ builtin: ",
     ["DateConstructor", "TemporalPlainDateConstructor", "JsonStringify",
      "ArrayBufferConstructor", "StringPrototypeToLowerCase"]),
    ("runtime-entry-contract", "Unsupported Wasm-context runtime: ",
     ["DateCurrentTime", "IsInvalidTemporalCalendarField", "DynamicImportCall",
      "AsyncFunctionAwait", "LoadLookupSlotForCall", "DebugBreakOnBytecode"]),
]:
    executable = (args.probes / (program + suffix)).resolve()
    for name in names:
        result = subprocess.run([str(executable), name], capture_output=True,
                                text=True, timeout=60)
        if (prefix + name not in result.stderr
                or result.returncode not in fatal_statuses):
            raise SystemExit(f"{name}: expected named entry trap, got {result.returncode}\n{result.stderr}")

# Checked-in binary fixtures are paired with readable WAT sources. No JS-global
# constructor is involved: compile, serialize, instantiate and call through C API.
executable = (args.probes / ("runtime-boundaries" + suffix)).resolve()
for mode in ["--liftoff-only --no-wasm-lazy-compilation",
             "--no-liftoff --no-wasm-lazy-compilation",
             "--wasm-lazy-compilation",
             "--wasm-sync-tier-up --wasm-tiering-budget=1"]:
    subprocess.run([str(executable), mode, str(assets / "runtime-boundaries.wasm"),
                    str(assets / "runtime-shared-memory.wasm")], check=True, timeout=120)
print("Native runtime boundaries passed in four Wasm compilation modes")
