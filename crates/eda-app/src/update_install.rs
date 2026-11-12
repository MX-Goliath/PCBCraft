//! Durable hand-off to a copy of this executable, outside the install directory.
//! The GUI first completes its normal unsaved-work and settings-save paths.
use super::{Distribution, Package, Phase, State, VERSION, download_directory};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static RESTART: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
struct Pending {
    package: Package,
    path: PathBuf,
    digest: String,
    executable: PathBuf,
}

fn executable() -> Result<PathBuf, String> {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| std::env::current_exe().map_err(|e| e.to_string()))
}

fn directory() -> Result<PathBuf, String> {
    // Installed copies sharing a user cache must never update one another.
    let key = format!(
        "{:x}",
        Sha256::digest(executable()?.to_string_lossy().as_bytes())
    );
    Ok(download_directory()?.join(&key[..16]))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing parent")?)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, value).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(super) fn stage(
    package: &Package,
    path: &Path,
    cancelled: &AtomicBool,
) -> Result<bool, String> {
    let root = directory()?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("apply.lock"))
        .map_err(|e| e.to_string())?;
    // Hold the lock until the pending manifest is durably published.
    lock.lock().map_err(|e| e.to_string())?;
    if package.distribution == Distribution::WindowsPortable {
        let staging = tempfile::tempdir_in(&root).map_err(|e| e.to_string())?;
        extract_portable(path, staging.path())?;
    }
    let pending = Pending {
        package: package.clone(),
        path: path.into(),
        digest: hash_file(path)?,
        executable: executable()?,
    };
    if cancelled.load(Ordering::Relaxed) {
        return Ok(false);
    }
    write_json(&root.join("pending.json"), &pending)?;
    let _ = fs::remove_file(root.join("error.txt"));
    Ok(true)
}

fn read_pending(root: &Path) -> Result<Pending, String> {
    let file = File::open(root.join("pending.json")).map_err(|e| e.to_string())?;
    serde_json::from_reader(file.take(64 * 1024)).map_err(|e| e.to_string())
}

pub(super) fn restored_state() -> Option<State> {
    let root = directory().ok()?;
    let pending = read_pending(&root).ok()?;
    if pending.executable != executable().ok()? {
        return None;
    }
    let phase = fs::read_to_string(root.join("error.txt"))
        .map(Phase::Error)
        .unwrap_or(Phase::Ready);
    Some(State {
        phase,
        downloaded: pending.package.asset.size,
        package: Some(pending.package),
        pending_install: true,
        ..State::default()
    })
}

pub fn request_restart() {
    RESTART.store(true, Ordering::Relaxed);
    if let Ok(root) = directory() {
        let _ = fs::remove_file(root.join("error.txt"));
    }
}

pub fn cancel_restart() {
    RESTART.store(false, Ordering::Relaxed);
}

/// Returns true when startup was handed off to an update worker.
pub fn before_start() -> Result<bool, String> {
    let args: Vec<_> = std::env::args_os().collect();
    if args
        .get(1)
        .is_some_and(|arg| arg == "--pcbcraft-apply-update")
    {
        let root = args
            .get(2)
            .map(PathBuf::from)
            .ok_or("Missing update directory")?;
        let pid = args
            .get(3)
            .and_then(|arg| arg.to_str())
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or("Missing parent process")?;
        let restart = args.get(4).is_some_and(|arg| arg == "restart");
        run_worker(&root, pid, restart, &args[5..])?;
        return Ok(true);
    }
    let root = directory()?;
    if root.exists() {
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.join("apply.lock"))
            .map_err(|e| e.to_string())?;
        // A newly loaded Windows executable must exit rather than hold its
        // image mapped while another worker tries to replace it. Hand a quick
        // ordinary relaunch to a second cached worker, which waits on the lock.
        if lock.try_lock().is_err() {
            if root.join("pending.json").is_file() {
                spawn_worker(&root, true, &args[1..])?;
                return Ok(true);
            }
            lock.lock().map_err(|e| e.to_string())?;
        }
        drop(lock);
    }
    if !root.join("pending.json").is_file() || root.join("error.txt").exists() {
        return Ok(false);
    }
    let pending = read_pending(&root)?;
    if pending.executable != executable()? {
        return Err("Update belongs to another installation".into());
    }
    if pending
        .package
        .version
        .cmp_precedence(&semver::Version::parse(VERSION).map_err(|e| e.to_string())?)
        != std::cmp::Ordering::Greater
    {
        let _ = fs::remove_file(root.join("pending.json"));
        return Ok(false);
    }
    spawn_worker(&root, true, &args[1..])?;
    Ok(true)
}

