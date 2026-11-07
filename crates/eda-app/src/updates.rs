//! Public GitHub Releases channel. Network and disk work never run on the UI thread.
use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub const CONTROL_WIDTH: f32 = 142.0;
const REPOSITORY: &str = match option_env!("PCBCRAFT_UPDATE_REPOSITORY") {
    Some(repository) => repository,
    None => "MX-Goliath/PCBCraft",
};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_PACKAGE_SIZE: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Distribution {
    WindowsInstaller,
    WindowsPortable,
    Deb,
    AppImage,
    Mac,
}

impl Distribution {
    fn current() -> Result<Self, String> {
        Self::detect(
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("portable.flag")))
                .is_some_and(|p| p.is_file()),
            std::env::var_os("APPIMAGE").is_some(),
        )
    }

    fn detect(os: &str, arch: &str, portable: bool, appimage: bool) -> Result<Self, String> {
        match (os, arch) {
            ("windows", "x86_64") if portable => Ok(Self::WindowsPortable),
            ("windows", "x86_64") => Ok(Self::WindowsInstaller),
            ("linux", "x86_64") if appimage => Ok(Self::AppImage),
            ("linux", "x86_64") => Ok(Self::Deb),
            ("macos", "x86_64" | "aarch64") => Ok(Self::Mac),
            _ => Err(format!("No release package is configured for {os}/{arch}.")),
        }
    }

    fn filename(self, version: &Version, arch: &str) -> String {
        match self {
            Self::WindowsInstaller => format!("PCBCraft-{version}-windows-x64-setup.exe"),
            Self::WindowsPortable => format!("PCBCraft-{version}-windows-x64-portable.zip"),
            Self::Deb => format!("PCBCraft-{version}-linux-amd64.deb"),
            Self::AppImage => format!("PCBCraft-{version}-x86_64.AppImage"),
            Self::Mac => format!(
                "PCBCraft-{version}-macos-{}.dmg",
                if arch == "aarch64" { "arm64" } else { "x64" }
            ),
        }
    }

    fn instructions(self) -> &'static str {
        match self {
            Self::WindowsInstaller => {
                "Save your work and close PCBCraft before completing the installer."
            }
            Self::WindowsPortable => {
                "Close PCBCraft, extract the ZIP and replace the application files. Keep your existing Data folder and portable.flag."
            }
            Self::Deb => {
                "Open the DEB in your system package installer, or install it with sudo apt install /path/to/package.deb. Close PCBCraft first."
            }
            Self::AppImage => {
                "Close PCBCraft, replace your previous AppImage with the downloaded file and make it executable. Your settings are kept separately."
            }
            Self::Mac => {
                "Open the disk image, quit PCBCraft and drag PCBCraft.app to Applications, replacing the previous version."
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Clone, Debug)]
struct Package {
    version: Version,
    distribution: Distribution,
    asset: Asset,
    checksum: Option<Asset>,
    release_url: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Checking,
    Current,
    Unpublished,
    Available,
    Downloading,
    Verifying,
    Ready,
    Error(String),
}

impl Phase {
    fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading | Self::Verifying)
    }
}

#[derive(Clone, Default)]
struct State {
    phase: Phase,
    package: Option<Package>,
    downloaded: u64,
    path: Option<PathBuf>,
}

#[derive(Default)]
pub struct Updater {
    state: Arc<Mutex<State>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Updater {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Updater {
    fn snapshot(&self) -> State {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn check(&self, ctx: &egui::Context) {
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.phase.busy() {
                return;
            }
            *state = State {
                phase: Phase::Checking,
                ..Default::default()
            };
        }
        let shared = self.state.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = check_release();
            let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
            match result {
                Ok((phase, package)) => {
                    state.phase = phase;
                    state.package = package;
                }
                Err(error) => state.phase = Phase::Error(error),
            }
            ctx.request_repaint();
        });
    }

