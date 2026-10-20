#!/usr/bin/env python3
"""Select builtin and optional runtime entries for the Wasm-context prototype."""
import argparse
import difflib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--context", type=Path, required=True,
                    help="matching build.py output, including bootstrapper.o")
parser.add_argument("--work", type=Path, required=True)
parser.add_argument("--cxx", default="clang++")
parser.add_argument("--ar", default="llvm-ar")
parser.add_argument("--runtime-allowlist", action="store_true",
                    help="also select runtime implementations and inline fallbacks")
args = parser.parse_args()
source, out, context, work = (
    p.resolve() for p in (args.source, args.out, args.context, args.work)
)
if work in (out, context) or out in work.parents or context in work.parents:
    raise SystemExit("Use a separate output directory")
work.mkdir(parents=True, exist_ok=True)
assets = Path(__file__).resolve().parent
env = {k: v for k, v in os.environ.items()
       if k not in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH",
                    "OBJC_INCLUDE_PATH")}
commands = json.loads(subprocess.check_output(
    ["ninja", "-C", str(out), "-t", "compdb", "custom_wee8_cxx"],
    text=True))

# All three consumers use one selection. The original source tree is untouched;
# without the experiment define the new address macro selects the original.
edits = {
    "builtins.cc": [
        ('#include "src/builtins/builtins.h"',
         '#include "src/builtins/builtins.h"\n'
         '#include "cpp-builtin-selection.h"'),
        ("{#Name, Builtins::CPP, {FUNCTION_ADDR(Builtin_##Name)}},",
         "{#Name, Builtins::CPP, {WASM_CONTEXT_CPP_ENTRY(Name)}},"),
    ],
    "external-reference-table.cc": [
        ('#include "src/codegen/external-reference-table.h"',
         '#include "src/codegen/external-reference-table.h"\n'
         '#include "cpp-builtin-selection.h"'),
        ("#define DEF_ENTRY(Name, ...) FUNCTION_ADDR(&Builtin_##Name),",
         "#define DEF_ENTRY(Name, ...) WASM_CONTEXT_CPP_ENTRY(Name),"),
    ],
    "setup-builtins-internal.cc": [
        ('#include "src/builtins/builtins-inl.h"',
         '#include "src/builtins/builtins-inl.h"\n'
         '#include "cpp-builtin-selection.h"'),
        ("  // The C++ implementation and function-pointer table remain intact.",
         "  // Native entry selection is handled separately by the CPP tables."),
        ("  const auto omit_cpp_body = [](Builtin builtin) {\n",
         "  const auto omit_cpp_body = [](Builtin builtin) {\n"
         "#if defined(V8_WASM_CONTEXT_CPP_ALLOWLIST)\n"
         "    return !WasmContextKeepsCppBuiltin(builtin);\n"
         "#endif\n"),
        ("FUNCTION_ADDR(Builtin_##Name), #Name,",
         "WASM_CONTEXT_CPP_ENTRY(Name), #Name,"),
    ],
}
if args.runtime_allowlist:
    edits["runtime.cc"] = [
        ('#include "src/runtime/runtime.h"',
         '#include "src/runtime/runtime.h"\n#include "runtime-selection.h"'),
        ("Runtime::k##name, Runtime::RUNTIME, #name, FUNCTION_ADDR(Runtime_##name)",
         "Runtime::k##name, Runtime::RUNTIME, #name, WASM_CONTEXT_RUNTIME_ENTRY(name)"),
        ("FUNCTION_ADDR(Runtime_##name), number_of_args, result_size",
         "WASM_CONTEXT_RUNTIME_ENTRY(name), number_of_args, result_size"),
    ]
