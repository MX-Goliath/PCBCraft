"""Release contract tests: the workflow must publish a complete, verified set."""
import importlib.util
import hashlib
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest import mock

from release_common import checksum


def load(filename, name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parent / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


verify = load("verify-release.py", "verify_release")
build = load("build-release.py", "build_release")
appimage = load("build-appimage.py", "build_appimage")
portable = load("build-windows-portable.py", "build_windows_portable")


class ReleaseTests(unittest.TestCase):
    def test_windows_only_release_is_explicit_and_still_checks_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            for name in verify.filenames("0.0.1", "windows"):
                path = directory / name
                path.write_bytes(b"verified Windows package")
                checksum(path)
            verify.verify_artifacts(directory, "0.0.1", "windows")
            with self.assertRaisesRegex(ValueError, "missing"):
                verify.verify_artifacts(directory, "0.0.1")
            package = directory / verify.filenames("0.0.1", "windows")[0]
            package.write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                verify.verify_artifacts(directory, "0.0.1", "windows")

    def test_windows_portable_records_actual_target_and_excludes_user_settings(self):
        for explicit_target in [None, "x86_64-pc-windows-msvc"]:
            with self.subTest(target=explicit_target), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = explicit_target or "x86_64-pc-windows-gnu"
                build_dir = root / "target"
                if explicit_target:
                    build_dir /= explicit_target
                (build_dir / "release").mkdir(parents=True)
                (build_dir / "release/pcbcraft.exe").write_bytes(b"synthetic PE payload")
                (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.0.1"\n', encoding="utf-8")

                def command(*args):
                    if args[:2] == ("rustc", "-vV"):
                        return mock.Mock(stdout="host: x86_64-pc-windows-gnu\n")
                    if args[:2] == ("rustc", "--version"):
                        return mock.Mock(stdout="rustc 1.90.0\n")
                    if args[:2] == ("cargo", "metadata"):
                        self.assertEqual(args[-1], target)
                        return mock.Mock(stdout=json.dumps({"target_directory": str(root / "target")}))
                    if args == ("git", "rev-parse", "HEAD"):
                        return mock.Mock(stdout="a" * 40 + "\n")
                    self.assertEqual(args, ("git", "status", "--porcelain"))
                    return mock.Mock(stdout="")

                argv = ["build-windows-portable.py", "--no-build"]
                if explicit_target:
                    argv += ["--target", explicit_target]
                with (mock.patch.object(portable, "ROOT", root),
                      mock.patch.object(portable.platform, "system", return_value="Windows"),
                      mock.patch.object(portable.platform, "machine", return_value="AMD64"),
                      mock.patch.object(portable, "run", side_effect=command),
                      mock.patch.object(portable, "find_crt", return_value=None),
                      mock.patch.object(portable, "bundle_dependencies", return_value={}),
                      mock.patch.object(portable, "notices"),
                      mock.patch("sys.argv", argv)):
                    portable.main()
                import zipfile
                output = root / "dist/PCBCraft-0.0.1-windows-x64-portable.zip"
                with zipfile.ZipFile(output) as archive:
                    prefix = "PCBCraft-0.0.1-windows-x64-portable/"
                    info = json.loads(archive.read(prefix + "build-info.json"))
                    self.assertEqual(info["target"], target)
                    self.assertEqual(info["version"], "0.0.1")
                    self.assertFalse(info["working_tree_modified"])
                    self.assertIn(prefix + "portable.flag", archive.namelist())
                    self.assertNotIn(prefix + "Data/app.ron", archive.namelist())
                verify_checksum = hashlib.sha256(output.read_bytes()).hexdigest()
                self.assertEqual(output.with_suffix(".zip.sha256").read_text(), f"{verify_checksum}  {output.name}\n")

    def test_appimage_staging_collects_licenses_before_packaging(self):
        # Exercise staging on every CI host, replacing only native Linux tools
        # and symlink creation (which requires elevated rights on some Windows hosts).
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "1.2.3"\n', encoding="utf-8")
            (root / "target/release").mkdir(parents=True)
            (root / "target/release/pcbcraft").write_bytes(b"binary fixture")
            (root / "assets/linux").mkdir(parents=True)
            for name in ["pcbcraft.desktop", "pcbcraft.svg"]:
                (root / "assets/linux" / name).write_text("fixture", encoding="utf-8")
            tool = root / "appimagetool"
            tool.write_bytes(b"tool fixture")
            metadata = {"target_directory": str(root / "target"), "packages": []}

            def native_tool(*args, **kwargs):
                if "metadata" in args:
                    return mock.Mock(stdout=json.dumps(metadata))
                if args[0] == "strip":
                    return mock.Mock()
                self.assertEqual(args[0], str(tool))
                staged = Path(args[-2])
                licenses = staged / "usr/share/licenses/pcbcraft"
                self.assertTrue((licenses / "PCBCraft-MIT.txt").is_file())
                self.assertEqual(json.loads((licenses / "dependencies.json").read_text(encoding="utf-8")), [])
                self.assertTrue((staged / "AppRun").is_file())
                Path(args[-1]).write_bytes(b"packaged fixture")
                return mock.Mock()

            with (mock.patch.object(appimage, "ROOT", root),
                  mock.patch.object(appimage.platform, "system", return_value="Linux"),
                  mock.patch.object(appimage.platform, "machine", return_value="x86_64"),
                  mock.patch.object(appimage.shutil, "which", return_value=str(tool)),
                  mock.patch.object(appimage, "bundle_libraries", return_value={}),
                  mock.patch.object(appimage, "run", side_effect=native_tool),
                  mock.patch.object(Path, "symlink_to"),
                  mock.patch("sys.argv", ["build-appimage.py", "--no-build"])):
                appimage.main()
            output = root / "dist/PCBCraft-1.2.3-x86_64.AppImage"
            self.assertEqual(output.read_bytes(), b"packaged fixture")
            digest = hashlib.sha256(output.read_bytes()).hexdigest()
            self.assertEqual(output.with_suffix(".AppImage.sha256").read_text(), f"{digest}  {output.name}\n")

    def test_tags_match_embedded_version(self):
        verify.verify_tag("v1.2.3", "1.2.3")
        verify.verify_tag("main", "1.2.3", allow_branch=True)
        verify.verify_tag("variants", "1.2.3", allow_branch=True)
        for tag, app_version in [("v1.2.4", "1.2.3"), ("1.2.3", "1.2.3"),
                                 ("v1.2.3-beta.1", "1.2.3-beta.1"), ("v01.2.3", "01.2.3")]:
            with self.assertRaises(ValueError):
                verify.verify_tag(tag, app_version)

    def test_complete_set_and_checksums(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            for name in verify.filenames("1.2.3"):
                path = directory / name
                path.write_bytes(b"synthetic package fixture")
                checksum(path)
            verify.verify_artifacts(directory, "1.2.3")
            package = directory / verify.filenames("1.2.3")[0]
            package.write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                verify.verify_artifacts(directory, "1.2.3")
            checksum(package)
            verify.verify_artifacts(directory, "1.2.3")
            package.unlink()
            with self.assertRaisesRegex(ValueError, "missing"):
                verify.verify_artifacts(directory, "1.2.3")

    def test_extra_assets_cannot_be_accidentally_published(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            for name in verify.filenames("1.2.3"):
                path = directory / name
                path.write_bytes(b"package")
                checksum(path)
            (directory / "private-design.json").write_text("private", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "unexpected"):
                verify.verify_artifacts(directory, "1.2.3")

    def test_mac_bundle_metadata_roundtrips(self):
        info = plistlib.loads(plistlib.dumps(build.mac_plist("1.2.3")))
        self.assertEqual(info["CFBundleExecutable"], "pcbcraft")
        self.assertEqual(info["CFBundleShortVersionString"], "1.2.3")
        self.assertEqual(info["CFBundleVersion"], "1.2.3")
        self.assertEqual(info["LSMinimumSystemVersion"], "11.0")

    def test_deb_control_has_native_dependencies_and_version(self):
        control = build.deb_control("1.2.3", "libc6 (>= 2.35), libssl3", 1024)
        self.assertIn("Version: 1.2.3\n", control)
        self.assertIn("Architecture: amd64\n", control)
        self.assertIn("Depends: libc6 (>= 2.35), libssl3\n", control)
        self.assertTrue(control.endswith("\n"))


if __name__ == "__main__":
    unittest.main()
