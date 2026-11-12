# Release channel and desktop updates

The default stable channel is [MX-Goliath/PCBCraft Releases](https://github.com/MX-Goliath/PCBCraft/releases).
The application checks `GET /repos/MX-Goliath/PCBCraft/releases/latest` at
startup and every hour while running, including after network failures. A dedicated
background clock drives checks and starts downloads without requiring redraws,
including while the application is minimized. A newer
native stable package downloads automatically in a background thread. Only one
check/download runs at a time. A verified, durable pending update is retained
while hourly metadata checks continue, so a later network error cannot hide it.

The title-bar control immediately left of Search displays readable status text.
During downloads a blue progress stroke follows its rounded perimeter, using
received bytes; the popover also shows percentage and MB. **Pause download**
stops the current transfer; **Retry download** resumes with a fresh verified
transfer, and the next hourly check also retries. **Restart to update** runs the
normal unsaved-document Save/Discard/Cancel flow. Cancelling that flow cancels
the restart. The 23-point hit rectangle stays within the title strip, with the
painted border inset on all sides. This update control is an explicit requested
extension to the Altium reference chrome, not a claim of Altium pixel parity.

## Package selection

Stable semantic version tags such as `v0.2.0` are compared with the embedded
workspace version. Drafts, prereleases, older versions and build-metadata-only
changes are excluded. Missing native assets produce an error rather than selecting
another architecture or installation type.

| Installation | Release asset |
| --- | --- |
| Windows x64, installed/default | `PCBCraft-VERSION-windows-x64-setup.exe` |
| Windows x64 with `portable.flag` beside the executable | `PCBCraft-VERSION-windows-x64-portable.zip` |
| Linux x86_64, DEB/default | `PCBCraft-VERSION-linux-amd64.deb` |
| Linux x86_64 with AppImage runtime `APPIMAGE` set | `PCBCraft-VERSION-x86_64.AppImage` |
| macOS Apple Silicon | `PCBCraft-VERSION-macos-arm64.dmg` |
| macOS Intel | `PCBCraft-VERSION-macos-x64.dmg` |

Other architectures report that no package is configured. A Linux source build
selects DEB by default. An Intel build running under Rosetta selects the Intel
package. No authentication token is embedded or requested: before the repository
is public and a stable release is published, GitHub's 404 is shown as **No releases
yet**, with an explanation that the repository may be private. Rate limits,
network errors and missing packages are displayed separately.

## Download and installation behavior

HTTPS uses the operating system trust store. Metadata/checksum responses have
size limits, package sizes are bounded to 4 GiB and selected URLs must belong to
the configured repository/tag. Downloads stream to private temporary files;
progress reflects received bytes. The GitHub asset's `sha256:` digest is required,
or, for older assets without a digest, an exact-name `.sha256` sidecar. Integrity
and length must match before a package becomes available to open. These checks
protect transport/package integrity; they are not a separate publisher signature.
Interrupted, corrupt, oversized and cancelled downloads are discarded. Cancellation
is observed between network reads; a blocked request is subject to its timeout
(30 seconds for metadata/checksums; at most one hour for a package request).

Verified packages remain in their own `release-*` subdirectories under:

- Installed Windows: `%LOCALAPPDATA%/PCBCraft/updates`.
- Portable Windows: `Data/Updates` beside the executable.
- Linux: `$XDG_CACHE_HOME/PCBCraft/updates` or `~/.cache/PCBCraft/updates`.
- macOS: `~/Library/Caches/PCBCraft/updates`.

After verification, a per-installation `pending.json` records the package,
version, SHA-256 and destination executable. **Restart to update** closes the GUI,
flushes application settings, starts a copy of PCBCraft in update-worker mode,
and relaunches after installation, preserving command-line arguments and the
original working directory for relative project paths. An ordinary close installs without relaunching.
If an earlier process ended before hand-off, the next launch applies the pending
package before opening the editor. The worker waits for the old process to exit
before changing files and serializes replacement with an exclusive file lock.
It rechecks the package hash immediately before installation.

- **Windows portable:** validates/extracts the ZIP, replaces the executable,
  runtime DLLs, licenses and other shipped files in the same directory. Removes
  obsolete files using the previous managed-file inventory (older releases use
  `build-info.json` and known shipped paths). Preserves `Data`, `portable.flag`
  and unrelated user files. Backup and a persisted replacement journal support
  rollback on replacement errors and recovery on a subsequent worker attempt.
  No elevation, installer, PowerShell or archive UI is used; the portable folder
  must be writable by its owner. Archive traversal, symlinks, Windows device
  names, alternate streams and case-duplicate file paths are rejected.
- **Windows installed:** runs the verified per-user Inno installer silently,
  targeting the current installation directory, with OS reboot and automatic
  application closing/relaunching disabled. Obsolete DLLs and license files are
  removed by the installer. Installer results are checked and logged.
- **AppImage:** copies to a temporary file beside the original AppImage, sets
  executable permissions and atomically replaces the original pathname. No
  elevation is requested for a user-owned writable location.
- **DEB:** invokes `pkexec /usr/bin/dpkg --install` on the verified DEB after exit,
  preserving package-manager ownership of all installed files. System package
  installation can require the operating system's administrator authorization.
- **macOS:** mounts the DMG read-only, stages and verifies the full app bundle,
  replaces the original bundle and restores the previous one on rename failure.
  The enclosing application directory must be writable.

Errors retain the downloaded package and are displayed on the next launch, with
**Retry installation on restart**. A failed install does not trigger an endless
startup/restart loop. Update-worker copies and packages stay in the update cache
and may be removed while no update is running. EDA documents and project files
are never installation targets. The updater code reaches existing installations
only after they install a release containing it; old executables retain their
previous update behavior until that first upgrade.

## Preparing a release

1. Set `[workspace.package].version` in `Cargo.toml`; update `Cargo.lock`.
2. Run the normal format, strict Clippy and workspace test gates.
3. Build the requested packages locally. Windows produces an Inno Setup per-user
   installer and a portable ZIP. Include a SHA-256 sidecar and license notices.
4. Verify the exact package set, tag and embedded version. A Windows-only release
   must explicitly select `--platform windows`; the default verifier requires
   all six Windows/Linux/macOS packages.
5. Commit the release sources, build/package from that clean revision, push a
   matching `vMAJOR.MINOR.PATCH` tag and upload only the verified assets to a
   GitHub Release. Publish after checking the uploaded sizes and hashes.

Both workflows use `workflow_dispatch` only: pushing commits/tags or publishing
a release does not start GitHub Actions. Version 0.0.2 Windows packages are built
and uploaded locally without using Actions minutes.

The optional release workflow builds/tests all platforms on Windows, Ubuntu 22.04
and native macOS runners when explicitly dispatched. A branch dispatch produces
downloadable CI artifacts; a tag dispatch requires all six packages and creates
a draft release. Version tags must exactly match the workspace version.
Prerelease tags are outside this stable-channel workflow.
The workflow embeds `github.repository` as
`PCBCRAFT_UPDATE_REPOSITORY`; for a local fork/channel override, set that environment
variable to `owner/repository` **when compiling**, not when launching the app.
The public channel needs no additional backend or release credentials in clients.
The workflow uses its scoped `GITHUB_TOKEN` only to create a release draft.
CI disables debug symbols and incremental artifacts to keep the full workspace
checks and both build profiles within hosted-runner disk limits; local development
profiles retain their usual debug information.

Native packaging commands (Python 3.11+ and Rust 1.90):

```text
python scripts/build-release.py --format windows-installer
python scripts/build-windows-portable.py
python scripts/build-release.py --format deb
python scripts/build-appimage.py
python scripts/build-release.py --format macos
python -m unittest discover -s scripts -p test_release.py
python scripts/verify-release.py --tag v0.0.2 --artifacts dist
python scripts/verify-release.py --tag v0.0.2 --platform windows --artifacts dist
```

`--no-build` packages an existing native release binary. Windows supports native
MSVC or GNU toolchains and needs Inno Setup 6 (`--iscc` can specify its path).
The portable metadata records the actual target; `--target` selects an explicit
Windows x64 target and its corresponding Cargo output directory. MSVC runtime
DLLs are bundled when imported, using the redistributable CRT directory.
DEB needs
`dpkg-dev`, `binutils`, OpenSSL development headers and the documented X11/Wayland
libraries; runtime dependencies are generated with `dpkg-shlibdeps` plus dlopened
windowing libraries. AppImage uses the existing bundler, now also including Rust
dependency license notices. Linux artifacts are built on Ubuntu 22.04 to establish
a glibc 2.35 baseline, subject to native runtime testing.

macOS uses `hdiutil` and `codesign`. By default the app is ad-hoc signed, suitable
for development/testing; production Gatekeeper acceptance is **not configured**.
For a production release, provision an Apple Developer ID certificate in the
build keychain, set `PCBCRAFT_MACOS_SIGN_IDENTITY`, and optionally set
`PCBCRAFT_MACOS_NOTARY_PROFILE` to a preconfigured `notarytool` keychain profile.
The packager supports hardened-runtime signing, notarization and stapling when
these are supplied. The stock GitHub workflow does not provision Apple credentials.
Windows Authenticode signing is also not configured. Do not describe these
packages as publisher signed/notarized until those release credentials are set up.

## Verification limits

Offline Rust regressions cover platform selection, scheduling deadlines,
semantic versions, release filtering, URL/checksum rejection, streamed integrity,
cancellation, title-strip clipping and hit testing, portable payload replacement,
obsolete-file cleanup, preserved user data, archive rejection, rollback, journal
recovery, package tampering and AppImage byte replacement. Python regressions
cover release metadata and checksums. Windows live UI and process-handoff results
and workspace gate outcomes are recorded in `PLAN.md`. Linux/macOS native update
execution and administrator prompts require platform testing; source coverage
alone does not establish native installation compatibility or visual parity.

Protocol reference: [GitHub Releases REST API](https://docs.github.com/en/rest/releases/releases).
Signing reference: [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

## Native UI regression fixtures (debug Windows only)

`python scripts/prepare-desktop-qa.py` creates a new disposable folder under
`target/desktop-qa-*`: a portable debug application, local release templates,
a native schematic/library project, two clones and a local bare Git remote.
It does not change installed applications or publish a release. The test channel
requires both `portable.flag` and `update-qa.flag` beside the debug executable,
plus its `UpdateQA` directory. This code is excluded from normal release builds.
Without the explicit marker the debug application uses the production channel.

The optional `UpdateQA/release.json` is GitHub-shaped metadata; missing metadata
means no public release. `release-current.json` and `release-new.json` are prepared
templates. Metadata still goes through the production native/version/URL/size
selection; package bytes come from the corresponding local filename and pass
through the same streamed SHA-256 verifier, checkpoint and real update worker.
`check-delay-ms` and `chunk-ms` allow the transient controls to be inspected.
`interval-ms` can scale the hour down for a minimized-window regression; production
always uses 3,600 seconds. Record the interval override when reporting results.
Local fixture QA does not establish a successful live GitHub package transfer.

`python scripts/verify-desktop-update.py target/debug/pcbcraft.exe` exercises the
real worker in disposable directories: parent-process waiting, complete payload
replacement, corruption refusal, rollback and relaunch of a different executable
that records Unicode arguments, working directory and environment cleanup.
