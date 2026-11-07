# Release channel and desktop updates

The default stable channel is [MX-Goliath/PCBCraft Releases](https://github.com/MX-Goliath/PCBCraft/releases).
The application checks `GET /repos/MX-Goliath/PCBCraft/releases/latest` only when
the user presses **Check for updates**, immediately left of Search in the title
bar. The compact rounded control opens a popover, shows checking/error/current/
available/ready states, and shows real byte progress while downloading, both in
the title bar and in the popover. There is a cancel action and a retry action.
This is an explicit user-requested departure from the Altium reference chrome;
ChatGPT Desktop is an approximate visual inspiration, not measured pixel parity.

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

Installation is user initiated, not an unattended replacement/restart. **Open
downloaded package** opens the Windows installer, portable ZIP, DEB, or DMG with
the native shell. AppImage offers **Show downloaded AppImage** to avoid launching
a second GUI. Save work and close PCBCraft before installing. For portable ZIP,
replace application files while retaining `Data` and `portable.flag`. For
AppImage, replace the old file and preserve executable permission. For DMG,
drag the app to Applications. No updater action edits EDA documents, history or
project files. Existing downloaded versions can be removed manually from cache.

## Preparing a release

1. Set `[workspace.package].version` in `Cargo.toml`; update `Cargo.lock`.
2. Run the normal format, strict Clippy and workspace test gates.
3. Push a matching `vMAJOR.MINOR.PATCH` tag when ready to create a release draft.
4. `.github/workflows/release.yml` builds/tests on Windows, Ubuntu 22.04 and native
   macOS Apple Silicon/Intel runners. Windows produces an Inno Setup per-user
   installer and a portable ZIP; Linux produces DEB and AppImage; macOS produces
   an app bundle in DMG. Every asset has a SHA-256 sidecar and license notices.
5. A final job requires all six packages and checks every checksum, then creates
   one **draft** GitHub Release with generated notes. Review the packages and
   publish the draft as a stable release to make it visible to the updater.

`workflow_dispatch` on a branch builds downloadable CI artifacts without creating
a release. Version tags must exactly match the workspace version. Prerelease tags
are intentionally outside this stable-channel workflow. Publishing a partial
platform set is refused. The workflow embeds `github.repository` as
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
python scripts/verify-release.py --tag v0.1.0 --artifacts dist
```

`--no-build` packages an existing native release binary. Windows requires MSVC,
redistributable CRT and Inno Setup 6 (`--iscc` can specify its path). DEB needs
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

Offline Rust regressions cover platform selection, semantic versions, release
filtering, URL/checksum rejection, streamed integrity, cancellation and partial
file cleanup. Python regressions cover tag/version agreement, complete artifact
sets, tampering, bundle metadata and Debian control metadata. Native Windows UI
and workspace gate results are recorded in `PLAN.md`. Linux/macOS native package
builds and installation require their CI runners; a workflow definition alone
does not establish runtime compatibility or visual parity on those platforms.

Protocol reference: [GitHub Releases REST API](https://docs.github.com/en/rest/releases/releases).
Signing reference: [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
