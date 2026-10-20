#!/usr/bin/env python3
"""Compare generated identities/metadata and invoke every omitted JS entry."""
import argparse
import json
import os
from pathlib import Path
import resource
import shlex
import signal
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--baseline", type=Path, required=True)
parser.add_argument("--candidate", type=Path, required=True)
parser.add_argument("--work", type=Path, required=True)
parser.add_argument("--cxx", default="clang++")
parser.add_argument("--linker", required=True)
args = parser.parse_args()
out, baseline, candidate, work = (
    path.resolve() for path in (args.out, args.baseline, args.candidate, args.work)
)
assets = Path(__file__).resolve().parent
config = {}
for line in (out / "custom_wee8/obj/wee8-runtime-check.ninja").read_text().splitlines():
    if " = " in line:
        key, value = line.split(" = ", 1)
        config[key.strip()] = shlex.split(value)
flags = sum((config[key] for key in
             ("defines", "include_dirs", "cflags", "cflags_cc")), [])
link_flags = [flag for flag in
              config["ldflags"] + config["frameworks"] + config["libs"]
              if flag != "-Wl,-no_warn_duplicate_libraries"]
link_flags += ["--ld-path=" + args.linker]
env = {key: value for key, value in os.environ.items()
       if key not in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH",
                      "OBJC_INCLUDE_PATH")}
catalogs = {}
for variant, archive in (("baseline", baseline), ("candidate", candidate)):
    output = work / variant
    output.mkdir(parents=True, exist_ok=True)
    obj, exe = output / "generated.o", output / "generated"
    with (output / "build.log").open("w") as log:
        subprocess.run([args.cxx, *flags, "-I" + str(assets), "-c",
                        str(assets / "generated-builtin-contract.cc"), "-o", str(obj)],
                       cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
        subprocess.run([args.cxx, *link_flags, str(obj), str(archive),
                        "-o", str(exe)], cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    catalog = subprocess.check_output([str(exe)], text=True, timeout=60)
    (output / "code.tsv").write_text(catalog)
    catalogs[variant] = [line.split("\t") for line in catalog.splitlines()]

previous, current = catalogs["baseline"], catalogs["candidate"]
if len(previous) != len(current):
    raise SystemExit("Builtin count changed")
omitted = []
unselected_size_changes = []
for old, new in zip(previous, current):
    if old[:4] != new[:4]:
        raise SystemExit(f"Builtin identity/kind/argc changed: {old[1]}")
    if new[5] == "1":
        omitted.append(new[1])
        if int(new[4]) > int(old[4]):
            raise SystemExit(f"Unsupported body grew: {new[1]}")
    elif old[4] != new[4]:
        # Fewer generated constants can shorten constant-table loads even in
        # unchanged generators. Record these changes for the comparison report;
        # instruction-size equality is not an invariant of snapshot generation.
        unselected_size_changes.append({
            "name": new[1], "before": int(old[4]), "after": int(new[4])
        })

# Every omitted body must trap through its own JS function entry. Reject a
# normal return, a generic fatal fallback, or an unrelated memory-access fault.
resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
for name in omitted:
    result = subprocess.run([str(work / "candidate/generated"), name],
                            capture_output=True, text=True, timeout=60)
    if (result.returncode not in (-signal.SIGTRAP, -signal.SIGILL) or
            f"invoking:{name}\n" not in result.stderr or
            "Unsupported generated entry returned" in result.stderr):
        raise SystemExit(f"Unexpected trap outcome for {name}: {result.returncode}\n"
                         + result.stderr)
result = {
    "builtin_identities": len(current),
    "identity_kind_and_argc_unchanged": True,
    "unselected_instruction_size_changes": unselected_size_changes,
    "unsupported_generated_entries_passed": omitted,
}
(work / "results.json").write_text(json.dumps(result, indent=2) + "\n")
print(f"{len(current)} identities preserved; {len(omitted)} generated entries trap")
