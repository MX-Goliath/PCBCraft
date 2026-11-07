"""Shared release metadata, checksums and third-party license collection."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], cwd=ROOT, check=True, **kwargs)


def cargo_metadata():
    return json.loads(run("cargo", "metadata", "--locked", "--format-version=1", capture_output=True, text=True).stdout)


def checksum(path):
    with path.open("rb") as handle:
        digest = hashlib.file_digest(handle, "sha256").hexdigest()
    path.with_name(path.name + ".sha256").write_text(f"{digest}  {path.name}\n", encoding="ascii")
    return digest


def notices(metadata, destination):
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "LICENSE", destination / "PCBCraft-MIT.txt")
    shutil.copytree(ROOT / "assets/fonts", destination / "fonts", dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns("*.ttf"))
    summaries = []
    for package in metadata["packages"]:
        if package["source"] is None:
            continue
        summaries.append({key: package[key] for key in ("name", "version", "license", "repository")})
        source = Path(package["manifest_path"]).parent
        candidates = [entry for entry in source.iterdir() if entry.is_file()
                      and entry.name.lower().startswith(("license", "licence", "copying", "notice", "copyright"))]
        if package["license_file"]:
            candidates.append(source / package["license_file"])
        for entry in set(candidates):
            target = destination / "crates" / f'{package["name"]}-{package["version"]}' / entry.name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(entry, target)
    (destination / "dependencies.json").write_text(json.dumps(summaries, indent=2), encoding="utf-8")
