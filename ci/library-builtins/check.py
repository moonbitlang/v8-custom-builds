#!/usr/bin/env python3
"""Check unsupported JS entries and retained Wasm behavior in child processes."""
import argparse
import csv
import io
import json
import os
from pathlib import Path
import signal
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--before', type=Path, help='Optional matching control probe')
parser.add_argument('--before-promise-cut', action='store_true',
                    help='The control already omits public library entries')
parser.add_argument('--after', type=Path, required=True)
parser.add_argument('--output', type=Path, help='Optional identity/size report')
args = parser.parse_args()
if os.name == 'nt':
    import ctypes
    # Intentional traps must not open Windows Error Reporting dialogs.
    ctypes.windll.kernel32.SetErrorMode(0x0002 | 0x8000)
    trap_statuses = {0x80000003, -2147483645, 0xC000001D, -1073741795}
else:
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    trap_statuses = {-signal.SIGTRAP, -signal.SIGILL}
# Private closures require their combinator contexts in an untrimmed build.
# Inspect control identities, but call only the candidate's explicit traps.
closures = {'PromiseAllResolveElementClosure',
            'PromiseAllSettledResolveElementClosure',
            'PromiseAllSettledRejectElementClosure',
            'PromiseAnyRejectElementClosure'}
reports = {}
probes = [('before', args.before)] if args.before else []
probes.append(('after', args.after))
for label, path in probes:
    probe = path.resolve()
    result = subprocess.run([str(probe), '--inspect'], capture_output=True,
                            text=True, check=True, timeout=60)
    rows = list(csv.reader(io.StringIO(result.stdout)))
    assert len(rows) == 153, f'{label}: incomplete explicit selection'
    reports[label] = {row[0]: list(map(int, row[1:])) for row in rows}
    assert len(reports[label]) == len(rows), f'{label}: duplicate builtin name'
    assert closures <= reports[label].keys()
    for name in reports[label]:
        if label == 'before' and name in closures:
            continue
        result = subprocess.run([str(probe), '--invoke=' + name],
                                capture_output=True, text=True, timeout=60)
        assert 'invoking:' + name in result.stderr, (label, name, result.stderr)
        if label == 'before' and not args.before_promise_cut:
            assert result.returncode == 0, (label, name, result.returncode)
        else:
            assert result.returncode in trap_statuses, (
                label, name, result.returncode, result.stderr)
    for mode in ['--liftoff-only --no-wasm-lazy-compilation',
                 '--no-liftoff --no-wasm-lazy-compilation',
                 '--wasm-lazy-compilation',
                 '--wasm-sync-tier-up --wasm-tiering-budget=1']:
        subprocess.run([str(probe), mode], check=True, timeout=60)
        subprocess.run([str(probe), '--retained=' + mode], check=True, timeout=60)
    print(f'{label}: 153 identities; public entries and candidate closure invocations; four Wasm modes passed')
if args.before:
    assert reports['before'].keys() == reports['after'].keys()
    for name, before in reports['before'].items():
        after = reports['after'][name]
        assert before[:3] == after[:3], (name, 'ID or argument metadata changed')
        if args.before_promise_cut and name not in closures:
            assert before[3] == after[3], (name, 'existing body changed')
        else:
            assert 0 < after[3] < before[3], (name, 'body did not shrink')
    print('Selected new bodies shrank; builtin IDs and argument metadata match.')
if args.output:
    args.output.write_text(json.dumps(reports, indent=2) + '\n')
