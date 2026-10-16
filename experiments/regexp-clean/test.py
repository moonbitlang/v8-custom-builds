#!/usr/bin/env python3
"""Compare two complete native builds and exercise the registration boundary.

The supplied GN output directories must each have completed the wee8 target.
This harness uses the producer's unchanged C/Rust consumers, plus a probe that
checks absent JS registrations and explicit rejection at supported entry points.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('baseline', type=Path)
parser.add_argument('candidate', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--compiler', required=True)
parser.add_argument('--nm', required=True)
args = parser.parse_args()
repo = Path(__file__).resolve().parents[2]
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
results = {}
for name, build in [('baseline', args.baseline.resolve()), ('candidate', args.candidate.resolve())]:
    destination = out/name
    destination.mkdir(exist_ok=True)
    archive = build/'obj/v8/libwee8.a'
    header = build.parent/'rusty_v8/v8/third_party/wasm-api/wasm.h'
    assert hashlib.sha256(header.read_bytes()).hexdigest() == '7b6414f525d503a27609f0dfc3a33fa135f5cdf22d8b03f9e20a948ee8e62301'
    shutil.copy2(archive, destination/'libwee8.a')
    shutil.copy2(header, destination/'wasm.h')
    env = os.environ.copy()
    env.update(WEE8_DIR=str(destination), WEE8_ARCHIVE=str(destination/'libwee8.a'), WEE8_HEADER=str(destination/'wasm.h'))
    with (destination/'c-api-build.log').open('w') as log:
        subprocess.run(['cargo', 'rustc', '--release', '--locked', '--offline', '--manifest-path',
                        str(repo/'ci/c-api/Cargo.toml'), '--target-dir', str(out/'consumer-target'), '--',
                        '-C', f'link-arg=-Wl,-map,{destination / "consumer.map"}'],
                       env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = destination/'consumer'
    shutil.copy2(out/'consumer-target/release/wee8-smoke', binary)
    symbols = subprocess.check_output([args.nm, '--defined-only', '--demangle', str(binary)], text=True)
    (destination/'symbols.txt').write_text(symbols)
    shutil.copy2(binary, destination/'consumer.unstripped')
    subprocess.run(['strip', '-x', str(binary)], check=True)
    with (destination/'c-api-test.log').open('w') as log:
        smoke = subprocess.run([str(binary)], stdout=log, stderr=subprocess.STDOUT)
    with (destination/'rust-tests.log').open('w') as log:
        rust = subprocess.run(['cargo', 'test', '--locked', '--offline', '--manifest-path',
                              str(repo/'crates/wee8/Cargo.toml'), '--target-dir', str(out/'rust-target')],
                             env=env, stdout=log, stderr=subprocess.STDOUT)
    with (destination/'numeric-test.log').open('w') as log:
        numeric = subprocess.run(['cargo', 'run', '--release', '--locked', '--offline', '--manifest-path',
                                 str(Path(__file__).parent/'consumer/Cargo.toml'),
                                 '--target-dir', str(out/'numeric-target')],
                                env=env, stdout=log, stderr=subprocess.STDOUT)
    archive_data = archive.read_bytes()
    compressed = gzip.compress(archive_data, compresslevel=9, mtime=0)
    (destination/'libwee8.a.gz').write_bytes(compressed)
    forbidden = ['RegExpParser', 'RegExpCompiler', 'IrregexpInterpreter', 'RegExpMacroAssembler', 'ExperimentalRegExp::', 'ExperimentalRegExpInterpreter',
                 'Builtin_RegExp', 'Builtins_RegExp', 're_match_for_call_from_js']
    retained = {pattern: sum(pattern in line for line in symbols.splitlines()) for pattern in forbidden}
    archive_symbols = subprocess.check_output([args.nm, '--defined-only', '--demangle', str(archive)], text=True, stderr=subprocess.DEVNULL)
    archive_retained = {pattern: sum(pattern in line for line in archive_symbols.splitlines()) for pattern in forbidden}
    results[name] = {'archive_bytes': len(archive_data), 'gzip_bytes': len(compressed),
                     'stripped_consumer_bytes': binary.stat().st_size,
                     'archive_sha256': hashlib.sha256(archive_data).hexdigest(),
                     'consumer_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                     'c_api_exit': smoke.returncode, 'rust_tests_exit': rust.returncode,
                     'numeric_serialization_exit': numeric.returncode,
                     'retained_engine_or_library_builtin_symbols': retained,
                     'archive_engine_or_library_builtin_symbols': archive_retained}
    print(name, json.dumps(results[name]), flush=True)
    (out/'results.json').write_text(json.dumps(results, indent=2)+'\n')

# Reuse the candidate's exact native defines, ABI flags and generated includes.
build = args.candidate.resolve()
variables = {}
for line in (build/'obj/v8/wee8.ninja').read_text().splitlines():
    if line.startswith('build '):
        break
    if ' = ' in line:
        key, value = line.split(' = ', 1)
        variables[key] = shlex.split(value)
flags = sum((variables[k] for k in ['defines', 'include_dirs', 'cflags', 'cflags_cc']), [])
probe = out/'registration-probe'
with (out/'registration-build.log').open('w') as log:
    subprocess.run([args.compiler, *flags, str(Path(__file__).with_name('registration-probe.cc').resolve()),
                    str(build/'obj/v8/libwee8.a'), '-pthread', '-ldl', '-Wl,-dead_strip', '-o', str(probe)],
                   cwd=build, stdout=log, stderr=subprocess.STDOUT, check=True)
with (out/'registration-test.log').open('w') as log:
    registered = subprocess.run([str(probe)], stdout=log, stderr=subprocess.STDOUT)
results['registration_probe_exit'] = registered.returncode
(out/'results.json').write_text(json.dumps(results, indent=2)+'\n')
assert registered.returncode == 0
assert results['baseline']['archive_engine_or_library_builtin_symbols']['RegExpCompiler'] > 0
assert all(v == 0 for v in results['candidate']['archive_engine_or_library_builtin_symbols'].values())
assert all(v == 0 for v in results['candidate']['retained_engine_or_library_builtin_symbols'].values())
assert all(results[name]['c_api_exit'] == results[name]['rust_tests_exit'] == results[name]['numeric_serialization_exit'] == 0 for name in ['baseline', 'candidate'])
