//! Explicit local release fixtures for native GUI QA. Excluded from release builds.
//! Requires BOTH portable.flag and update-qa.flag next to the debug executable.
use super::*;

fn directory() -> Option<PathBuf> {
    if Distribution::current().ok()? != Distribution::WindowsPortable {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let root = exe.parent()?;
    let directory = root.join("UpdateQA");
    (root.join("update-qa.flag").is_file() && directory.is_dir()).then_some(directory)
}

fn milliseconds(directory: &Path, name: &str, default: u64, min: u64, max: u64) -> Duration {
    let value = std::fs::read_to_string(directory.join(name))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(default);
    Duration::from_millis(value.clamp(min, max))
}

pub(super) fn channel() -> Option<(ReleaseCheck, Download, Duration)> {
    let directory = directory()?;
    let interval = milliseconds(&directory, "interval-ms", 3_600_000, 250, 3_600_000);
    let check = Arc::new(move || {
        std::thread::sleep(milliseconds(&directory, "check-delay-ms", 0, 0, 5_000));
        let path = directory.join("release.json");
        if !path.exists() {
            return Ok((Phase::Unpublished, None));
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let release: Release = serde_json::from_slice(&read_limited(file, 2 * 1024 * 1024)?)
            .map_err(|e| format!("Invalid QA release metadata: {e}"))?;
        select_release(
            release,
            &Version::parse(VERSION).map_err(|e| e.to_string())?,
            Distribution::WindowsPortable,
            std::env::consts::ARCH,
            REPOSITORY,
        )
    });
    Some((check, download, interval))
}

fn download(
    package: &Package,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(u64, bool),
) -> Result<Option<PathBuf>, String> {
    struct SlowFile {
        file: std::fs::File,
        delay: Duration,
    }
    impl Read for SlowFile {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            let count = self.file.read(bytes)?;
            if count > 0 {
                std::thread::sleep(self.delay);
            }
            Ok(count)
        }
    }
    let directory = directory().ok_or("QA fixture was removed")?;
    let digest = parse_digest(
        package
            .asset
            .digest
            .as_deref()
            .ok_or("QA fixture needs SHA-256")?,
    )?;
    let reader = SlowFile {
        file: std::fs::File::open(directory.join(&package.asset.name))
            .map_err(|e| e.to_string())?,
        delay: milliseconds(&directory, "chunk-ms", 50, 0, 1_000),
    };
    let cache = download_directory()?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    // Use exactly the production stream verifier, staging and update worker.
    transfer(
        reader,
        &cache,
        &package.asset.name,
        package.asset.size,
        &digest,
        cancelled,
        progress,
    )
}
