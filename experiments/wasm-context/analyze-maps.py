#!/usr/bin/env python3
"""Summarize live Mach-O link-map symbols without recording machine paths."""
import argparse
import collections
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("checks", type=Path)
args = parser.parse_args()
reports = {}
for variant in ("baseline", "candidate"):
    reports[variant] = {}
    for name in ("initialize", "c-api"):
        objects = {}
        sizes = collections.Counter()
        live = False
        for line in (args.checks / variant / (name + ".map")).read_text().splitlines():
            match = re.match(r"\[\s*(\d+)\]\s+(.+)", line)
            if match:
                path = match[2]
                objects[int(match[1])] = (
                    path.rsplit("(", 1)[-1].rstrip(")") if ".a(" in path
                    else Path(path).name
                )
            if line == "# Symbols:":
                live = True
            if line.startswith("# Dead Stripped Symbols:"):
                live = False
            match = re.match(
                r"0x[0-9A-Fa-f]+\s+0x([0-9A-Fa-f]+)\s+\[\s*(\d+)\]", line)
            if live and match:
                sizes[objects[int(match[2])]] += int(match[1], 16)
        reports[variant][name] = {
            "live_symbol_bytes": sum(sizes.values()),
            "live_objects": len(sizes),
            "top_objects": dict(sizes.most_common(15)),
            "selected_objects": {
                key: value for key, value in sizes.items()
                if any(part in key for part in (
                    "temporal", "c-api", "bootstrapper", "external-reference-table",
                    "embedded", "snapshot", "builtins-date", "wasm-js"))
            },
        }
print(json.dumps(reports, indent=2))
