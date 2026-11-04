#!/usr/bin/env python3
"""Check tracked content, or all reachable Git history, without printing secrets.

This detects common accidental disclosures, not every form of personal data.
Synthetic fixtures must be reviewed before adding them; a scanner cannot decide
whether a circuit, image or project name came from a private design.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DESIGNS = (".prjpcb", ".pcbdoc", ".schdoc", ".pcblib", ".schlib", ".intlib",
           ".outjob", ".step", ".stp", ".pcbcraft.json")
EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@([A-Za-z0-9.-]+\.[A-Za-z]{2,})")
HOME = re.compile(r"(?:[A-Za-z]:[\\/]+(?:Users|Documents and Settings)[\\/]+|/(?:home|Users)/)([^\\/\s\x00\"`<>|]+)", re.I)
SECRET = re.compile(r"(?:github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|AKIA[A-Z0-9]{16}|-----BEGIN [A-Z ]*PRIVATE KEY-----)")
LOCAL_DIRS = {"private-projects", "projects", "local", "captures", "screenshots",
              "pcb-update-checks", "Data", "History", ".tools", "target", "dist", ".idea", ".vscode"}


def findings(path: str, data: bytes) -> set[str]:
    problems: set[str] = set()
    parts = Path(path).parts
    if parts and parts[0] in LOCAL_DIRS:
        problems.add("local-only directory")
    if path.lower().endswith(DESIGNS) and not path.startswith("fixtures/"):
        problems.add("design outside reviewed synthetic fixtures")
    if Path(path).name.startswith(".env") or path.lower().endswith((".pem", ".key", ".pfx", ".p12")):
        problems.add("local credentials file")
    # Preserve third-party copyright and license attribution, including in fonts.
    if path.startswith("assets/fonts/"):
        return problems
    texts = [data.decode("utf-8", errors="replace")]
    if b"\x00" in data:
        texts.extend(data.decode(encoding, errors="ignore") for encoding in ("utf-16-le", "utf-16-be"))
    for text in texts:
        for match in HOME.finditer(text):
            if match[1] not in {"example", "..."}:
                problems.add("personal home directory")
        for match in EMAIL.finditer(text):
            domain = match[1].lower()
            if domain not in {"example.com", "example.org", "example.net", "example.test", "example.invalid"}:
                problems.add("email address")
        if SECRET.search(text):
            problems.add("possible credential")
    return problems


def git_objects(records):
    process = subprocess.Popen(["git", "cat-file", "--batch"], cwd=ROOT,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    assert process.stdin is not None and process.stdout is not None
    try:
        for oid, filename in records:
            process.stdin.write(oid + b"\n")
            process.stdin.flush()
            header = process.stdout.readline().split()
            if len(header) != 3:
                raise RuntimeError("Cannot read reachable Git object")
            data = process.stdout.read(int(header[2]))
            if process.stdout.read(1) != b"\n":
                raise RuntimeError("Incomplete Git object")
            kind = header[1].decode("ascii")
            if kind in {"blob", "commit", "tag"}:
                yield filename.decode("utf-8") if filename else kind, data
    finally:
        process.stdin.close()
        process.stdout.close()
        process.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--history", action="store_true", help="Also scan all reachable refs and commit metadata")
    args = parser.parse_args()
    filenames = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode("utf-8").split("\0")
    objects = [(name, (ROOT / name).read_bytes()) for name in filenames if name and (ROOT / name).is_file()]
    entries = subprocess.check_output(["git", "ls-files", "--stage", "-z"], cwd=ROOT).split(b"\0")
    staged = []
    for entry in entries:
        if entry:
            metadata, name = entry.split(b"\t", 1)
            staged.append((metadata.split()[1], name))
    objects.extend(git_objects(staged))
    if args.history:
        records = subprocess.check_output(["git", "rev-list", "--objects", "--all"], cwd=ROOT).splitlines()
        objects.extend(git_objects((record.partition(b" ")[0], record.partition(b" ")[2]) for record in records))
    failures: set[tuple[str, str]] = set()
    for path, data in objects:
        failures.update((path, problem) for problem in findings(path, data))
    for path, problem in sorted(failures):
        print(f"{path}: {problem}")
    if failures:
        print(f"Privacy check failed: {len(failures)} findings. Sensitive values are not printed.")
        return 1
    print(f"Privacy check passed ({len(objects)} files/objects scanned).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
