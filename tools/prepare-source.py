#!/usr/bin/env python3
"""Apply the ordered upstream patches and stage owned sources, without compiling."""

import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def main():
    root = Path(__file__).resolve().parents[1]
    source = root / "rusty_v8"
    repositories = {
        "rusty_v8": source,
        "chromium-build": source / "build",
        "v8": source / "v8",
    }
    series = [
        line.strip()
        for line in (root / "patches/series").read_text().splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    patches = sorted(
        str(p.relative_to(root / "patches"))
        for p in (root / "patches").rglob("*.patch")
    )
    if sorted(series) != patches or len(series) != len(set(series)):
        raise SystemExit("patches/series must list every patch exactly once")
    for entry in series:
        if len(Path(entry).parts) != 2 or Path(entry).parts[0] not in repositories:
            raise SystemExit(f"unknown patch dependency: {entry}")
        text = (root / "patches" / entry).read_text()
        if not text.startswith("From ") or any(
            f"\n{field}: " not in text
            for field in ("Subject", "Upstream-Status", "Maintenance", "Tests")
        ):
            raise SystemExit(f"missing Git patch metadata or maintenance contract: {entry}")

    # Preparation never resets a developer's edited dependency checkout. Inspect
    # the complete applied source with git diff; use a fresh checkout to reapply.
    for name, repo in repositories.items():
        parent, ref = (
            (root, ":rusty_v8")
            if name == "rusty_v8"
            else (source, f"HEAD:{repo.name}")
        )
        expected = subprocess.check_output(
            ["git", "-C", str(parent), "rev-parse", ref], text=True
        ).strip()
        actual = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True
        ).strip()
        if actual != expected:
            raise SystemExit(f"{name} is not at its pinned revision; initialize its submodule first")
        if subprocess.run(["git", "-C", str(repo), "diff", "--quiet", "HEAD", "--"]).returncode:
            raise SystemExit(f"{name} has changes; preserve them and use a fresh source checkout")
    if (source / "custom_wee8").exists():
        raise SystemExit("owned sources already exist; use a fresh source checkout")

    origins = json.loads((root / "custom_wee8/upstream.json").read_text())
    for local, origin in origins.items():
        if origin.get("unmodified"):
            data = (root / "custom_wee8" / local).read_bytes()
        else:
            actual = subprocess.check_output(
                ["git", "-C", str(source / "v8"), "rev-parse", "HEAD"], text=True
            ).strip()
            if actual != origin["revision"]:
                raise SystemExit("V8 changed: reconcile the owned adapter and update its provenance")
            data = (source / "v8" / origin["path"]).read_bytes()
        if hashlib.sha256(data).hexdigest() != origin["sha256"]:
            raise SystemExit(f"upstream source digest mismatch: {local}")

    for entry in series:
        repo = repositories[Path(entry).parts[0]]
        print(f"Applying {entry}", flush=True)
        subprocess.run(
            ["git", "-C", str(repo), "apply", str(root / "patches" / entry)],
            check=True,
        )
    shutil.copytree(root / "custom_wee8", source / "custom_wee8")
    print("Prepared sources without compiling. Review upstream edits with git diff in each dependency.")


if __name__ == "__main__":
    main()
