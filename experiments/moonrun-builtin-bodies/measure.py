#!/usr/bin/env python3
"""Measure matching native artifacts; never count generated source text as code."""
import argparse
import hashlib
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--before', type=Path, required=True)
parser.add_argument('--after', type=Path, required=True)
parser.add_argument('--identities', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
identities = json.loads(args.identities.read_text())
reports = {}
body_sizes = {}
hashes = {}
for label, folder in [('before', args.before), ('after', args.after)]:
    assembly = (folder / 'embedded.S').read_text()
    snapshot = (folder / 'snapshot.cc').read_text()
    reports[label] = {
        'archive': (folder / 'libwee8.a').stat().st_size,
        'stripped_consumer': (folder / 'consumer-stripped').stat().st_size,
        'embedded_code': int(re.search(
            r'_?v8_Default_embedded_blob_code_size_:\s+\.long (\d+)', assembly)[1]),
        'embedded_data': int(re.search(
            r'_?v8_Default_embedded_blob_data_size_:\s+\.long (\d+)', assembly)[1]),
        'snapshot': int(re.search(r'static const int blob_size = (\d+)', snapshot)[1]),
    }
    hashes[label] = hashlib.sha256((folder / 'libwee8.a').read_bytes()).hexdigest()
    sizes = {}
    name = None
    for line in assembly.splitlines():
        match = re.match(r'_?Builtins_(\w+):', line)
        if match:
            name = match[1]
            sizes[name] = 0
        elif line and not line[0].isspace() and line.endswith(':'):
            name = None
        elif name:
            match = re.match(r'\s*\.(octa|quad|xword|long|word|byte)\s+(.+)', line)
            if match:
                width = {'octa': 16, 'quad': 8, 'xword': 8,
                         'long': 4, 'word': 4, 'byte': 1}[match[1]]
                sizes[name] += width * len(match[2].split(','))
    body_sizes[label] = sizes
assert body_sizes['before'].keys() == body_sizes['after'].keys(), 'Builtin labels changed'
changed = {name for name, size in body_sizes['before'].items()
           if size != body_sizes['after'][name]}
assert changed == identities['before'].keys(), 'Unexpected body-size changes'
result = {
    'sizes': {name: {
        'before': before,
        'after': reports['after'][name],
        'saved_bytes': before - reports['after'][name],
        'saved_percent': round((before - reports['after'][name]) / before * 100, 3),
    } for name, before in reports['before'].items()},
    'archive_sha256': hashes,
    'total_builtin_labels': len(body_sizes['before']),
    'unselected_builtin_body_lengths_unchanged': True,
}
args.output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result['sizes'], indent=2))