pub fn after_exit() -> Result<(), String> {
    let root = directory()?;
    if root.join("pending.json").is_file() && !root.join("error.txt").exists() {
        spawn_worker(
            &root,
            RESTART.load(Ordering::Relaxed),
            &std::env::args_os().skip(1).collect::<Vec<_>>(),
        )?;
    }
    Ok(())
}

fn spawn_worker(root: &Path, restart: bool, args: &[std::ffi::OsString]) -> Result<(), String> {
    let worker_dir = tempfile::Builder::new()
        .prefix("worker-")
        .tempdir_in(root)
        .map_err(|e| e.to_string())?;
    let source = std::env::current_exe().map_err(|e| e.to_string())?;
    let worker = worker_dir.path().join(if cfg!(windows) {
        "pcbcraft-update.exe"
    } else {
        "pcbcraft-update"
    });
    write_json(
        &worker_dir.path().join("pending.json"),
        &read_pending(root)?,
    )?;
    fs::copy(&source, &worker).map_err(|e| format!("Cannot prepare update worker: {e}"))?;
    // Portable MSVC builds may rely on bundled runtime DLLs. Keep those beside
    // the worker so replacing the original application cannot unload them.
    if cfg!(windows)
        && let Some(parent) = source.parent()
    {
        for entry in fs::read_dir(parent).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            {
                fs::copy(
                    &path,
                    worker_dir
                        .path()
                        .join(path.file_name().ok_or("Invalid DLL")?),
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    let mut command = Command::new(&worker);
    command
        .arg("--pcbcraft-apply-update")
        .arg(root)
        .arg(std::process::id().to_string())
        .arg(if restart { "restart" } else { "exit" })
        .args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
        .current_dir(root)
        .env(
            "PCBCRAFT_UPDATE_WORKING_DIR",
            std::env::current_dir().map_err(|e| e.to_string())?,
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot start update worker: {e}"))?;
    let _ = worker_dir.keep();
    Ok(())
}

fn run_worker(
    root: &Path,
    pid: u32,
    restart: bool,
    args: &[std::ffi::OsString],
) -> Result<(), String> {
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("apply.lock"))
        .map_err(|e| e.to_string())?;
    // A simultaneous normal launch waits for the worker already replacing files.
    lock.lock().map_err(|e| e.to_string())?;
    let pending = read_pending(root).or_else(|_| {
        read_pending(
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .parent()
                .ok_or("Missing worker directory")?,
        )
    })?;
    let exited = wait_for_exit(pid);
    let can_restart = exited.is_ok();
    let result = exited.and_then(|()| {
        if root.join("pending.json").exists() {
            apply(root, &pending)
        } else {
            Ok(())
        }
    });
    if let Err(error) = &result {
        fs::write(root.join("error.txt"), format!("Update could not be installed: {error}. Your downloaded update is retained; retry from Updates."))
            .map_err(|e| e.to_string())?;
    } else {
        if root.join("pending.json").exists() {
            fs::remove_file(root.join("pending.json")).map_err(|e| e.to_string())?;
        }
        let _ = fs::remove_file(root.join("error.txt"));
    }
    drop(lock);
    if restart && can_restart {
        // Failure restarts the preserved old version, which displays error.txt.
        let working_directory = std::env::var_os("PCBCRAFT_UPDATE_WORKING_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .unwrap_or_else(|| pending.executable.parent().unwrap_or(root).to_path_buf());
        Command::new(&pending.executable)
            .args(args)
            .current_dir(working_directory)
            .env_remove("PCBCRAFT_UPDATE_WORKING_DIR")
            .env_remove("APPIMAGE")
            .env_remove("APPDIR")
            .spawn()
            .map_err(|e| format!("Cannot restart PCBCraft: {e}"))?;
    }
    result
}

#[cfg(windows)]
fn wait_for_exit(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    // SAFETY: OpenProcess owns this handle until CloseHandle, and no pointer is dereferenced.
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                Ok(())
            } else {
                Err(error.to_string())
            };
        }
        let status = WaitForSingleObject(handle, 120_000);
        CloseHandle(handle);
        if status == WAIT_OBJECT_0 {
            Ok(())
        } else {
            Err("PCBCraft did not finish exiting; no application files were replaced".into())
        }
    }
}

#[cfg(unix)]
fn wait_for_exit(pid: u32) -> Result<(), String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        // SAFETY: signal 0 only checks existence/permission; it sends no signal.
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            return if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err("Cannot wait for the old PCBCraft process".into())
            };
        }
        if std::time::Instant::now() >= deadline {
            return Err("PCBCraft did not finish exiting".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn apply(root: &Path, pending: &Pending) -> Result<(), String> {
    if hash_file(&pending.path)? != pending.digest {
        return Err("Downloaded package has changed (SHA-256 mismatch)".into());
    }
    let target = pending
        .executable
        .parent()
        .ok_or("Missing install directory")?;
    match pending.package.distribution {
        Distribution::WindowsPortable => {
            let staging = tempfile::tempdir_in(root).map_err(|e| e.to_string())?;
            extract_portable(&pending.path, staging.path())?;
            replace_portable(staging.path(), target, root)
        }
        Distribution::WindowsInstaller => run(Command::new(&pending.path)
            .args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/SP-",
                "/NORESTART",
                "/NOCLOSEAPPLICATIONS",
                "/NORESTARTAPPLICATIONS",
                "/RESTARTEXITCODE=3010",
            ])
            .arg(format!("/DIR={}", target.display()))
            .arg(format!("/LOG={}", root.join("installer.log").display()))),
        Distribution::Deb => run(Command::new("pkexec")
            .arg("/usr/bin/dpkg")
            .arg("--install")
            .arg(&pending.path)),
        Distribution::AppImage => replace_appimage(&pending.path, &pending.executable),
        Distribution::Mac => replace_mac(&pending.path, &pending.executable, root),
    }
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Installer exited with {status}"))
    }
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && !path.to_string_lossy().contains(':')
        && (cfg!(windows) || !path.to_string_lossy().contains('\\'))
}