    fn download(&self, ctx: &egui::Context) {
        let package = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.phase.busy() {
                return;
            }
            let Some(package) = state.package.clone() else {
                return;
            };
            state.phase = Phase::Downloading;
            state.downloaded = 0;
            state.path = None;
            package
        };
        self.cancelled.store(false, Ordering::Relaxed);
        let shared = self.state.clone();
        let cancelled = self.cancelled.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = download_package(&package, &cancelled, |downloaded, verifying| {
                let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
                state.downloaded = downloaded;
                if verifying {
                    state.phase = Phase::Verifying;
                }
                ctx.request_repaint();
            });
            let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
            match result {
                Ok(Some(path)) => {
                    state.path = Some(path);
                    state.phase = Phase::Ready;
                }
                Ok(None) => state.phase = Phase::Available,
                Err(error) => state.phase = Phase::Error(error),
            }
            ctx.request_repaint();
        });
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        let state = self.snapshot();
        let downloading = matches!(state.phase, Phase::Downloading | Phase::Verifying);
        let progress = state
            .package
            .as_ref()
            .map_or(0., |p| state.downloaded as f32 / p.asset.size as f32)
            .clamp(0., 1.);
        let label = match &state.phase {
            Phase::Idle => "Check for updates".into(),
            Phase::Checking => "Checking…".into(),
            Phase::Current => "Up to date".into(),
            Phase::Unpublished => "No releases yet".into(),
            Phase::Available => "Update available".into(),
            Phase::Downloading => format!("Downloading {:3.0}%", progress * 100.),
            Phase::Verifying => "Verifying…".into(),
            Phase::Ready => "Update ready".into(),
            Phase::Error(_) => "Update unavailable".into(),
        };
        let accent = if matches!(state.phase, Phase::Available | Phase::Ready) {
            egui::Color32::from_rgb(181, 221, 190)
        } else {
            egui::Color32::from_gray(220)
        };
        let response = ui
            .add_sized(
                [CONTROL_WIDTH - 6., 20.],
                egui::Button::new(
                    egui::RichText::new(format!("↓  {label}"))
                        .size(11.)
                        .color(accent),
                )
                .fill(egui::Color32::from_gray(60))
                .corner_radius(9),
            )
            .on_hover_text("PCBCraft updates · stable GitHub Releases");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Check for updates")
        });
        if downloading {
            let track = egui::Rect::from_min_size(
                response.rect.left_bottom() - egui::vec2(0., 2.),
                egui::vec2(response.rect.width(), 2.),
            );
            ui.painter()
                .rect_filled(track, 1., egui::Color32::from_gray(90));
            ui.painter().rect_filled(
                egui::Rect::from_min_size(track.min, egui::vec2(track.width() * progress, 2.)),
                1.,
                accent,
            );
        }
        if response.clicked()
            && !state.phase.busy()
            && matches!(
                state.phase,
                Phase::Idle | Phase::Current | Phase::Unpublished | Phase::Error(_)
            )
        {
            self.check(ui.ctx());
        }
        egui::Popup::menu(&response)
            .id(egui::Id::new("desktop_updates_popup"))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(310.);
                // The title bar uses zero spacing; restore readable popup spacing.
                ui.spacing_mut().item_spacing = egui::vec2(8., 7.);
                ui.label(egui::RichText::new("PCBCraft updates").strong());
                ui.label(format!("Installed {VERSION} · Stable channel"));
                ui.separator();
                match &state.phase {
                    Phase::Idle => {
                        ui.label("Check GitHub Releases for a newer version.");
                    }
                    Phase::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Checking for updates…");
                        });
                    }
                    Phase::Current => {
                        ui.label("You’re up to date.");
                    }
                    Phase::Unpublished => {
                        ui.label(
                        "No public release is available yet. The repository may still be private.",
                    );
                    }
                    Phase::Available => {
                        ui.label("A new version is available.");
                    }
                    Phase::Downloading | Phase::Verifying => {
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .show_percentage()
                                .desired_width(300.),
                        );
                        if let Some(p) = &state.package {
                            ui.label(format!(
                                "{:.1} / {:.1} MB",
                                state.downloaded as f64 / 1e6,
                                p.asset.size as f64 / 1e6
                            ));
                        }
                        if state.phase == Phase::Verifying {
                            ui.label("Verifying SHA-256…");
                        }
                        if ui.button("Cancel download").clicked() {
                            self.cancelled.store(true, Ordering::Relaxed);
                        }
                    }
                    Phase::Ready => {
                        ui.label("Download complete · SHA-256 verified");
                    }
                    Phase::Error(error) => {
                        ui.colored_label(egui::Color32::from_rgb(244, 176, 164), error);
                    }
                }
                if let Some(package) = &state.package {
                    ui.label(format!(
                        "Version {} · {}",
                        package.version, package.asset.name
                    ));
                    if !state.phase.busy()
                        && state.phase != Phase::Ready
                        && ui.button("Download update").clicked()
                    {
                        self.download(ui.ctx());
                    }
                    if state.phase == Phase::Ready {
                        ui.label(package.distribution.instructions());
                        if let Some(path) = &state.path {
                            let open_folder = package.distribution == Distribution::AppImage;
                            if ui
                                .button(if open_folder {
                                    "Show downloaded AppImage"
                                } else {
                                    "Open downloaded package"
                                })
                                .clicked()
                                && let Err(error) = open_path(if open_folder {
                                    path.parent().unwrap_or(path)
                                } else {
                                    path
                                })
                            {
                                self.state.lock().unwrap_or_else(|p| p.into_inner()).phase =
                                    Phase::Error(error);
                            }
                            ui.label(path.display().to_string());
                        }
                    }
                    ui.hyperlink_to("Release notes", &package.release_url);
                }
                ui.separator();
                if ui
                    .add_enabled(!state.phase.busy(), egui::Button::new("Check again"))
                    .clicked()
                {
                    self.check(ui.ctx());
                }
                ui.hyperlink_to(
                    "All releases",
                    format!("https://github.com/{REPOSITORY}/releases"),
                );
            });
        ui.add_space(6.);
    }
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("PCBCraft/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("Cannot initialize the update connection: {e}"))
}

