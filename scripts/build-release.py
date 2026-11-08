#!/usr/bin/env python3
"""Build a native Windows installer, Debian package or macOS disk image. Python 3.11+."""
import argparse
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
import tempfile
import zipfile

from release_common import ROOT, cargo_metadata, checksum, notices, run, version


def deb_control(app_version, depends, installed_size):
    return (f"Package: pcbcraft\nVersion: {app_version}\nArchitecture: amd64\n"
            "Maintainer: MX-Goliath <98145946+MX-Goliath@users.noreply.github.com>\n"
            "Section: electronics\nPriority: optional\n"
            f"Installed-Size: {installed_size}\nDepends: {depends}\n"
            "Recommends: xdg-desktop-portal, xdg-utils\n"
            "Homepage: https://github.com/MX-Goliath/PCBCraft\n"
            "Description: Independent schematic and PCB editor\n"
            " Rust-based EDA with an independent document model and renderer.\n")


def package_deb(metadata, staging, dist, app_version):
    root = staging / "deb"
    bindir = root / "usr/bin"
    bindir.mkdir(parents=True)
    binary = bindir / "pcbcraft"
    shutil.copy2(Path(metadata["target_directory"]) / "release/pcbcraft", binary)
    binary.chmod(0o755)
    run("strip", "--strip-unneeded", binary)
    for source, target in [("pcbcraft.desktop", "usr/share/applications/pcbcraft.desktop"),
                           ("pcbcraft.svg", "usr/share/icons/hicolor/scalable/apps/pcbcraft.svg")]:
        destination = root / target
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "assets/linux" / source, destination)
    notices(metadata, root / "usr/share/doc/pcbcraft/licenses")
    # shlibdeps needs a Debian source control file, even for standalone binaries.
    source_root = staging / "shlibdeps"
    (source_root / "debian").mkdir(parents=True)
    (source_root / "debian/control").write_text("Source: pcbcraft\nSection: electronics\nPriority: optional\nMaintainer: MX-Goliath <98145946+MX-Goliath@users.noreply.github.com>\n\nPackage: pcbcraft\nArchitecture: any\nDescription: Schematic and PCB editor\n", encoding="utf-8")
    result = subprocess.run(["dpkg-shlibdeps", "-O", f"-e{binary}"], cwd=source_root, check=True, capture_output=True, text=True)
    linked = next(line.removeprefix("shlibs:Depends=") for line in result.stdout.splitlines() if line.startswith("shlibs:Depends="))
    # Winit also dlopens these libraries; they are absent from ELF NEEDED entries.
    depends = linked + ", libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libx11-6, libx11-xcb1, libxi6, libxcursor1, libxrandr2, libxinerama1, libgl1, libegl1"
    installed_size = sum(p.stat().st_size for p in root.rglob("*") if p.is_file()) // 1024 + 1
    (root / "DEBIAN").mkdir()
    (root / "DEBIAN/control").write_text(deb_control(app_version, depends, installed_size), encoding="utf-8")
    output = dist / f"PCBCraft-{app_version}-linux-amd64.deb"
    run("dpkg-deb", "--build", "--root-owner-group", root, output)
    run("dpkg-deb", "--info", output)
    return output


def mac_plist(app_version):
    return {
        "CFBundleName": "PCBCraft", "CFBundleDisplayName": "PCBCraft",
        "CFBundleExecutable": "pcbcraft", "CFBundleIdentifier": "io.github.mx-goliath.pcbcraft",
        "CFBundlePackageType": "APPL", "CFBundleShortVersionString": app_version,
        "CFBundleVersion": app_version, "LSMinimumSystemVersion": "11.0",
        "NSHighResolutionCapable": True,
    }


def package_macos(metadata, staging, dist, app_version):
    contents = staging / "image/PCBCraft.app/Contents"
    (contents / "MacOS").mkdir(parents=True)
    shutil.copy2(Path(metadata["target_directory"]) / "release/pcbcraft", contents / "MacOS/pcbcraft")
    (contents / "MacOS/pcbcraft").chmod(0o755)
    (contents / "Info.plist").write_bytes(plistlib.dumps(mac_plist(app_version)))
    notices(metadata, contents / "Resources/licenses")
    app = contents.parent
    identity = os.environ.get("PCBCRAFT_MACOS_SIGN_IDENTITY", "-")
    command = ["codesign", "--force", "--sign", identity]
    if identity != "-":
        command += ["--options", "runtime", "--timestamp"]
    run(*command, app)
    run("codesign", "--verify", "--strict", app)
    (staging / "image/Applications").symlink_to("/Applications")
    arch = {"arm64": "arm64", "x86_64": "x64"}[platform.machine()]
    output = dist / f"PCBCraft-{app_version}-macos-{arch}.dmg"
    run("hdiutil", "create", "-volname", "PCBCraft", "-srcfolder", staging / "image", "-ov", "-format", "UDZO", output)
    profile = os.environ.get("PCBCRAFT_MACOS_NOTARY_PROFILE")
    if profile:
        if identity == "-":
            raise RuntimeError("Notarization requires a Developer ID signing identity")
        run("xcrun", "notarytool", "submit", output, "--keychain-profile", profile, "--wait")
        run("xcrun", "stapler", "staple", output)
        run("xcrun", "stapler", "validate", output)
    run("hdiutil", "verify", output)
    return output


def package_windows(staging, dist, app_version, iscc, no_build, target=None):
    command = [sys.executable, str(ROOT / "scripts/build-windows-portable.py")]
    if no_build:
        command.append("--no-build")
    if target:
        command += ["--target", target]
    run(*command)
    # Only our freshly generated archive is extracted, never a downloaded update.
    with zipfile.ZipFile(dist / f"PCBCraft-{app_version}-windows-x64-portable.zip") as archive:
        archive.extractall(staging)
    source = staging / f"PCBCraft-{app_version}-windows-x64-portable"
    compiler = shutil.which(iscc) or str(Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Inno Setup 6/ISCC.exe")
    run(compiler, f"/DAppVersion={app_version}", f"/DSourceDir={source}", f"/DOutputDir={dist}", ROOT / "assets/windows/installer.iss")
    return dist / f"PCBCraft-{app_version}-windows-x64-setup.exe"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=["windows-installer", "deb", "macos"], required=True)
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--iscc", default="ISCC.exe")
    parser.add_argument("--target", choices=["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"],
                        help="Explicit Rust target for Windows packaging")
    args = parser.parse_args()
    required = {"windows-installer": "Windows", "deb": "Linux", "macos": "Darwin"}[args.format]
    if platform.system() != required:
        parser.error(f"Build {args.format} natively on {required}")
    if args.format != "macos" and platform.machine().lower() not in ("x86_64", "amd64"):
        parser.error("Windows and Debian packaging currently supports x64 only")
    if not args.no_build and args.format != "windows-installer":
        run("cargo", "build", "--locked", "--release", "-p", "eda-app", "--bin", "pcbcraft")
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".release-", dir=dist) as temporary:
        staging = Path(temporary)
        app_version = version()
        if args.format == "windows-installer":
            output = package_windows(staging, dist, app_version, args.iscc, args.no_build, args.target)
        elif args.format == "deb":
            output = package_deb(cargo_metadata(), staging, dist, app_version)
        else:
            output = package_macos(cargo_metadata(), staging, dist, app_version)
        digest = checksum(output)
        print(f"Built {output}\nSHA256 {digest}")


if __name__ == "__main__":
    main()
