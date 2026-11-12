//! Public GitHub Releases channel. Network and package I/O run off the UI thread.
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub const CONTROL_WIDTH: f32 = 158.0;
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
#[path = "update_install.rs"]
pub mod install;
#[cfg(debug_assertions)]
#[path = "updates_qa.rs"]
mod qa;
const REPOSITORY: &str = match option_env!("PCBCRAFT_UPDATE_REPOSITORY") {
    Some(repository) => repository,
    None => "MX-Goliath/PCBCraft",
};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_PACKAGE_SIZE: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    Paused,
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
    pending_install: bool,
    scheduler: Option<std::sync::mpsc::Sender<ScheduleEvent>>,
    shutdown: bool,
}

pub struct Updater {
    state: Arc<Mutex<State>>,
    cancelled: Arc<AtomicBool>,
    last_check: Option<Instant>,
    checking: Arc<AtomicBool>,
    restart: bool,
}

impl Default for Updater {
    fn default() -> Self {
        let state = install::restored_state().unwrap_or_default();
        Self {
            state: Arc::new(Mutex::new(state)),
            cancelled: Arc::default(),
            last_check: None,
            checking: Arc::default(),
            restart: false,
        }
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.shutdown = true;
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(send) = state.scheduler.take() {
            let _ = send.send(ScheduleEvent::Stop);
        }
    }
}

impl Updater {
    /// Arm once on the first frame. All subsequent work is independent of redraws.
    pub fn tick(&mut self, ctx: &egui::Context) {
        if self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .scheduler
            .is_some()
        {
            return;
        }
        let (check, download, interval) = channel();
        self.start_schedule(ctx, interval, check, download);
    }