fn validate_repository(repository: &str) -> Result<(), String> {
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|s| {
            s.is_empty()
                || *s == "."
                || *s == ".."
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return Err("Invalid update repository; expected owner/repository.".into());
    }
    Ok(())
}

fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read the update response: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err("The update response exceeds its size limit.".into());
    }
    Ok(bytes)
}

fn http_error(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        403 | 429 => {
            "GitHub refused the update request or its rate limit was reached. Try again later."
                .into()
        }
        _ => format!("GitHub update request failed (HTTP {status})."),
    }
}

fn check_release() -> Result<(Phase, Option<Package>), String> {
    validate_repository(REPOSITORY)?;
    let distribution = Distribution::current()?;
    let response = client()?
        .get(format!(
            "https://api.github.com/repos/{REPOSITORY}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|e| format!("Cannot check for updates: {e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok((Phase::Unpublished, None));
    }
    if !response.status().is_success() {
        return Err(http_error(response.status()));
    }
    let release: Release = serde_json::from_slice(&read_limited(response, 2 * 1024 * 1024)?)
        .map_err(|e| format!("Invalid GitHub release metadata: {e}"))?;
    select_release(
        release,
        &Version::parse(VERSION).map_err(|e| e.to_string())?,
        distribution,
        std::env::consts::ARCH,
        REPOSITORY,
    )
}

fn select_release(
    release: Release,
    current: &Version,
    distribution: Distribution,
    arch: &str,
    repository: &str,
) -> Result<(Phase, Option<Package>), String> {
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| "Release tags must use semantic versions such as v0.2.0.".to_string())?;
    if release.draft || release.prerelease || !version.pre.is_empty() {
        return Ok((Phase::Unpublished, None));
    }
    if version.cmp_precedence(current) != std::cmp::Ordering::Greater {
        return Ok((Phase::Current, None));
    }
    let name = distribution.filename(&version, arch);
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .cloned()
        .ok_or_else(|| format!("Version {version} has no {name} package. See All releases."))?;
    if asset.size == 0 || asset.size > MAX_PACKAGE_SIZE {
        return Err("Invalid release package size.".into());
    }
    let checksum = release
        .assets
        .iter()
        .find(|a| a.name == format!("{name}.sha256"))
        .cloned();
    let prefix = format!(
        "https://github.com/{repository}/releases/download/{}/",
        release.tag_name
    );
    for file in std::iter::once(&asset).chain(checksum.iter()) {
        if file.browser_download_url != format!("{prefix}{}", file.name) {
            return Err("Release asset URL does not match the configured repository.".into());
        }
    }
    if asset.digest.is_none() && checksum.is_none() {
        return Err("This release has no SHA-256 checksum. Download refused.".into());
    }
    if let Some(digest) = &asset.digest {
        parse_digest(digest)?;
    }
    Ok((
        Phase::Available,
        Some(Package {
            version,
            distribution,
            asset,
            checksum,
            release_url: format!(
                "https://github.com/{repository}/releases/tag/{}",
                release.tag_name
            ),
        }),
    ))
}

fn parse_digest(digest: &str) -> Result<String, String> {
    let digest = digest.strip_prefix("sha256:").unwrap_or(digest);
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid SHA-256 checksum in the release.".into());
    }
    Ok(digest.to_ascii_lowercase())
}

