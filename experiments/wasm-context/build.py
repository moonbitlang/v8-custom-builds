#!/usr/bin/env python3
"""Relink an isolated ARM64 macOS prototype from an existing custom_wee8 build."""
import argparse
import difflib
import os
from pathlib import Path
import shlex
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=Path, required=True, help="rusty_v8 checkout")
parser.add_argument("--out", type=Path, required=True, help="existing GN output")
parser.add_argument("--archive", type=Path, required=True, help="matching libwee8.a")
parser.add_argument("--work", type=Path, required=True, help="separate output directory")
parser.add_argument("--cxx", default="clang++")
parser.add_argument("--ar", default="llvm-ar")
args = parser.parse_args()
source, out, archive, work = (
    path.resolve() for path in (args.source, args.out, args.archive, args.work)
)
work.mkdir(parents=True, exist_ok=True)
assets = Path(__file__).resolve().parent
env = {key: value for key, value in os.environ.items()
       if key not in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH",
                      "OBJC_INCLUDE_PATH")}

original = (source / "v8/src/init/bootstrapper.cc").read_text()
patched = original
# Each insertion is anchored to an exact upstream statement and fails closed
# after source drift. Normal bootstrap bodies are preserved verbatim.
edits = [
    ("  void InitializeGlobal(Handle<JSGlobalObject> global_object,",
     "  void InitializeWasmContext(Handle<JSGlobalObject> global_object,\n"
     "                             Handle<JSFunction> empty_function);\n"
     "  void InitializeGlobal(Handle<JSGlobalObject> global_object,"),
    ("Genesis::Genesis(Isolate* isolate,\n"
     "                 MaybeHandle<JSGlobalProxy> maybe_global_proxy,\n"
     "                 v8::Local<v8::ObjectTemplate> global_proxy_template,\n",
     '#ifdef V8_WASM_CONTEXT_PROTOTYPE\n'
     '#include "wasm-context-bootstrap.inc"\n'
     '#endif\n\n'
     "Genesis::Genesis(Isolate* isolate,\n"
     "                 MaybeHandle<JSGlobalProxy> maybe_global_proxy,\n"
     "                 v8::Local<v8::ObjectTemplate> global_proxy_template,\n"),
    ("    CreateRoots();\n    MathRandom::InitializeContext",
     "    CreateRoots();\n"
     "#ifdef V8_WASM_CONTEXT_PROTOTYPE\n"
     "    Handle<JSFunction> empty_function = CreateEmptyFunction();\n"
     "    CreateSloppyModeFunctionMaps(empty_function);\n"
     "    CreateStrictModeFunctionMaps(empty_function);\n"
     "    CreateObjectFunction(empty_function);\n"
     "    Handle<JSGlobalObject> global_object =\n"
     "        CreateNewGlobals(global_proxy_template, global_proxy);\n"
     "    InitializeMapCaches();\n"
     "    InitializeWasmContext(global_object, empty_function);\n"
     "    if (!ConfigureGlobalObject(global_proxy_template)) return;\n"
     "#else\n"
     "    MathRandom::InitializeContext"),
    ("\n#ifdef V8_ENABLE_WEBASSEMBLY\n"
     "    WasmJs::PrepareForSnapshot(isolate);",
     "\n#endif  // V8_WASM_CONTEXT_PROTOTYPE\n"
     "#ifdef V8_ENABLE_WEBASSEMBLY\n"
     "    WasmJs::PrepareForSnapshot(isolate);"),
    ("    InitializeExperimentalGlobal();",
     "#ifndef V8_WASM_CONTEXT_PROTOTYPE\n"
     "    InitializeExperimentalGlobal();"),
    ("        string_function_prototype->map());\n  }\n",
     "        string_function_prototype->map());\n"
     "#endif  // V8_WASM_CONTEXT_PROTOTYPE\n  }\n"),
    ("  WasmJs::Install(isolate, v8_flags.expose_wasm);",
     "#ifdef V8_WASM_CONTEXT_PROTOTYPE\n"
     "  WasmJs::Install(isolate, false);\n"
     "#else\n"
     "  WasmJs::Install(isolate, v8_flags.expose_wasm);\n"
     "#endif"),
]
for before, after in edits:
    if patched.count(before) != 1:
        raise SystemExit(f"Expected one bootstrap anchor: {before!r}")
    patched = patched.replace(before, after)