fn extract_portable(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    if zip.len() > 20_000 {
        return Err("Too many files in update".into());
    }
    let mut total = 0u64;
    let mut root_name = None;
    let mut names = BTreeSet::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let path = PathBuf::from(entry.name());
        if entry.name().contains('\\')
            || !safe_relative(&path)
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Unsafe path or symlink in portable update".into());
        }
        let mut components = path.components();
        let first = components
            .next()
            .ok_or("Missing archive root")?
            .as_os_str()
            .to_owned();
        if root_name.as_ref().is_some_and(|root| root != &first) {
            return Err("Multiple archive roots".into());
        }
        root_name = Some(first);
        let relative = components.as_path();
        if relative.as_os_str().is_empty() {
            continue;
        }
        // Portable settings and markers are owned by this installation, never the update.
        let first = relative
            .components()
            .next()
            .ok_or("Missing entry name")?
            .as_os_str()
            .to_string_lossy();
        if first.eq_ignore_ascii_case("Data") || first.eq_ignore_ascii_case("portable.flag") {
            continue;
        }
        if !safe_relative(relative)
            || first.starts_with(".pcbcraft-")
            || first.eq_ignore_ascii_case("update-files.json")
        {
            return Err("Reserved update path".into());
        }
        // Windows device names, alternate streams and case aliases must be refused
        // even when the regression is run on Linux.
        for name in relative.components() {
            let name = name.as_os_str().to_string_lossy();
            let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
            if name.ends_with(['.', ' '])
                || name.contains(['<', '>', '"', '|', '?', '*'])
                || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
            {
                return Err("Invalid Windows update filename".into());
            }
        }
        let output = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|e| e.to_string())?;
            continue;
        }
        if !names.insert(relative.to_string_lossy().to_ascii_lowercase()) {
            return Err("Duplicate update path".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Update size overflow")?;
        if total > super::MAX_PACKAGE_SIZE {
            return Err("Unpacked update is too large".into());
        }
        fs::create_dir_all(output.parent().ok_or("Missing parent")?).map_err(|e| e.to_string())?;
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(output)
            .map_err(|e| e.to_string())?;
        let size = entry.size();
        let copied =
            std::io::copy(&mut entry.take(size + 1), &mut file).map_err(|e| e.to_string())?;
        if copied != size {
            return Err("Truncated ZIP entry".into());
        }
        file.sync_all().map_err(|e| e.to_string())?;
    }
    if !destination.join("pcbcraft.exe").is_file() {
        return Err("Portable update has no pcbcraft.exe".into());
    }
    Ok(())
}

