#!/usr/bin/env python3
"""Build a native Linux AppImage with the installed appimagetool. Python >= 3.11."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
# These are loaded with dlopen and therefore do not all appear in ldd output.
WINDOW_LIBS = (
    "libX11.so.6", "libX11-xcb.so.1", "libxcb.so.1", "libXi.so.6",
    "libXcursor.so.1", "libXrandr.so.2", "libXinerama.so.1", "libXrender.so.1",
    "libXfixes.so.3", "libxkbcommon.so.0", "libxkbcommon-x11.so.0",
    "libwayland-client.so.0", "libwayland-cursor.so.0", "libwayland-egl.so.1",
)
# Keep the host's C runtime and GPU loader/driver stack together.
HOST_LIB = re.compile(
    r"^(ld-linux.*|lib(c|m|dl|pthread|rt|resolv|util|anl)\.so\..*|"
    r"lib(GL.*|EGL|OpenGL|GLX.*|vulkan|drm.*|gbm)\.so\..*)$"
)


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def bundle_libraries(binary, destination):
    cache = run("ldconfig", "-p", capture_output=True).stdout
    libraries = {}
    for line in cache.splitlines():
        match = re.match(r"\s*(\S+) \(([^)]+)\) => (\S+)", line)
        if match and "x86-64" in match[2]:
            libraries[match[1]] = Path(match[3])
    missing = set(WINDOW_LIBS) - libraries.keys()
    if missing:
        raise RuntimeError(f"Install windowing libraries: {', '.join(sorted(missing))}")
    queue = [binary, *(libraries[name] for name in WINDOW_LIBS)]
    copied = {}
    while queue:
        current = queue.pop()
        dependencies = run("ldd", str(current), capture_output=True).stdout
        if "not found" in dependencies:
            raise RuntimeError(f"Unresolved dependencies for {current}:\n{dependencies}")
        for name, path in re.findall(r"\s*(\S+) => (/\S+)", dependencies):
            name = Path(name).name
            if not HOST_LIB.match(name) and name not in copied:
                copied[name] = Path(path)
                queue.append(Path(path))
        if current != binary and not HOST_LIB.match(current.name):
            copied.setdefault(current.name, current)
    for name, source in sorted(copied.items()):
        shutil.copy2(source, destination / name)
    return copied


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true", help="Package the existing release binary")
    parser.add_argument("--appimagetool", default=os.environ.get("APPIMAGETOOL", "appimagetool"))
    parser.add_argument("--runtime-file", type=Path, help="Use an already downloaded AppImage runtime")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("This packaging script currently supports native Linux x86_64 only")
    tool = shutil.which(args.appimagetool)
    if not tool:
        parser.error("Install appimagetool or specify --appimagetool /path/to/appimagetool")
    if not args.no_build:
        run("bash", str(ROOT / "scripts/dev.sh"), "build", "--locked", "--release", "-p", "eda-app")
    metadata = run("bash", str(ROOT / "scripts/dev.sh"), "metadata", "--no-deps", "--format-version=1", capture_output=True)
    binary = Path(json.loads(metadata.stdout)["target_directory"]) / "release/pcbcraft"
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    output_dir = ROOT / "dist"
    output_dir.mkdir(exist_ok=True)
    output = output_dir / f"PCBCraft-{version}-x86_64.AppImage"
    staging = ROOT / "target/appimage"
    staging.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="build-", dir=staging) as temporary:
        appdir = Path(temporary) / "PCBCraft.AppDir"
        bindir = appdir / "usr/bin"
        libdir = appdir / "usr/lib"
        licenses = appdir / "usr/share/licenses/pcbcraft"
        for directory in (bindir, libdir, licenses):
            directory.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, bindir / "pcbcraft")
        run("strip", "--strip-unneeded", str(bindir / "pcbcraft"))
        bundled = bundle_libraries(bindir / "pcbcraft", libdir)
        for filename in ("pcbcraft.desktop", "pcbcraft.svg"):
            shutil.copy2(ROOT / "assets/linux" / filename, appdir / filename)
        (appdir / ".DirIcon").symlink_to("pcbcraft.svg")
        launcher = appdir / "AppRun"
        launcher.write_text(
            '#!/bin/sh\nset -eu\n'
            'appdir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"\n'
            'export LD_LIBRARY_PATH="$appdir/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\n'
            'exec "$appdir/usr/bin/pcbcraft" "$@"\n'
        )
        launcher.chmod(0o755)
        shutil.copy2(ROOT / "LICENSE", licenses / "LICENSE")
        shutil.copytree(ROOT / "assets/fonts", licenses / "fonts", ignore=shutil.ignore_patterns("*.ttf"))
        # Arch supplies package-specific third-party notices here. Also include
        # Debian copyright files when building on Debian/Ubuntu.
        for source in bundled.values():
            if shutil.which("pacman"):
                package = run("pacman", "-Qqo", str(source.resolve()), capture_output=True).stdout.strip()
                notices = Path("/usr/share/licenses") / package
                if notices.is_dir():
                    shutil.copytree(notices, licenses / "system" / package, dirs_exist_ok=True)
            elif shutil.which("dpkg-query"):
                owner = run("dpkg-query", "-S", str(source), capture_output=True).stdout.split(": ")[0]
                package = owner.split(":")[0]
                notice = Path("/usr/share/doc") / package / "copyright"
                if notice.is_file():
                    target = licenses / "system" / package
                    target.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(notice, target / "copyright")
        (licenses / "bundled-libraries.txt").write_text("\n".join(sorted(bundled)) + "\n")
        command = [tool]
        # Official appimagetool binaries are themselves AppImages. Extraction
        # mode also works on builders without a mounted /dev/fuse.
        with open(tool, "rb") as handle:
            handle.seek(8)
            if handle.read(3) == b"AI\x02":
                command.append("--appimage-extract-and-run")
        command += ["--no-appstream", "--comp", "zstd"]
        if args.runtime_file:
            command += ["--runtime-file", str(args.runtime_file.resolve())]
        # The previous image may still be running. Build on the destination
        # filesystem, then replace its directory entry without overwriting the
        # executable inode or exposing a partially packaged file.
        with tempfile.TemporaryDirectory(prefix=".build-", dir=output_dir) as package_dir:
            packaged = Path(package_dir) / output.name
            command += [str(appdir), str(packaged)]
            run(*command, env={**os.environ, "ARCH": "x86_64", "VERSION": version})
            packaged.chmod(0o755)
            with packaged.open("rb") as handle:
                digest = hashlib.file_digest(handle, "sha256").hexdigest()
            packaged.replace(output)
    output.with_suffix(".AppImage.sha256").write_text(f"{digest}  {output.name}\n")
    print(f"Built {output}\nSHA256 {digest}")


if __name__ == "__main__":
    main()