replacement_objects = {}
patches = []
for filename, changes in edits.items():
    entries = [e for e in commands if e["file"].endswith("/" + filename)]
    if len(entries) != 1:
        raise SystemExit(f"Expected one compilation command for {filename}")
    entry = entries[0]
    original = (out / entry["file"]).read_text()
    patched = original
    for before, after in changes:
        if patched.count(before) != 1:
            raise SystemExit(f"Source drift in {filename}: {before!r}")
        patched = patched.replace(before, after)
    src = work / filename
    src.write_text(patched)
    relative = (out / entry["file"]).resolve().relative_to(source)
    patches.extend(difflib.unified_diff(
        original.splitlines(True), patched.splitlines(True),
        fromfile="a/" + str(relative), tofile="b/" + str(relative)))
    obj = work / (src.stem + ".o")
    command = shlex.split(entry["command"])
    command[0] = args.cxx
    command[command.index("-c") + 1] = str(src)
    command[command.index("-o") + 1] = str(obj)
    if "-MF" in command:
        command[command.index("-MF") + 1] = str(obj.with_suffix(".d"))
    command += ["-DV8_WASM_CONTEXT_CPP_ALLOWLIST", "-I" + str(assets)]
    if args.runtime_allowlist:
        command += ["-DV8_WASM_CONTEXT_RUNTIME_ALLOWLIST"]
    with (work / (src.stem + ".log")).open("w") as log:
        subprocess.run(command, cwd=out, env=env, stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    replacement_objects[entry["output"]] = str(obj)
    print(f"Compiled {filename}", flush=True)
(work / ("runtime-entry-selection.patch" if args.runtime_allowlist else
         "cpp-entry-selection.patch")).write_text("".join(patches))

# Derive the inventory from V8's own macro expansion, not source-name patterns.
# A checked-in inventory makes new/renamed entries require review on a V8 bump.
configs = {}
for name in ("v8_base_without_compiler", "mksnapshot", "v8_snapshot"):
    config = {}
    for line in (out / f"custom_wee8/obj/v8/{name}.ninja").read_text().splitlines():
        if " = " in line:
            key, value = line.split(" = ", 1)
            config[key.strip()] = shlex.split(value)
    configs[name] = config
config = configs["v8_base_without_compiler"]
flags = sum((config[k] for k in ("defines", "include_dirs", "cflags", "cflags_cc")), [])
catalogs = [("cpp-builtin-catalog.cc", "cpp-builtins.tsv")]
if args.runtime_allowlist:
    catalogs += [("runtime-catalog.cc", "runtime-entries.tsv")]
for filename, inventory_name in catalogs:
    catalog = work / Path(filename).stem
    with catalog.with_suffix(".log").open("w") as log:
        subprocess.run([args.cxx, *flags, "-I" + str(assets),
                        str(assets / filename), "-o", str(catalog)],
                       cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    inventory = subprocess.check_output([str(catalog)], text=True)
    (work / inventory_name).write_text(inventory)
    if inventory != (assets / inventory_name).read_text():
        raise SystemExit(f"Inventory changed; review {inventory_name}")

link_rule = next(line for line in
                (out / "custom_wee8/obj/v8/mksnapshot.ninja").read_text().splitlines()
                if line.startswith("build custom_wee8/mksnapshot:"))
inputs = shlex.split(link_rule.split("custom_wee8_link ", 1)[1].split(" |", 1)[0])
bootstrap = [p for p in inputs if p.endswith("/bootstrapper.o")]
if len(bootstrap) != 1 or not replacement_objects.keys() <= set(inputs):
    raise SystemExit("Unexpected snapshot-generator link inputs")
replacement_objects[bootstrap[0]] = str(context / "bootstrapper.o")
inputs = [replacement_objects.get(p, p) for p in inputs]
config = configs["mksnapshot"]
flags = sum((config.get(k, []) for k in ("ldflags", "frameworks", "libs")), [])
with (work / "mksnapshot.link.log").open("w") as log:
    subprocess.run([args.cxx, *flags, *inputs, "-o", str(work / "mksnapshot")],
                   cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
with (work / "snapshot.log").open("w") as log:
    subprocess.run([
        str(work / "mksnapshot"), "--turbo_instruction_scheduling",
        "--stress-turbo-late-spilling", "--target_os=mac", "--target_arch=arm64",
        "--embedded_src", str(work / "embedded.S"), "--embedded_variant", "Default",
        "--random-seed", "314159265", "--startup_src", str(work / "snapshot.cc"),
        "--no-native-code-counters",
    ], cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
config = configs["v8_snapshot"]
flags = sum((config[k] for k in ("defines", "include_dirs", "cflags", "cflags_cc")), [])
with (work / "snapshot.build.log").open("w") as log:
    for filename in ("snapshot.cc", "embedded.S"):
        subprocess.run([args.cxx, *flags, "-c", str(work / filename),
                        "-o", str(work / (Path(filename).stem + ".o"))],
                       cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
archive = work / "libwee8.a"
shutil.copyfile(context / "libwee8.a", archive)
subprocess.run([
    args.ar, "r", str(archive),
    *[str(work / name) for name in
      ("builtins.o", "external-reference-table.o", "snapshot.o", "embedded.o")],
    *([str(work / "runtime.o")] if args.runtime_allowlist else []),
], check=True)
print("Entry selection and matching snapshot generated.", flush=True)