fn checksum_contents(bytes: &[u8], filename: &str) -> Result<String, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| "Invalid checksum file encoding.".to_string())?;
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 2 || fields[1].trim_start_matches('*') != filename {
        return Err("Checksum file does not name the selected package.".into());
    }
    parse_digest(fields[0])
}

fn download_directory() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("portable.flag").is_file()
    {
        return Ok(dir.join("Data/Updates"));
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|p| Path::new(p).is_absolute())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache")))
    };
    base.map(|p| p.join("PCBCraft/updates"))
        .ok_or_else(|| "Cannot locate the update download folder.".into())
}

fn download_package(
    package: &Package,
    cancelled: &AtomicBool,
    progress: impl FnMut(u64, bool),
) -> Result<Option<PathBuf>, String> {
    let client = client()?;
    let digest = if let Some(digest) = &package.asset.digest {
        parse_digest(digest)?
    } else {
        let checksum = package
            .checksum
            .as_ref()
            .ok_or("Missing release checksum.")?;
        let response = client
            .get(&checksum.browser_download_url)
            .send()
            .map_err(|e| format!("Cannot download checksum: {e}"))?;
        if !response.status().is_success() {
            return Err(http_error(response.status()));
        }
        checksum_contents(&read_limited(response, 4096)?, &package.asset.name)?
    };
    if cancelled.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let response = client
        .get(&package.asset.browser_download_url)
        .timeout(Duration::from_secs(60 * 60))
        .send()
        .map_err(|e| format!("Cannot download update: {e}"))?;
    if !response.status().is_success() {
        return Err(http_error(response.status()));
    }
    if response
        .content_length()
        .is_some_and(|n| n != package.asset.size)
    {
        return Err("Download size disagrees with the release metadata.".into());
    }
    let directory = download_directory()?;
    std::fs::create_dir_all(&directory)
        .map_err(|e| format!("Cannot create download folder: {e}"))?;
    transfer(
        response,
        &directory,
        &package.asset.name,
        package.asset.size,
        &digest,
        cancelled,
        progress,
    )
}

fn transfer(
    mut reader: impl Read,
    directory: &Path,
    filename: &str,
    size: u64,
    digest: &str,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(u64, bool),
) -> Result<Option<PathBuf>, String> {
    // A private temporary file is removed on error/cancel. Only verified bytes
    // become a package; a symlink or previous download is never overwritten.
    let mut file = tempfile::Builder::new()
        .prefix(".pcbcraft-")
        .suffix(".part")
        .tempfile_in(directory)
        .map_err(|e| format!("Cannot create update download: {e}"))?;
    let mut hash = Sha256::new();
    let mut downloaded = 0;
    let mut buffer = [0; 64 * 1024];
    let mut last = Instant::now();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let count = reader
            .read(&mut buffer)
            .map_err(|e| format!("Download interrupted: {e}"))?;
        if count == 0 {
            break;
        }
        downloaded += count as u64;
        if downloaded > size {
            return Err("Download exceeds the expected package size.".into());
        }
        file.write_all(&buffer[..count])
            .map_err(|e| format!("Cannot save update (check free disk space): {e}"))?;
        hash.update(&buffer[..count]);
        if last.elapsed() >= Duration::from_millis(100) {
            progress(downloaded, false);
            last = Instant::now();
        }
    }
    if downloaded != size {
        return Err("Incomplete update download. Please retry.".into());
    }
    progress(downloaded, true);
    if format!("{:x}", hash.finalize()) != digest {
        return Err("SHA-256 verification failed. The update was discarded.".into());
    }
    if cancelled.load(Ordering::Relaxed) {
        return Ok(None);
    }
    file.as_file()
        .sync_all()
        .map_err(|e| format!("Cannot flush update to disk: {e}"))?;
    // Separate directories allow safe retries without deleting or executing an
    // existing file. They remain in the user's cache for manual installation.
    let destination = tempfile::Builder::new()
        .prefix("release-")
        .tempdir_in(directory)
        .map_err(|e| format!("Cannot stage update: {e}"))?;
    let path = destination.path().join(filename);
    file.persist_noclobber(&path)
        .map_err(|e| format!("Cannot finish update download: {e}"))?;
    let _ = destination.keep();
    Ok(Some(path))
}

fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path).spawn().map(|_| ()).map_err(|e| {
        format!("Cannot open the downloaded package: {e}. Open it from its displayed path.")
    })
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