    fn start_schedule(
        &mut self,
        ctx: &egui::Context,
        interval: Duration,
        check: ReleaseCheck,
        download: Download,
    ) {
        let (send, receive) = std::sync::mpsc::channel();
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.scheduler.is_some() || state.shutdown {
                return;
            }
            state.scheduler = Some(send);
        }
        let shared = self.state.clone();
        let checking = self.checking.clone();
        let cancelled = self.cancelled.clone();
        let ctx = ctx.clone();
        let mut last = self.last_check;
        std::thread::spawn(move || {
            loop {
                let state = shared.lock().unwrap_or_else(|p| p.into_inner()).clone();
                if state.shutdown {
                    break;
                }
                if state.phase == Phase::Available {
                    start_download(&shared, &cancelled, &checking, &ctx, download);
                }
                if check_due(last, Instant::now(), interval) {
                    let check = check.clone();
                    if start_check(&shared, &checking, &ctx, move || check()) {
                        last = Some(Instant::now());
                    }
                }
                let delay = last.map_or(interval, |last| interval.saturating_sub(last.elapsed()));
                let delay = if delay.is_zero() {
                    interval.min(Duration::from_secs(1))
                } else {
                    delay
                };
                match receive.recv_timeout(delay) {
                    Ok(ScheduleEvent::Stop)
                    | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(ScheduleEvent::Wake) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });
    }

    pub fn take_restart(&mut self) -> bool {
        std::mem::take(&mut self.restart)
    }

    fn snapshot(&self) -> State {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn check(&mut self, ctx: &egui::Context) {
        let (check, _, _) = channel();
        self.check_using(ctx, move || check());
    }

    fn check_using(
        &mut self,
        ctx: &egui::Context,
        check: impl FnOnce() -> Result<(Phase, Option<Package>), String> + Send + 'static,
    ) {
        if start_check(&self.state, &self.checking, ctx, check) {
            self.last_check = Some(Instant::now());
        }
    }

    fn download(&self, ctx: &egui::Context) {
        let (_, download, _) = channel();
        start_download(&self.state, &self.cancelled, &self.checking, ctx, download);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let state = self.snapshot();
        let downloading = matches!(state.phase, Phase::Downloading | Phase::Verifying);
        let progress = state
            .package
            .as_ref()
            .map_or(0., |p| state.downloaded as f32 / p.asset.size.max(1) as f32)
            .clamp(0., 1.);
        let label = match &state.phase {
            Phase::Idle => "Updates".into(),
            Phase::Checking => "Checking updates…".into(),
            Phase::Current => "Up to date".into(),
            Phase::Unpublished => "Updates".into(),
            Phase::Available => "Preparing download…".into(),
            Phase::Paused => "Download paused".into(),
            Phase::Downloading => format!("Downloading {:.0}%", progress * 100.),
            Phase::Verifying => "Preparing update…".into(),
            Phase::Ready => "Restart to update".into(),
            Phase::Error(_) => "Update issue".into(),
        };
        // Paint inside a fixed hit rectangle. Button's default minimum height
        // and title-bar padding must not extend below the 23-point title strip.
        let (hit, response) =
            ui.allocate_exact_size(egui::vec2(CONTROL_WIDTH - 6., 23.), egui::Sense::click());
        let rect = hit.shrink2(egui::vec2(1., 2.));
        let ready = state.phase == Phase::Ready;
        let accent = if ready || downloading {
            egui::Color32::from_rgb(115, 202, 242)
        } else {
            egui::Color32::from_gray(155)
        };
        let fill = if response.is_pointer_button_down_on() {
            39
        } else if response.hovered() {
            85
        } else {
            49
        };
        ui.painter().rect(
            rect,
            7.,
            egui::Color32::from_gray(fill),
            egui::Stroke::new(
                1.,
                if downloading {
                    egui::Color32::from_gray(105)
                } else {
                    accent
                },
            ),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &label,
            egui::FontId::proportional(11.5),
            egui::Color32::WHITE,
        );
        if downloading {
            let points = perimeter(rect.shrink(1.), 6.);
            let count = ((points.len() - 1) as f32 * progress).floor() as usize;
            let mut visible = points[..=count].to_vec();
            if count + 1 < points.len() {
                visible.push(points[count].lerp(
                    points[count + 1],
                    (points.len() - 1) as f32 * progress - count as f32,
                ));
            }
            ui.painter()
                .add(egui::Shape::line(visible, egui::Stroke::new(2., accent)));
        }
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        let response = response.on_hover_text(if ready {
            "Restart PCBCraft to install the downloaded update"
        } else {
            "Automatic updates · checked at startup and every hour"
        });
        if response.clicked() && ready {
            self.restart = true;
        }
        let mut restart = false;
        egui::Popup::menu(&response)
            .id(egui::Id::new("desktop_updates_popup"))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                crate::theme::dialog_contents(ui, 340., |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8., 8.);
                    ui.label(egui::RichText::new("PCBCraft updates").size(15.).strong());
                    ui.label(format!("Installed {VERSION} · Stable channel"));
                    ui.separator();
                    match &state.phase {
                        Phase::Idle | Phase::Available => { ui.label("Updates download automatically."); }
                        Phase::Paused => { ui.label("Download paused. Resume below or wait for the next hourly check."); }
                        Phase::Checking => { ui.horizontal(|ui| { ui.spinner(); ui.label("Checking for updates…"); }); }
                        Phase::Current => { ui.label("You’re up to date."); }
                        Phase::Unpublished => { ui.label("No public release is available. The repository may still be private."); }
                        Phase::Downloading | Phase::Verifying => {
                            ui.add(egui::ProgressBar::new(progress).show_percentage().desired_width(320.));
                            if let Some(p) = &state.package {
                                ui.label(format!("{:.1} / {:.1} MB", state.downloaded as f64 / 1e6, p.asset.size as f64 / 1e6));
                            }
                            if state.phase == Phase::Verifying { ui.label("Verifying and preparing installation…"); }
                            if ui.button("Pause download").clicked() { self.cancelled.store(true, Ordering::Relaxed); }
                        }
                        Phase::Ready => {
                            ui.label("Update downloaded and verified.");
                            ui.label("It will install when PCBCraft closes. You can also restart now.");
                            if ui.button("Restart to update").clicked() { restart = true; }
                        }
                        Phase::Error(error) => {
                            ui.colored_label(egui::Color32::from_rgb(255, 194, 176), error);
                            if state.pending_install && ui.button("Retry installation on restart").clicked() {
                                restart = true;
                            }
                        }
                    }
                    if let Some(package) = &state.package {
                        ui.label(format!("Version {}", package.version));
                        if matches!(state.phase, Phase::Paused | Phase::Error(_))
                            && ui.button("Retry download").clicked() { self.download(ui.ctx()); }
                        ui.hyperlink_to("Release notes", &package.release_url);
                    }
                    ui.separator();
                    ui.label("Checks at startup and every hour. Downloads are automatic.");
                    if ui.add_enabled(!state.phase.busy() && !self.checking.load(Ordering::Relaxed)
                        && state.phase != Phase::Ready, egui::Button::new("Check again")).clicked() {
                        self.check(ui.ctx());
                    }
                });
            });
        self.restart |= restart;
        ui.add_space(6.);
    }
}

enum ScheduleEvent {
    Wake,
    Stop,
}
type ReleaseCheck = Arc<dyn Fn() -> Result<(Phase, Option<Package>), String> + Send + Sync>;
type Download =
    fn(&Package, &AtomicBool, &mut dyn FnMut(u64, bool)) -> Result<Option<PathBuf>, String>;

fn channel() -> (ReleaseCheck, Download, Duration) {
    #[cfg(debug_assertions)]
    if let Some(channel) = qa::channel() {
        return channel;
    }
    (Arc::new(check_release), fetch_package, CHECK_INTERVAL)
}

