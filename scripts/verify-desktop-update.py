#!/usr/bin/env python3
"""Exercise the real update worker on disposable portable copies, without a GUI.

Usage: python scripts/verify-desktop-update.py target/debug/pcbcraft.exe
No release is published and no installed application is changed.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import zipfile


def fixture(root, *, corrupt=False, blocked=False):
    target = root / "Portable ' Юникод"
    target.mkdir()
    (target / "pcbcraft.exe").write_bytes(b"old executable fixture")
    (target / "portable.flag").write_bytes(b"keep marker")
    (target / "Data").mkdir()
    (target / "Data/app.ron").write_bytes(b"keep settings")
    (target / "old.dll").write_bytes(b"obsolete DLL")
    (target / "build-info.json").write_text(json.dumps({"dependencies": {"pcbcraft.exe": [], "old.dll": []}}))
    package = root / "release.zip"
    with zipfile.ZipFile(package, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("release/pcbcraft.exe", b"new executable fixture")
        archive.writestr("release/runtime.dll", b"new DLL")
        archive.writestr("release/licenses/current.txt", b"new notices")
        archive.writestr("release/Data/app.ron", b"must not replace")
        if blocked:
            archive.writestr("release/z-blocked.dll", b"blocked")
            (target / "z-blocked.dll").mkdir()
    cache = root / "cache"
    cache.mkdir()
    pending = {
        "package": {
            "version": "999.0.0", "distribution": "WindowsPortable",
            "asset": {"name": package.name, "browser_download_url": "https://github.com/fixture/fixture", "size": package.stat().st_size, "digest": None},
            "checksum": None, "release_url": "https://github.com/fixture/fixture",
        },
        "path": str(package), "digest": hashlib.sha256(package.read_bytes()).hexdigest(),
        "executable": str(target / "pcbcraft.exe"),
    }
    (cache / "pending.json").write_text(json.dumps(pending), encoding="utf-8")
    if corrupt:
        package.write_bytes(b"tampered")
    return target, cache


def main():
    binary = Path(sys.argv[1]).resolve(strict=True)
    for scenario in ("success", "tampered", "rollback"):
        with tempfile.TemporaryDirectory(prefix="pcbcraft-worker-qa-") as temporary:
            target, cache = fixture(Path(temporary), corrupt=scenario == "tampered", blocked=scenario == "rollback")
            # This child models the still-running GUI; the worker must wait.
            parent = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(2)"])
            worker = subprocess.Popen([str(binary), "--pcbcraft-apply-update", str(cache), str(parent.pid), "exit"],
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            time.sleep(0.5)
            assert parent.poll() is None
            assert worker.poll() is None, "worker exited before parent"
            assert (target / "pcbcraft.exe").read_bytes() == b"old executable fixture"
            parent.wait(timeout=10)
            output, errors = worker.communicate(timeout=30)
            assert worker.returncode == 0, (output, errors)
            assert (target / "Data/app.ron").read_bytes() == b"keep settings"
            assert (target / "portable.flag").read_bytes() == b"keep marker"
            if scenario == "success":
                assert (target / "pcbcraft.exe").read_bytes() == b"new executable fixture"
                assert (target / "runtime.dll").read_bytes() == b"new DLL"
                assert (target / "licenses/current.txt").is_file()
                assert not (target / "old.dll").exists()
                assert not (cache / "pending.json").exists()
                assert not (cache / "error.txt").exists()
            else:
                assert (target / "pcbcraft.exe").read_bytes() == b"old executable fixture"
                assert (target / "old.dll").read_bytes() == b"obsolete DLL"
                assert (cache / "pending.json").exists()
                assert (cache / "error.txt").is_file()
            print(f"PASS: {scenario}; parent wait, file replacement and user-data preservation")

    # A tiny real executable records the relaunch context without opening a GUI.
    with tempfile.TemporaryDirectory(prefix="pcbcraft-restart-qa-") as temporary:
        root = Path(temporary)
        target, cache = fixture(root)
        source = root / "observer.rs"
        observer = root / "observer.exe"
        source.write_text('''fn main() {
    let args: Vec<_> = std::env::args().collect();
    let text = format!("{}\\n{}\\n{}", std::env::current_dir().unwrap().display(), args[2],
        std::env::var_os("PCBCRAFT_UPDATE_WORKING_DIR").is_none());
    std::fs::write(&args[1], text).unwrap();
}''', encoding="utf-8")
        subprocess.run(["rustc", "--edition=2024", str(source), "-o", str(observer)], check=True)
        package = root / "release.zip"
        with zipfile.ZipFile(package, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.write(observer, "release/pcbcraft.exe")
        pending = json.loads((cache / "pending.json").read_text(encoding="utf-8"))
        pending["digest"] = hashlib.sha256(package.read_bytes()).hexdigest()
        pending["package"]["asset"]["size"] = package.stat().st_size
        (cache / "pending.json").write_text(json.dumps(pending), encoding="utf-8")
        working = root / "working directory ' Юникод"
        working.mkdir()
        relative = "relative project ' Юникод.PcbDoc"
        log = root / "launch.txt"
        parent = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(2)"])
        worker = subprocess.Popen([str(binary), "--pcbcraft-apply-update", str(cache), str(parent.pid), "restart", str(log), relative],
                                  env=dict(os.environ, PCBCRAFT_UPDATE_WORKING_DIR=str(working)),
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        parent.wait(timeout=10)
        output, errors = worker.communicate(timeout=30)
        assert worker.returncode == 0, (output, errors)
        expected = f"{working}\n{relative}\ntrue"
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if log.exists() and log.read_text(encoding="utf-8") == expected:
                break
            time.sleep(0.05)
        else:
            raise AssertionError(f"Relaunch context was not preserved: {output!r}, {errors!r}")
        assert not (cache / "pending.json").exists()
        assert not (cache / "error.txt").exists()
        assert (target / "pcbcraft.exe").read_bytes() == observer.read_bytes()
        time.sleep(0.2)  # Allow the observer's process-exit handles to close on Windows.
        print("PASS: restart launches replaced executable; cwd, Unicode arguments and environment preserved")


if __name__ == "__main__":
    main()
