#!/usr/bin/env python3
"""Show the maintained adapter's differences from its exact upstream baseline."""

import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    root = Path(__file__).resolve().parents[1]
    origins = json.loads((root / "custom_wee8/upstream.json").read_text())
    for local, origin in origins.items():
        current = (root / "custom_wee8" / local).read_bytes()
        if origin.get("unmodified"):
            if hashlib.sha256(current).hexdigest() != origin["sha256"]:
                raise SystemExit(f"unmodified upstream header changed: {local}")
            continue
        original = subprocess.check_output([
            "git", "-C", str(root / "rusty_v8/v8"), "show",
            f"{origin['revision']}:{origin['path']}",
        ])
        if hashlib.sha256(original).hexdigest() != origin["sha256"]:
            raise SystemExit(f"upstream source digest mismatch: {local}")
        sys.stdout.writelines(difflib.unified_diff(
            original.decode().splitlines(keepends=True),
            current.decode().splitlines(keepends=True),
            fromfile=f"upstream/{origin['revision']}/{origin['path']}",
            tofile=f"custom_wee8/{local}",
        ))


if __name__ == "__main__":
    main()