fn wake_scheduler(shared: &Arc<Mutex<State>>) {
    let send = shared
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .scheduler
        .clone();
    if let Some(send) = send {
        let _ = send.send(ScheduleEvent::Wake);
    }
}

fn check_due(last: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last.is_none_or(|last| now.saturating_duration_since(last) >= interval)
}

fn start_check(
    shared: &Arc<Mutex<State>>,
    checking: &Arc<AtomicBool>,
    ctx: &egui::Context,
    check: impl FnOnce() -> Result<(Phase, Option<Package>), String> + Send + 'static,
) -> bool {
    {
        let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
        if state.shutdown || state.phase.busy() || checking.swap(true, Ordering::Relaxed) {
            return false;
        }
        if !state.pending_install {
            *state = State {
                phase: Phase::Checking,
                scheduler: state.scheduler.clone(),
                ..Default::default()
            };
        }
    }
    let shared = shared.clone();
    let checking = checking.clone();
    let ctx = ctx.clone();
    ctx.request_repaint();
    std::thread::spawn(move || {
        let result = check();
        let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
        match result {
            _ if state.pending_install => {}
            Ok((phase, package)) => {
                state.phase = phase;
                state.package = package;
            }
            Err(error) => state.phase = Phase::Error(error),
        }
        checking.store(false, Ordering::Relaxed);
        drop(state);
        ctx.request_repaint();
        wake_scheduler(&shared);
    });
    true
}

fn fetch_package(
    package: &Package,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(u64, bool),
) -> Result<Option<PathBuf>, String> {
    download_package(package, cancelled, progress)
}

fn start_download(
    shared: &Arc<Mutex<State>>,
    cancelled: &Arc<AtomicBool>,
    checking: &Arc<AtomicBool>,
    ctx: &egui::Context,
    download: Download,
) {
    let package = {
        let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
        if state.shutdown || state.phase.busy() || checking.load(Ordering::Relaxed) {
            return;
        }
        let Some(package) = state.package.clone() else {
            return;
        };
        state.phase = Phase::Downloading;
        state.downloaded = 0;
        cancelled.store(false, Ordering::Relaxed);
        package
    };
    let shared = shared.clone();
    let cancelled = cancelled.clone();
    let ctx = ctx.clone();
    ctx.request_repaint();
    std::thread::spawn(move || {
        let result = download(&package, &cancelled, &mut |downloaded, verifying| {
            let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
            state.downloaded = downloaded;
            if verifying {
                state.phase = Phase::Verifying;
            }
            ctx.request_repaint();
        });
        // Persist the verified package before advertising Restart to update.
        let result = result.and_then(|path| {
            if let Some(path) = &path {
                if cancelled.load(Ordering::Relaxed) {
                    return Ok(None);
                }
                if !install::stage(&package, path, &cancelled)? {
                    return Ok(None);
                }
            }
            Ok(path)
        });
        let mut state = shared.lock().unwrap_or_else(|p| p.into_inner());
        match result {
            Ok(Some(_)) => {
                state.phase = Phase::Ready;
                state.pending_install = true;
            }
            Ok(None) => state.phase = Phase::Paused,
            Err(error) => state.phase = Phase::Error(error),
        }
        drop(state);
        ctx.request_repaint();
        wake_scheduler(&shared);
    });
}

/// Clockwise rounded perimeter, starting at the top center.
fn perimeter(rect: egui::Rect, radius: f32) -> Vec<egui::Pos2> {
    let mut points = vec![egui::pos2(rect.center().x, rect.top())];
    for (center, start) in [
        (
            egui::pos2(rect.right() - radius, rect.top() + radius),
            -90.0_f32,
        ),
        (
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.,
        ),
        (
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            90.,
        ),
        (egui::pos2(rect.left() + radius, rect.top() + radius), 180.),
    ] {
        // Sample by arc length as well as straight edges for linear byte progress.
        let first = center + egui::Vec2::angled(start.to_radians()) * radius;
        let previous = *points.last().unwrap();
        let steps = (previous.distance(first) / 2.).ceil().max(1.) as usize;
        for i in 1..=steps {
            points.push(previous.lerp(first, i as f32 / steps as f32));
        }
        for i in 1..=8 {
            points.push(
                center + egui::Vec2::angled((start + 90. * i as f32 / 8.).to_radians()) * radius,
            );
        }
    }
    let previous = *points.last().unwrap();
    let end = points[0];
    let steps = (previous.distance(end) / 2.).ceil().max(1.) as usize;
    for i in 1..=steps {
        points.push(previous.lerp(end, i as f32 / steps as f32));
    }
    points
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
    // existing file. They remain in the user's cache for the restart hand-off.
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

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
