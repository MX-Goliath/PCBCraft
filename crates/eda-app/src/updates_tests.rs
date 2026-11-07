use super::*;

fn release(tag: &str, distribution: Distribution, arch: &str) -> Release {
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).unwrap();
    let name = distribution.filename(&version, arch);
    Release {
        tag_name: tag.into(),
        draft: false,
        prerelease: false,
        assets: vec![Asset {
            browser_download_url: format!(
                "https://github.com/owner/repo/releases/download/{tag}/{name}"
            ),
            name,
            size: 123,
            digest: Some(format!("sha256:{}", "ab".repeat(32))),
        }],
    }
}

fn select(release: Release) -> Result<(Phase, Option<Package>), String> {
    select_release(
        release,
        &Version::parse("0.1.0").unwrap(),
        Distribution::WindowsInstaller,
        "x86_64",
        "owner/repo",
    )
}

#[test]
fn chooses_native_packages_without_switching_installation_type() {
    for (os, arch, portable, appimage, kind, suffix) in [
        (
            "windows",
            "x86_64",
            false,
            false,
            Distribution::WindowsInstaller,
            "windows-x64-setup.exe",
        ),
        (
            "windows",
            "x86_64",
            true,
            false,
            Distribution::WindowsPortable,
            "windows-x64-portable.zip",
        ),
        (
            "linux",
            "x86_64",
            false,
            false,
            Distribution::Deb,
            "linux-amd64.deb",
        ),
        (
            "linux",
            "x86_64",
            false,
            true,
            Distribution::AppImage,
            "x86_64.AppImage",
        ),
        (
            "macos",
            "aarch64",
            false,
            false,
            Distribution::Mac,
            "macos-arm64.dmg",
        ),
        (
            "macos",
            "x86_64",
            false,
            false,
            Distribution::Mac,
            "macos-x64.dmg",
        ),
    ] {
        assert_eq!(
            Distribution::detect(os, arch, portable, appimage).unwrap(),
            kind
        );
        let (_, package) = select_release(
            release("v0.2.0", kind, arch),
            &Version::parse("0.1.0").unwrap(),
            kind,
            arch,
            "owner/repo",
        )
        .unwrap();
        assert!(package.unwrap().asset.name.ends_with(suffix));
    }
    assert!(Distribution::detect("linux", "aarch64", false, false).is_err());
    assert!(Distribution::detect("windows", "aarch64", false, false).is_err());
}

#[test]
fn semantic_version_ordering_rejects_downgrades_and_build_only_updates() {
    for tag in ["v0.0.9", "0.1.0", "v0.1.0+build.42"] {
        assert_eq!(
            select(release(tag, Distribution::WindowsInstaller, "x86_64"))
                .unwrap()
                .0,
            Phase::Current
        );
    }
    assert_eq!(
        select(release("v0.10.0", Distribution::WindowsInstaller, "x86_64"))
            .unwrap()
            .0,
        Phase::Available
    );
    let mut invalid = release("v0.2.0", Distribution::WindowsInstaller, "x86_64");
    invalid.tag_name = "latest".into();
    assert!(select(invalid).is_err());
}

#[test]
fn stable_channel_excludes_drafts_and_prereleases() {
    for (tag, draft, prerelease) in [
        ("v0.2.0", true, false),
        ("v0.2.0", false, true),
        ("v0.2.0-beta.1", false, false),
    ] {
        let mut r = release(tag, Distribution::WindowsInstaller, "x86_64");
        r.draft = draft;
        r.prerelease = prerelease;
        assert_eq!(select(r).unwrap().0, Phase::Unpublished);
    }
}

#[test]
fn missing_platform_package_does_not_offer_another_platform() {
    assert!(
        select(release("v0.2.0", Distribution::WindowsPortable, "x86_64"))
            .unwrap_err()
            .contains("no PCBCraft")
    );
}

#[test]
fn refuses_untrusted_urls_sizes_and_invalid_checksums() {
    for url in [
        "http://github.com/owner/repo/releases/download/v0.2.0/a.exe",
        "https://evil.example/a.exe",
        "https://github.com/other/repo/releases/download/v0.2.0/a.exe",
        "https://github.com/owner/repo/releases/download/v0.2.0/../../a.exe",
    ] {
        let mut r = release("v0.2.0", Distribution::WindowsInstaller, "x86_64");
        r.assets[0].browser_download_url = url.into();
        assert!(select(r).is_err());
    }
    for size in [0, MAX_PACKAGE_SIZE + 1] {
        let mut r = release("v0.2.0", Distribution::WindowsInstaller, "x86_64");
        r.assets[0].size = size;
        assert!(select(r).is_err());
    }
    for digest in [
        None,
        Some("sha256:broken".into()),
        Some(format!("sha512:{}", "ab".repeat(32))),
    ] {
        let mut r = release("v0.2.0", Distribution::WindowsInstaller, "x86_64");
        r.assets[0].digest = digest;
        assert!(select(r).is_err());
    }
}

#[test]
fn accepts_valid_sidecar_when_github_digest_is_absent() {
    let mut r = release("v0.2.0", Distribution::WindowsInstaller, "x86_64");
    r.assets[0].digest = None;
    let mut sidecar = r.assets[0].clone();
    sidecar.name.push_str(".sha256");
    sidecar.browser_download_url.push_str(".sha256");
    sidecar.size = 160;
    r.assets.push(sidecar);
    assert_eq!(select(r).unwrap().0, Phase::Available);
    let digest = "AB".repeat(32);
    assert_eq!(
        checksum_contents(format!("{digest}  package.zip\n").as_bytes(), "package.zip").unwrap(),
        digest.to_ascii_lowercase()
    );
    assert!(
        checksum_contents(format!("{digest}  another.zip\n").as_bytes(), "package.zip").is_err()
    );
}

