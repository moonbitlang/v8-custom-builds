#!/usr/bin/env python3
"""Run the existing C API smoke body with explicit V8 compilation modes."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('measurement', type=Path)
parser.add_argument('source', type=Path)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[2]
out = args.measurement.resolve()
source = args.source.resolve()
project = out/'tier-driver'
project.mkdir(exist_ok=True)
for name in ['Cargo.toml', 'Cargo.lock', 'probe.cc']:
    shutil.copy2(repo/'ci/c-api'/name, project/name)
# Keep all assertions and fixtures from the maintained consumer. The only
# addition configures V8 before its engine is initialized in the child process.
rust = (repo/'ci/c-api/probe.rs').read_text()
anchor = '    let bytes = wat::parse_str('
assert rust.count(anchor) == 1
rust = 'unsafe extern "C" { fn wee8_set_flags(flags: *const std::ffi::c_char); }\n' + rust
rust = rust.replace(anchor, '''    let flags = std::ffi::CString::new(std::env::var("WEE8_TEST_FLAGS").unwrap()).unwrap();
    unsafe { wee8_set_flags(flags.as_ptr()); }
''' + anchor)
(project/'probe.rs').write_text(rust)
(project/'flags.cc').write_text('''#include "v8-initialization.h"
extern "C" void wee8_set_flags(const char* flags) {
  v8::V8::SetFlagsFromString(flags);
}
''')
build = (repo/'ci/c-api/build.rs').read_text()
build = build.replace('.file("probe.cc")', '.file("probe.cc").file("flags.cc").include(std::env::var_os("V8_INCLUDE").unwrap())')
(project/'build.rs').write_text(build)
modes = {
    'liftoff': '--no-wasm-lazy-compilation --no-wasm-tier-up',
    'optimizing': '--no-liftoff --no-wasm-lazy-compilation',
    'lazy': '--wasm-lazy-compilation',
    'tier-up': '--wasm-sync-tier-up --wasm-tiering-budget=1',
}
results = {}
for variant in ['baseline', 'candidate']:
    env = os.environ.copy()
    env.update(WEE8_DIR=str(out/variant), V8_INCLUDE=str(source/'v8/include'))
    with (out/variant/'tier-build.log').open('w') as log:
        subprocess.run(['cargo', 'build', '--release', '--locked', '--offline', '--manifest-path',
                        str(project/'Cargo.toml'), '--target-dir', str(out/'tier-target')],
                       env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    results[variant] = {}
    for mode, flags in modes.items():
        env['WEE8_TEST_FLAGS'] = flags
        with (out/variant/f'tier-{mode}.log').open('w') as log:
            test = subprocess.run([str(out/'tier-target/release/wee8-smoke')], env=env,
                                  stdout=log, stderr=subprocess.STDOUT)
        results[variant][mode] = {'flags': flags, 'exit': test.returncode}
        print(variant, mode, test.returncode, flush=True)
    (out/'tier-results.json').write_text(json.dumps(results, indent=2)+'\n')
assert all(result['exit'] == 0 for variant in results.values() for result in variant.values())