(work / "bootstrapper.cc").write_text(patched)
(work / "bootstrap.patch").write_text("".join(difflib.unified_diff(
    original.splitlines(True), patched.splitlines(True),
    fromfile="a/v8/src/init/bootstrapper.cc",
    tofile="b/v8/src/init/bootstrapper.cc")))

configs = {}
for name in ("v8_base_without_compiler", "mksnapshot", "v8_snapshot"):
    configs[name] = {}
    for line in (out / f"custom_wee8/obj/v8/{name}.ninja").read_text().splitlines():
        if " = " in line:
            key, value = line.split(" = ", 1)
            configs[name][key.strip()] = shlex.split(value)

config = configs["v8_base_without_compiler"]
flags = sum((config[key] for key in
             ("defines", "include_dirs", "cflags", "cflags_cc")), [])
with (work / "bootstrap.build.log").open("w") as log:
    subprocess.run([args.cxx, *flags, "-DV8_WASM_CONTEXT_PROTOTYPE",
                    "-I" + str(assets), "-c", str(work / "bootstrapper.cc"),
                    "-o", str(work / "bootstrapper.o")],
                   cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT,
                   check=True)

link_rule = next(line for line in
                (out / "custom_wee8/obj/v8/mksnapshot.ninja").read_text().splitlines()
                if line.startswith("build custom_wee8/mksnapshot:"))
inputs = shlex.split(link_rule.split("custom_wee8_link ", 1)[1].split(" |", 1)[0])
if sum(path.endswith("/bootstrapper.o") for path in inputs) != 1:
    raise SystemExit("Expected one bootstrapper input")
inputs = [str(work / "bootstrapper.o") if path.endswith("/bootstrapper.o")
          else path for path in inputs]
for path in inputs:
    if not (out / path).exists():
        raise SystemExit(f"Missing prerequisite: {path}")
config = configs["mksnapshot"]
flags = sum((config.get(key, []) for key in ("ldflags", "frameworks", "libs")), [])
with (work / "mksnapshot.link.log").open("w") as log:
    subprocess.run([args.cxx, *flags, *inputs, "-o", str(work / "mksnapshot")],
                   cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT,
                   check=True)
with (work / "snapshot.log").open("w") as log:
    subprocess.run([
        str(work / "mksnapshot"), "--turbo_instruction_scheduling",
        "--stress-turbo-late-spilling", "--target_os=mac", "--target_arch=arm64",
        "--embedded_src", str(work / "embedded.S"), "--embedded_variant", "Default",
        "--random-seed", "314159265", "--startup_src", str(work / "snapshot.cc"),
        "--no-native-code-counters",
    ], cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
config = configs["v8_snapshot"]
flags = sum((config[key] for key in
             ("defines", "include_dirs", "cflags", "cflags_cc")), [])
with (work / "snapshot.build.log").open("w") as log:
    for filename in ("snapshot.cc", "embedded.S"):
        subprocess.run([args.cxx, *flags, "-c", str(work / filename),
                        "-o", str(work / (Path(filename).stem + ".o"))],
                       cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT,
                       check=True)
candidate = work / "libwee8.a"
if archive == candidate:
    raise SystemExit("Output archive must differ from baseline")
shutil.copyfile(archive, candidate)
subprocess.run([args.ar, "r", str(candidate), str(work / "bootstrapper.o"),
                str(work / "snapshot.o"), str(work / "embedded.o")], check=True)
print("Prototype archive and matching snapshot generated.")