fn files(root: &Path, relative: &Path, output: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    let path = root.join(relative);
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err(format!("Refusing symlink: {}", path.display()));
    }
    if metadata.is_file() {
        output.insert(relative.into());
    } else {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            files(
                root,
                &relative.join(entry.map_err(|e| e.to_string())?.file_name()),
                output,
            )?;
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Transaction {
    backup: PathBuf,
    old: BTreeSet<PathBuf>,
    new: BTreeSet<PathBuf>,
    committed: bool,
}

fn ensure_target_path(target: &Path, relative: &Path) -> Result<(), String> {
    if !safe_relative(relative) {
        return Err("Invalid managed file path".into());
    }
    let first = relative
        .components()
        .next()
        .ok_or("Empty path")?
        .as_os_str()
        .to_string_lossy();
    if first.eq_ignore_ascii_case("Data")
        || first.eq_ignore_ascii_case("portable.flag")
        || first.starts_with(".pcbcraft-")
    {
        return Err("Cannot replace portable user data".into());
    }
    let mut path = target.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && metadata.file_type().is_symlink()
        {
            return Err(format!("Refusing symlink: {}", path.display()));
        }
    }
    Ok(())
}

fn recover_transaction(target: &Path, root: &Path) -> Result<(), String> {
    let journal = root.join("transaction.json");
    if !journal.exists() {
        return Ok(());
    }
    let tx: Transaction = serde_json::from_reader(File::open(&journal).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if tx.backup.parent() != Some(target)
        || !tx
            .backup
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".pcbcraft-backup-"))
    {
        return Err("Invalid update recovery directory".into());
    }
    if !tx.committed {
        for relative in &tx.new {
            ensure_target_path(target, relative)?;
            if tx.backup.join(relative).exists() || !tx.old.contains(relative) {
                let path = target.join(relative);
                if path.is_file() {
                    fs::remove_file(path).map_err(|e| e.to_string())?;
                }
            }
        }
        for relative in &tx.old {
            ensure_target_path(target, relative)?;
            let backup = tx.backup.join(relative);
            if backup.is_file() {
                fs::rename(backup, target.join(relative)).map_err(|e| e.to_string())?;
            }
        }
    }
    if tx.backup.exists() {
        fs::remove_dir_all(&tx.backup).map_err(|e| e.to_string())?;
    }
    fs::remove_file(journal).map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_portable(payload: &Path, target: &Path, root: &Path) -> Result<(), String> {
    recover_transaction(target, root)?;
    let mut new = BTreeSet::new();
    files(payload, Path::new(""), &mut new)?;
    let mut old = BTreeSet::new();
    // Upgrade older distributions that predate a managed-file inventory.
    for name in [
        "pcbcraft.exe",
        "build-info.json",
        "README.txt",
        "Diagnose.cmd",
        "licenses",
        "update-files.json",
    ] {
        files(target, Path::new(name), &mut old)?;
    }
    if let Ok(file) = File::open(target.join("update-files.json")) {
        let previous: BTreeSet<PathBuf> =
            serde_json::from_reader(file).map_err(|e| e.to_string())?;
        for path in previous {
            ensure_target_path(target, &path)?;
            files(target, &path, &mut old)?;
        }
    }
    if let Ok(file) = File::open(target.join("build-info.json")) {
        let metadata: serde_json::Value =
            serde_json::from_reader(file).map_err(|e| e.to_string())?;
        if let Some(deps) = metadata.get("dependencies").and_then(|v| v.as_object()) {
            for name in deps.keys() {
                ensure_target_path(target, Path::new(name))?;
                files(target, Path::new(name), &mut old)?;
            }
        }
    }
    write_json(&payload.join("update-files.json"), &new)?;
    new.insert("update-files.json".into());
    // Include every overwritten file in rollback, including previously unknown files.
    for path in &new {
        ensure_target_path(target, path)?;
        files(target, path, &mut old)?;
    }
    let backup = tempfile::Builder::new()
        .prefix(".pcbcraft-backup-")
        .tempdir_in(target)
        .map_err(|e| e.to_string())?
        .keep();
    let mut tx = Transaction {
        backup,
        old,
        new,
        committed: false,
    };
    write_json(&root.join("transaction.json"), &tx)?;
    let result = (|| {
        for path in &tx.old {
            ensure_target_path(target, path)?;
            let backup = tx.backup.join(path);
            fs::create_dir_all(backup.parent().ok_or("Missing backup parent")?)
                .map_err(|e| e.to_string())?;
            fs::rename(target.join(path), backup).map_err(|e| e.to_string())?;
        }
        for path in &tx.new {
            let output = target.join(path);
            fs::create_dir_all(output.parent().ok_or("Missing parent")?)
                .map_err(|e| e.to_string())?;
            let mut file = File::create(output).map_err(|e| e.to_string())?;
            std::io::copy(
                &mut File::open(payload.join(path)).map_err(|e| e.to_string())?,
                &mut file,
            )
            .map_err(|e| e.to_string())?;
            file.flush().map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        tx.committed = true;
        write_json(&root.join("transaction.json"), &tx)
    })();
    // On failure restore every moved file; a persisted journal also handles
    // interruption halfway through replacement on the next update attempt.
    recover_transaction(target, root)?;
    result
}

fn replace_appimage(package: &Path, target: &Path) -> Result<(), String> {
    let mut staging =
        tempfile::NamedTempFile::new_in(target.parent().ok_or("Missing AppImage directory")?)
            .map_err(|e| e.to_string())?;
    std::io::copy(
        &mut File::open(package).map_err(|e| e.to_string())?,
        &mut staging,
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staging
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    staging.as_file().sync_all().map_err(|e| e.to_string())?;
    staging.persist(target).map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_mac(package: &Path, exe: &Path, root: &Path) -> Result<(), String> {
    let bundle = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("Executable is outside an app bundle")?;
    let mount = tempfile::tempdir_in(root).map_err(|e| e.to_string())?;
    run(Command::new("hdiutil")
        .arg("attach")
        .arg(package)
        .args(["-nobrowse", "-readonly", "-mountpoint"])
        .arg(mount.path()))?;
    let result = (|| {
        let staging =
            tempfile::tempdir_in(bundle.parent().ok_or("Missing Applications directory")?)
                .map_err(|e| e.to_string())?;
        let staged = staging.path().join("PCBCraft.app");
        run(Command::new("ditto")
            .arg(mount.path().join("PCBCraft.app"))
            .arg(&staged))?;
        run(Command::new("codesign")
            .args(["--verify", "--strict"])
            .arg(&staged))?;
        let backup = staging.path().join("previous.app");
        fs::rename(bundle, &backup).map_err(|e| e.to_string())?;
        if let Err(error) = fs::rename(staged, bundle) {
            if let Err(restore) = fs::rename(&backup, bundle) {
                let retained = staging.keep();
                return Err(format!(
                    "{error}; restore failed: {restore}. Previous app retained in {}",
                    retained.display()
                ));
            }
            return Err(error.to_string());
        }
        Ok(())
    })();
    let detached = run(Command::new("hdiutil").arg("detach").arg(mount.path()));
    result.and(detached)
}

#[cfg(test)]
#[path = "update_install_tests.rs"]
mod tests;
