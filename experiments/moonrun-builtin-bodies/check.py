#!/usr/bin/env python3
"""Compare builtins and exercise intentional traps in isolated POSIX processes."""
import argparse
import csv
import io
import json
from pathlib import Path
import resource
import signal
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--before', type=Path, required=True)
parser.add_argument('--after', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
reports = {}
for label, probe in [('before', args.before.resolve()), ('after', args.after.resolve())]:
    result = subprocess.run([str(probe), '--inspect'], capture_output=True,
                            text=True, check=True, timeout=60)
    rows = list(csv.reader(io.StringIO(result.stdout)))
    assert len(rows) == 66, f'{label}: incomplete explicit selection'
    reports[label] = {row[0]: list(map(int, row[1:])) for row in rows}
    assert len(reports[label]) == len(rows), f'{label}: duplicate builtin name'
    for name in reports[label]:
        result = subprocess.run([str(probe), '--invoke=' + name],
                                capture_output=True, text=True, timeout=60)
        assert 'invoking:' + name in result.stderr, (label, name, result.stderr)
        if label == 'before':
            assert result.returncode == 0, (label, name, result.returncode)
        else:
            assert result.returncode in (-signal.SIGTRAP, -signal.SIGILL), (
                label, name, result.returncode, result.stderr)
    for mode in ['--liftoff-only --no-wasm-lazy-compilation',
                 '--no-liftoff --no-wasm-lazy-compilation',
                 '--wasm-lazy-compilation',
                 '--wasm-sync-tier-up --wasm-tiering-budget=1']:
        subprocess.run([str(probe), mode], check=True, timeout=60)
        subprocess.run([str(probe), "--retained=" + mode], check=True, timeout=60)
    print(f'{label}: registrations, identities, 66 invocations and four Wasm modes passed')
assert reports['before'].keys() == reports['after'].keys()
for name, before in reports['before'].items():
    after = reports['after'][name]
    assert before[:3] == after[:3], (name, 'ID or argument metadata changed')
    assert 0 < after[3] < before[3], (name, 'body did not shrink')
args.output.write_text(json.dumps(reports, indent=2) + '\n')
print('All selected bodies shrank; builtin IDs and argument metadata match.')
