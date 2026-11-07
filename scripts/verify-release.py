#!/usr/bin/env python3
"""Validate release version, package completeness and checksums before publication."""
import argparse
import hashlib
from pathlib import Path
import re

from release_common import version


def filenames(app_version):
    return [f"PCBCraft-{app_version}-{suffix}" for suffix in (
        "windows-x64-setup.exe", "windows-x64-portable.zip", "linux-amd64.deb",
        "x86_64.AppImage", "macos-arm64.dmg", "macos-x64.dmg",
    )]


def verify_tag(tag, app_version, allow_branch=False):
    stable = re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", app_version)
    if not stable:
        raise ValueError("The stable release workflow requires a major.minor.patch workspace version")
    if not allow_branch:
        if tag != f"v{app_version}":
            raise ValueError(f"Tag {tag!r} does not match workspace version v{app_version}")


def verify_artifacts(directory, app_version):
    expected = filenames(app_version)
    expected_files = set(expected + [name + ".sha256" for name in expected])
    actual = {path.name for path in directory.iterdir() if path.is_file()}
    if actual != expected_files:
        raise ValueError(f"Release package set mismatch: missing={sorted(expected_files - actual)}, unexpected={sorted(actual - expected_files)}")
    for name in expected:
        path = directory / name
        if path.stat().st_size == 0:
            raise ValueError(f"Empty release package: {name}")
        with path.open("rb") as handle:
            digest = hashlib.file_digest(handle, "sha256").hexdigest()
        checksum = (directory / (name + ".sha256")).read_text(encoding="ascii").split()
        if checksum != [digest, name]:
            raise ValueError(f"SHA-256 mismatch: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--allow-branch", action="store_true", help="Permit a branch name for workflow_dispatch instead of requiring a release tag")
    parser.add_argument("--artifacts", type=Path)
    args = parser.parse_args()
    app_version = version()
    verify_tag(args.tag, app_version, args.allow_branch)
    if args.artifacts:
        verify_artifacts(args.artifacts, app_version)
    print(f"Release validation passed for {app_version}")


if __name__ == "__main__":
    main()
