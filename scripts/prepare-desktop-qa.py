#!/usr/bin/env python3
"""Prepare isolated native UI fixtures for updates and Git Pull (debug Windows)."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def git(path, *args):
    return subprocess.run(["git", "-C", str(path), *map(str, args)], check=True,
                          capture_output=True, text=True).stdout.strip()


def main():
    root = ROOT / "target" / f"desktop-qa-{uuid.uuid4().hex[:8]}"
    app = root / "app"
    controls = app / "UpdateQA"
    controls.mkdir(parents=True)
    binary = ROOT / "target/debug/pcbcraft.exe"
    shutil.copy2(binary, app / "pcbcraft.exe")
    (app / "portable.flag").write_text("Keep this QA portable marker.\n", encoding="utf-8")
    (app / "update-qa.flag").write_text("Explicit debug-only local release fixtures.\n", encoding="utf-8")
    (app / "Data").mkdir()
    (app / "Data/sentinel.txt").write_text("Keep these user data.\n", encoding="utf-8")
    (app / "obsolete.dll").write_bytes(b"obsolete application file fixture")
    (app / "build-info.json").write_text(json.dumps({"version": "0.0.2", "dependencies": {"pcbcraft.exe": [], "obsolete.dll": []}}), encoding="utf-8")
    (controls / "check-delay-ms").write_text("2000", encoding="ascii")
    (controls / "chunk-ms").write_text("60", encoding="ascii")
    name = "PCBCraft-999.0.0-windows-x64-portable.zip"
    package = controls / name
    with zipfile.ZipFile(package, "w", zipfile.ZIP_DEFLATED, compresslevel=1) as archive:
        archive.write(binary, "release/pcbcraft.exe")
        archive.writestr("release/build-info.json", json.dumps({"version": "999.0.0", "dependencies": {"pcbcraft.exe": []}}))
        archive.writestr("release/new-runtime.txt", "New shipped payload.\n")
        archive.writestr("release/licenses/current.txt", "QA package, not a published release.\n")
        archive.writestr("release/Data/sentinel.txt", "Must not overwrite user data.\n")
        archive.writestr("release/portable.flag", "Must not overwrite marker.\n")
    digest = hashlib.sha256(package.read_bytes()).hexdigest()
    release = {"tag_name": "v999.0.0", "draft": False, "prerelease": False, "assets": [{
        "name": name, "size": package.stat().st_size, "digest": f"sha256:{digest}",
        "browser_download_url": f"https://github.com/MX-Goliath/PCBCraft/releases/download/v999.0.0/{name}",
    }]}
    (controls / "release-new.json").write_text(json.dumps(release), encoding="utf-8")
    (controls / "release-current.json").write_text(json.dumps({"tag_name": "v0.0.1", "draft": False, "prerelease": False, "assets": []}), encoding="utf-8")

    seeds = root / "seeds"
    subprocess.run([str(ROOT / "target/debug/examples/library_update_probe.exe"), str(seeds)], check=True)
    origin = root / "origin.git"
    origin.mkdir()
    git(origin, "init", "--bare", "-b", "main")
    working = root / "working"
    working.mkdir()
    git(working, "init", "-b", "main")
    git(working, "config", "user.name", "PCBCraft UI QA")
    git(working, "config", "user.email", "qa@example.invalid")
    shutil.copy2(seeds / "Sheet.SchDoc", working / "Main.SchDoc")
    shutil.copy2(seeds / "Sheet.SchDoc", working / "Extra.SchDoc")
    shutil.copy2(seeds / "PartsBaseline.SchLib", working / "Parts.SchLib")
    (working / "Demo.PrjPcb").write_text("[Design]\nVersion=1.0\nName=GitPullQA\n[Document1]\nDocumentPath=Main.SchDoc\n[Document2]\nDocumentPath=Parts.SchLib\n[Document3]\nDocumentPath=Extra.SchDoc\n", encoding="utf-8")
    (working / "README.txt").write_text("Baseline outside the Altium file filter.\n", encoding="utf-8")
    git(working, "add", ".")
    git(working, "commit", "-m", "Initial UI fixture")
    git(working, "remote", "add", "origin", origin)
    git(working, "push", "-u", "origin", "main")
    peer = root / "peer"
    git(root, "clone", "--branch", "main", origin, peer)
    git(peer, "config", "user.name", "PCBCraft UI QA")
    git(peer, "config", "user.email", "qa@example.invalid")
    state = {"root": str(root), "app": str(app / "pcbcraft.exe"), "controls": str(controls),
             "working": str(working), "peer": str(peer), "origin": str(origin), "seeds": str(seeds),
             "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
             "package_sha256": digest, "baseline_head": git(working, "rev-parse", "HEAD")}
    (root / "qa-state.json").write_text(json.dumps(state, indent=2), encoding="utf-8")
    print(json.dumps(state))


if __name__ == "__main__":
    main()
