#!/usr/bin/env python3
"""Reproducible local checks. Writes logs and reports; never uploads project data.

Run from the repository checkout. --reference can be repeated. Reference files
are read only; round-trip outputs are written in a new report directory. Tests
against PCBCraft's own importer do NOT establish Altium interoperability.
"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", action="append", type=Path, default=[])
    parser.add_argument("--format", action="store_true", help="Run rustfmt in this copy before checking")
    parser.add_argument("--no-clippy", action="store_true")
    parser.add_argument("--report-dir", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    reports = (args.report_dir or root / "target" / "pcb-update-checks" / stamp).resolve()
    reports.mkdir(parents=True, exist_ok=False)
    result: dict = {"created_utc": stamp, "project": str(root), "github_writes": 0,
                    "altium_oracle_tested": False, "gui_tested": False, "checks": [], "references": []}

    def save() -> None:
        (reports / "results.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    cargo = shutil.which("cargo")
    if cargo is None:
        result["status"] = "blocked_no_cargo"
        save()
        print(f"Cargo was not found. No Rust check ran. Report: {reports}", file=sys.stderr)
        return 2

    def command(label: str, arguments: list[str]) -> bool:
        number = len(result["checks"]) + 1
        logfile = reports / f"{number:02d}-{label}.log"
        print(label, flush=True)
        try:
            with logfile.open("w", encoding="utf-8") as stream:
                completed = subprocess.run([cargo, *arguments], cwd=root, stdout=stream, stderr=subprocess.STDOUT, check=False)
            rc = completed.returncode
            status = "passed" if rc == 0 else "failed"
        except OSError as error:
            logfile.write_text(str(error), encoding="utf-8")
            rc, status = None, "tool_error"
        result["checks"].append({"name": label, "command": [cargo, *arguments], "status": status,
                                  "returncode": rc, "log": logfile.name})
        save()
        return status == "passed"

    # Validate the committed dependency resolution without upgrading unrelated crates.
    if not command("fetch-locked", ["fetch", "--locked"]):
        result["status"] = "blocked_dependency_resolution"
        save()
        return 2
    if args.format:
        command("format", ["fmt", "--all"])
    command("format-check", ["fmt", "--all", "--", "--check"])
    command("tests", ["test", "--locked", "--workspace"])
    if not args.no_clippy:
        command("clippy", ["clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"])
    built = command("build", ["build", "--locked", "--workspace", "--all-targets"])
    if built:
        inputs = [root / "fixtures/pcb-update/editor-v2.PcbDoc", *args.reference]
        for index, path in enumerate(inputs):
            path = path.resolve()
            if not path.is_file():
                result["references"].append({"path": str(path), "status": "missing"})
                continue
            before = hashlib.sha256(path.read_bytes()).hexdigest()
            out = reports / f"reference-{index:03d}-edited.PcbDoc"
            ok = command(f"roundtrip-{index:03d}", ["run", "--locked", "-p", "eda-io", "--example", "pcb_roundtrip",
                                                    "--", str(path), str(out), "--exercise"])
            unchanged = hashlib.sha256(path.read_bytes()).hexdigest() == before
            result["references"].append({"path": str(path), "sha256": before,
                "status": "own_reader_passed" if ok and unchanged else "failed",
                "input_unchanged": unchanged, "output": str(out), "altium_tested": False})
            save()
    checks_ok = all(c["status"] == "passed" for c in result["checks"])
    refs_ok = all(r["status"] == "own_reader_passed" for r in result["references"])
    result["status"] = "passed_local_checks_only" if checks_ok and refs_ok else "failed"
    save()
    print(f"Report: {reports / 'results.json'}")
    return 0 if checks_ok and refs_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