#[test]
fn repository_and_metadata_are_bounded() {
    for repository in ["owner/repo", "MX-Goliath/PCBCraft", "owner/repo.name"] {
        assert!(validate_repository(repository).is_ok());
    }
    for repository in [
        "",
        "../repo",
        "owner/..",
        "owner/repo/extra",
        "owner/repo?token=secret",
        "https://github.com/a/b",
    ] {
        assert!(validate_repository(repository).is_err());
    }
    assert_eq!(read_limited(&b"1234"[..], 4).unwrap(), b"1234");
    assert!(read_limited(&b"12345"[..], 4).is_err());
}

#[test]
fn streams_verifies_and_publishes_without_overwriting_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = vec![42; 200_000];
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let mut events = Vec::new();
    let cancelled = AtomicBool::new(false);
    let run = |events: &mut Vec<_>| {
        transfer(
            &bytes[..],
            directory.path(),
            "package.zip",
            bytes.len() as u64,
            &digest,
            &cancelled,
            |n, verifying| events.push((n, verifying)),
        )
        .unwrap()
        .unwrap()
    };
    let first = run(&mut events);
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    let second = run(&mut events);
    assert_ne!(first, second);
    assert!(first.is_file());
    assert_eq!(events.last(), Some(&(bytes.len() as u64, true)));
    assert!(
        !directory.path().read_dir().unwrap().any(|p| p
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "part"))
    );
}

#[test]
fn corrupt_truncated_oversized_and_cancelled_downloads_leave_no_packages() {
    let directory = tempfile::tempdir().unwrap();
    let digest = format!("{:x}", Sha256::digest(b"good"));
    let cancelled = AtomicBool::new(false);
    for (bytes, size) in [(&b"evil"[..], 4), (&b"goo"[..], 4), (&b"good-extra"[..], 4)] {
        assert!(
            transfer(
                bytes,
                directory.path(),
                "package.zip",
                size,
                &digest,
                &cancelled,
                |_, _| {}
            )
            .is_err()
        );
        assert_eq!(directory.path().read_dir().unwrap().count(), 0);
    }
    cancelled.store(true, Ordering::Relaxed);
    assert!(
        transfer(
            &b"good"[..],
            directory.path(),
            "package.zip",
            4,
            &digest,
            &cancelled,
            |_, _| {}
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(directory.path().read_dir().unwrap().count(), 0);
}

#[test]
fn cancellation_during_verification_does_not_publish() {
    let directory = tempfile::tempdir().unwrap();
    let cancelled = AtomicBool::new(false);
    let result = transfer(
        &b"good"[..],
        directory.path(),
        "package.zip",
        4,
        &format!("{:x}", Sha256::digest(b"good")),
        &cancelled,
        |_, verifying| {
            if verifying {
                cancelled.store(true, Ordering::Relaxed);
            }
        },
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(directory.path().read_dir().unwrap().count(), 0);
}

#[test]
fn reports_intermediate_bytes_and_cancels_an_in_flight_transfer() {
    struct SlowReader(std::io::Cursor<Vec<u8>>);
    impl Read for SlowReader {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_millis(110));
            self.0.read(bytes)
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let bytes = vec![7; 128 * 1024];
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let cancelled = AtomicBool::new(false);
    let mut events = Vec::new();
    let result = transfer(
        SlowReader(std::io::Cursor::new(bytes.clone())),
        directory.path(),
        "package.zip",
        bytes.len() as u64,
        &digest,
        &cancelled,
        |n, verifying| {
            events.push((n, verifying));
            cancelled.store(true, Ordering::Relaxed);
        },
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(events, vec![(64 * 1024, false)]);
    assert_eq!(directory.path().read_dir().unwrap().count(), 0);
}

#[test]
fn popup_and_title_control_render_in_every_phase() {
    let (_, package) = select(release("v0.2.0", Distribution::WindowsInstaller, "x86_64")).unwrap();
    for phase in [
        Phase::Idle,
        Phase::Checking,
        Phase::Current,
        Phase::Unpublished,
        Phase::Available,
        Phase::Downloading,
        Phase::Verifying,
        Phase::Ready,
        Phase::Error("Offline".into()),
    ] {
        let ctx = egui::Context::default();
        let updater = Updater::default();
        *updater.state.lock().unwrap() = State {
            phase,
            package: package.clone(),
            downloaded: 60,
            path: None,
        };
        let mut text = String::new();
        for _ in 0..2 {
            egui::Popup::open_id(&ctx, egui::Id::new("desktop_updates_popup"));
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| updater.show(ui));
            });
            fn collect(shape: &egui::Shape, text: &mut String) {
                match shape {
                    egui::Shape::Text(t) => {
                        text.push_str(&t.galley.job.text);
                        text.push('\n');
                    }
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, text);
                        }
                    }
                    _ => {}
                }
            }
            for shape in output.shapes {
                collect(&shape.shape, &mut text);
            }
        }
        assert!(text.contains("PCBCraft updates"), "{text}");
        assert!(text.contains(&format!("Installed {VERSION}")), "{text}");
        if updater.snapshot().phase == Phase::Downloading {
            assert!(text.contains("Downloading  49%"), "{text}");
            assert!(text.contains("Cancel download"), "{text}");
        }
    }
}
