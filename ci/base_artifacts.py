"""Restore a matching base library and generated headers from a prior producer."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import sys
import tempfile
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import base_cache
import native_artifacts as native


def restore(target):
    identity = base_cache.identity()
    runs = native.api("actions/workflows/build.yml/runs?status=completed&per_page=100")["workflow_runs"]
    for run in runs:
        if (run["event"] != "workflow_dispatch" or run["path"] != ".github/workflows/build.yml"
                or run["head_repository"]["full_name"] != os.environ["GITHUB_REPOSITORY"]):
            continue
        artifact = native.artifact_map(run["id"]).get(f"base-{target}-{identity}")
        if not artifact or base_cache.identity(revision=native.commit(run["head_sha"])) != identity:
            continue
        data = native.command("gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/artifacts/{artifact['id']}/zip")
        if "sha256:" + hashlib.sha256(data).hexdigest() != artifact["digest"]:
            raise ValueError("base artifact ZIP checksum mismatch")
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                manifest = json.loads(archive.read("base.json"))
                if manifest["runner_image"] != os.environ.get("ImageVersion", ""):
                    continue
                if any(item.filename != item.orig_filename for item in archive.infolist()):
                    raise ValueError("unexpected base artifact entry spelling")
                names = [item.filename for item in archive.infolist() if not item.is_dir()]
                if len(names) != len(set(names)):
                    raise ValueError("duplicate base artifact entry")
                for name in names:
                    path = PurePosixPath(name)
                    if (path.is_absolute() or ".." in path.parts or "\\" in name or ":" in name
                            or (name not in {"base.json", "libwee8_base.a", "wee8_base.lib"} and not name.startswith("gen/"))):
                        raise ValueError(f"unexpected base artifact entry: {name}")
                    destination = folder / name
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(archive.read(name))
            manifest = base_cache.verify(folder, target)
            if manifest["producer_revision"] != run["head_sha"] or manifest["producer_run"] != str(run["id"]):
                raise ValueError("base artifact producer differs")
            destination = base_cache.ROOT / "target/wee8-base" / target
            shutil.copytree(folder, destination)
            print(f"Reused base from run {run['id']}; extension and consumer validation will run anew", flush=True)
            return
    print("No matching base artifact; build the base from source", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    restore(parser.parse_args().target)
