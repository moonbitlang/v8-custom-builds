#!/usr/bin/env python3
"""Validate the optional body cut after a Linux custom wee8 source build."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parents[2]
os.chdir(root)
experiment = root / 'experiments/moonrun-builtin-bodies'
target = os.environ['TARGET']
assert target == 'x86_64-unknown-linux-gnu', 'Use the targeted Linux experiment job'
out = root / 'build' / target / 'release/gn_out'
tools = out.parent / 'ninja_gn_binaries'
gn = tools / 'gn/gn'
ninja = tools / 'ninja/ninja'
source = root / 'rusty_v8'
reports = root / 'target/builtin-experiment'
reports.mkdir(parents=True, exist_ok=True)
archive = out / 'custom_wee8/obj/v8/libwee8.a'
control_hash = hashlib.sha256(archive.read_bytes()).hexdigest()
subprocess.run(['git', '-C', str(source), 'apply',
                str(experiment / 'omit-bodies.patch')], check=True)
shutil.copyfile(experiment / 'probe.cc', source / 'moonrun-builtin-check.cc')
args_file = out / 'args.gn'
original_args = args_file.read_text()
for label, enabled in [('before', 'false'), ('after', 'true')]:
    args_file.write_text(original_args +
                        '\nv8_moonrun_omit_library_builtin_bodies = ' + enabled + '\n')
    subprocess.run([str(gn), 'gen', str(out)], cwd=source, check=True)
    subprocess.run([str(ninja), '-C', str(out), 'moonrun-builtin-experiment',
                    '-j', os.environ.get('CARGO_BUILD_JOBS', '3')], check=True)
    folder = reports / label
    folder.mkdir(exist_ok=True)
    for relative, name in [
        ('obj/v8/libwee8.a', 'libwee8.a'),
        ('moonrun-builtin-check', 'probe'),
        ('gen/v8/embedded.S', 'embedded.S'),
        ('gen/v8/snapshot.cc', 'snapshot.cc'),
    ]:
        shutil.copyfile(out / 'custom_wee8' / relative, folder / name)
    (folder / 'probe').chmod(0o755)
    shutil.copyfile(source / 'v8/third_party/wasm-api/wasm.h', folder / 'wasm.h')
    if label == 'before':
        assert hashlib.sha256(archive.read_bytes()).hexdigest() == control_hash, \
            'Disabled experiment changed the production archive'
    env = dict(os.environ, WEE8_DIR=str(folder),
               WEE8_ARCHIVE=str(folder / 'libwee8.a'),
               WEE8_HEADER=str(folder / 'wasm.h'))
    subprocess.run(['cargo', 'run', '--release', '--locked', '--manifest-path',
                    'ci/c-api/Cargo.toml', '--target-dir', 'target/c-api'],
                   env=env, check=True)
    shutil.copyfile(root / 'target/c-api/release/wee8-smoke',
                    folder / 'consumer-stripped')
    subprocess.run(['strip', str(folder / 'consumer-stripped')], check=True)
    if label == 'after':
        subprocess.run(['cargo', 'test', '--locked', '--manifest-path',
                        'crates/wee8/Cargo.toml', '--target-dir', 'target/wee8'],
                       env=env, check=True)
subprocess.run(['python3', str(experiment / 'check.py'),
                '--before', str(reports / 'before/probe'),
                '--after', str(reports / 'after/probe'),
                '--output', str(reports / 'identities.json')], check=True)
subprocess.run(['python3', str(experiment / 'measure.py'),
                '--before', str(reports / 'before'),
                '--after', str(reports / 'after'),
                '--identities', str(reports / 'identities.json'),
                '--output', str(reports / 'sizes.json')], check=True)
(reports / 'provenance.json').write_text(json.dumps({
    'head': os.environ['GITHUB_SHA'], 'run': os.environ['GITHUB_RUN_ID'],
    'target': target, 'production_enabled': False,
}, indent=2) + '\n')
