#!/usr/bin/env python3
"""Build a Windows x64 portable ZIP with Python 3.11+ and the MSVC toolchain."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tempfile
import tomllib
import zipfile

from release_common import notices


ROOT = Path(__file__).resolve().parents[1]


def run(*args):
    return subprocess.run(args, cwd=ROOT, check=True, capture_output=True, text=True)


def imports(path):
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise RuntimeError(f"Not a PE executable: {path}")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    if machine != 0x8664:
        raise RuntimeError(f"Expected Windows x64 executable: {path}")
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    if struct.unpack_from("<H", data, optional)[0] != 0x20B:
        raise RuntimeError(f"Expected PE32+: {path}")
    sections = []
    for index in range(count):
        section = optional + optional_size + index * 40
        virtual_size, rva, raw_size, raw = struct.unpack_from("<IIII", data, section + 8)
        sections.append((rva, max(virtual_size, raw_size), raw))

    def offset(rva):
        for start, size, raw in sections:
            if start <= rva < start + size:
                return raw + rva - start
        raise RuntimeError(f"Invalid PE RVA {rva:x}: {path}")

    import_rva = struct.unpack_from("<I", data, optional + 120)[0]
    if not import_rva:
        return []
    descriptor = offset(import_rva)
    names = []
    while any(data[descriptor:descriptor + 20]):
        name = offset(struct.unpack_from("<I", data, descriptor + 12)[0])
        names.append(data[name:data.index(b"\0", name)].decode("ascii"))
        descriptor += 20
    return names


def find_crt():
    vswhere = Path(os.environ.get("ProgramFiles(x86)", "")) / "Microsoft Visual Studio/Installer/vswhere.exe"
    if not vswhere.is_file():
        return None
    installations = json.loads(run(str(vswhere), "-products", "*", "-format", "json").stdout)
    candidates = []
    for installation in installations:
        base = Path(installation["installationPath"]) / "VC/Redist/MSVC"
        candidates += list(base.glob("*/x64/Microsoft.VC*.CRT"))
    return sorted(candidates)[-1] if candidates else None


def bundle_dependencies(binary, destination, crt_dir):
    system = Path(os.environ["SystemRoot"]) / "System32"
    queue, bundled, dependencies = [binary], set(), {}
    while queue:
        current = queue.pop()
        names = imports(current)
        dependencies[current.name] = names
        for name in names:
            lower = name.lower()
            if lower in bundled:
                continue
            runtime = re.match(r"(?:vcruntime|msvcp|concrt)\d+.*\.dll$", lower)
            if runtime:
                source = crt_dir / name if crt_dir else None
                if source is None or not source.is_file():
                    raise RuntimeError(f"Supply --crt-dir with the redistributable x64 CRT containing {name}")
                target = destination / name
                shutil.copy2(source, target)
                bundled.add(lower)
                queue.append(target)
            elif not lower.startswith(("api-ms-win-", "ext-ms-win-")) and not (system / name).is_file():
                raise RuntimeError(f"Unresolved dependency {name} of {current.name}")
    return dependencies


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--crt-dir", type=Path, help="MSVC redistributable x64 CRT directory")
    args = parser.parse_args()
    if platform.system() != "Windows" or platform.machine().lower() not in ("amd64", "x86_64"):
        parser.error("Run on Windows x64 with the native MSVC Rust toolchain")
    if not args.no_build:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "eda-app", "--bin", "pcbcraft"], cwd=ROOT, check=True)
    metadata = json.loads(run("cargo", "metadata", "--locked", "--format-version=1", "--filter-platform", "x86_64-pc-windows-msvc").stdout)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    name = f"PCBCraft-{version}-windows-x64-portable"
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    output = dist / f"{name}.zip"
    with tempfile.TemporaryDirectory(prefix=".windows-", dir=dist) as temporary:
        folder = Path(temporary) / name
        folder.mkdir()
        binary = folder / "pcbcraft.exe"
        shutil.copy2(Path(metadata["target_directory"]) / "release/pcbcraft.exe", binary)
        dependencies = bundle_dependencies(binary, folder, args.crt_dir or find_crt())
        (folder / "portable.flag").write_text("Keep this file beside pcbcraft.exe to store settings in Data/app.ron.\n", encoding="utf-8")
        (folder / "Data").mkdir()
        (folder / "Diagnose.cmd").write_text(
            '@echo off\nsetlocal\n'
            'if not exist "%~dp0Data" mkdir "%~dp0Data"\n'
            'set "PCBCRAFT_IMPORT_PROFILE=1"\n'
            'set "RUST_LOG=pcbcraft=info,eda_app=info,wgpu_hal=error"\n'
            '"%~dp0pcbcraft.exe" %* >"%~dp0Data\\diagnostic.log" 2>&1\n',
            encoding="ascii",
        )
        (folder / "README.txt").write_text(
            "PCBCraft для Windows x64 — portable\n\n"
            "Распакуйте всю папку в доступный для записи каталог и запустите pcbcraft.exe.\n"
            "Установка Rust, Altium или Visual C++ Redistributable не требуется.\n"
            "Настройки сохраняются в Data/app.ron рядом с программой, пока присутствует portable.flag.\n"
            "Для переноса настроек копируйте всю папку. Удаление portable.flag включает обычное хранилище AppData.\n"
            "Проекты открываются через File > Open или передачей пути первым аргументом.\n"
            "Нужны Windows 10/11 x64 и драйвер GPU с поддержкой используемого wgpu графического backend.\n\n"
            "Диагностика открытия: сначала закройте PCBCraft, затем запустите Diagnose.cmd\n"
            "и откройте плату (либо перетащите PCB-файл на Diagnose.cmd). Этапы загрузки\n"
            "и обработки STEP сохраняются в Data/diagnostic.log.\n\n"
            "Это текущая версия разработки. Сохранение PcbDoc/SchDoc поддерживает не все изменения;\n"
            "работайте с копиями исходных проектов. Подробности: docs/pcb-update/DESIGN_SYNC.md\n"
            "в исходном репозитории PCBCraft.\n",
            encoding="utf-8-sig",
        )
        notices(metadata, folder / "licenses")
        revision = run("git", "rev-parse", "HEAD").stdout.strip()
        dirty = bool(run("git", "status", "--porcelain").stdout.strip())
        (folder / "build-info.json").write_text(json.dumps({
            "version": version, "revision": revision, "working_tree_modified": dirty,
            "built_at_utc": datetime.now(timezone.utc).isoformat(),
            "target": "x86_64-pc-windows-msvc", "profile": "release", "dependencies": dependencies,
            "rustc": run("rustc", "--version").stdout.strip(),
            "executable_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        }, indent=2), encoding="utf-8")
        archive = Path(temporary) / output.name
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9, strict_timestamps=False) as handle:
            for entry in sorted(folder.rglob("*")):
                handle.write(entry, entry.relative_to(folder.parent))
        with zipfile.ZipFile(archive) as handle:
            if handle.testzip() is not None:
                raise RuntimeError("ZIP verification failed")
        archive.replace(output)
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(".zip.sha256").write_text(f"{digest}  {output.name}\n", encoding="ascii")
    print(f"Built {output}\nSHA256 {digest}")


if __name__ == "__main__":
    main()
