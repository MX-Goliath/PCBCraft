//! Project-scoped Git access. No shell, checkout, reset of working files or force push.
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const LIMIT: u64 = 256 * 1024 * 1024;

pub fn supported(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|s| {
        ["schdoc", "pcbdoc", "schlib", "pcblib", "prjpcb", "dsnwrk"]
            .iter()
            .any(|e| s.eq_ignore_ascii_case(e))
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
}
impl File {
    pub fn changed(&self) -> bool {
        !self.status.trim().is_empty()
    }
    pub fn conflicted(&self) -> bool {
        self.status.contains('U') || matches!(self.status.as_str(), "AA" | "DD")
    }
}
#[derive(Clone, Debug)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub body: String,
    pub refs: String,
}
#[derive(Clone, Debug)]
pub struct Repository {
    pub root: PathBuf,
    /// Repository-relative directory; never a shell/glob pathspec.
    pub scope: String,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub repository: Repository,
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub head: Option<String>,
    pub files: Vec<File>,
    pub commits: Vec<Commit>,
    pub refs: Vec<String>,
    pub remotes: Vec<String>,
    pub operation: Option<String>,
}

fn execute(mut command: Command) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot run Git (install Git and make it available on PATH): {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |stream: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            stream
                .take(LIMIT + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if started.elapsed() > Duration::from_secs(120) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "Git timed out after 120 seconds; check network and configured credentials"
                        .into(),
                );
            }
            None => thread::sleep(Duration::from_millis(25)),
        }
    };
    let bytes = out
        .join()
        .map_err(|_| "Git output reader stopped")?
        .map_err(|e| e.to_string())?;
    let error = err
        .join()
        .map_err(|_| "Git error reader stopped")?
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT || error.len() as u64 > LIMIT {
        return Err("Git output exceeds 256 MiB".into());
    }
    if !status.success() {
        return Err(String::from_utf8_lossy(&error).trim().to_owned());
    }
    Ok(bytes)
}

impl Repository {
    pub fn discover(directory: &Path) -> Result<Self, String> {
        let directory = directory.canonicalize().map_err(|e| e.to_string())?;
        let directory = if directory.is_file() {
            directory
                .parent()
                .ok_or("Project file has no parent directory")?
                .to_path_buf()
        } else {
            directory
        };
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&directory)
            .args(["rev-parse", "--show-toplevel"]);
        let root = PathBuf::from(
            String::from_utf8(execute(command)?)
                .map_err(|_| "Non-UTF8 Git root")?
                .trim(),
        );
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let scope = directory
            .strip_prefix(&root)
            .map_err(|_| "Project is outside repository")?
            .to_string_lossy()
            .replace('\\', "/");
        Ok(Self { root, scope })
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .arg("--literal-pathspecs")
            .arg("-C")
            .arg(&self.root)
            .args(args);
        command
    }
    fn run(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        execute(self.command(args))
    }
    fn text(&self, args: &[&str]) -> Result<String, String> {
        String::from_utf8(self.run(args)?)
            .map(|s| s.trim_end().to_owned())
            .map_err(|_| "Non-UTF8 Git output is not supported".into())
    }
    fn scope_arg(&self) -> &str {
        if self.scope.is_empty() {
            "."
        } else {
            &self.scope
        }
    }
    fn check_path(&self, path: &str) -> Result<(), String> {
        let p = Path::new(path);
        if path.is_empty()
            || p.is_absolute()
            || p.components().any(|c| !matches!(c, Component::Normal(_)))
            || path.contains('\\')
            || path.contains(':')
            || !supported(path)
            || (!self.scope.is_empty() && !path.starts_with(&format!("{}/", self.scope)))
        {
            return Err(
                "Only supported Altium files inside this project directory may be selected".into(),
            );
        }
        Ok(())
    }
    fn scoped_file(&self, mut file: File) -> Option<File> {
        let new_supported = self.check_path(&file.path).is_ok();
        let old_supported = file
            .old_path
            .as_ref()
            .is_some_and(|p| self.check_path(p).is_ok());
        if new_supported {
            if file.old_path.is_some() && !old_supported {
                file.old_path = None;
                file.status = "A".into();
            }
            Some(file)
        } else if old_supported {
            Some(File {
                path: file.old_path.take().unwrap(),
                old_path: None,
                status: "D".into(),
            })
        } else {
            None
        }
    }
    fn oid(&self, revision: &str) -> Result<String, String> {
        if revision.starts_with('-') || revision.contains(['\0', '\n', '\r']) {
            return Err("Invalid revision".into());
        }
        self.text(&[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{revision}^{{commit}}"),
        ])
    }
    pub fn snapshot(&self, reference: &str, count: usize) -> Result<Snapshot, String> {
        let head = self.oid("HEAD").ok();
        let branch = self
            .text(&["symbolic-ref", "--quiet", "--short", "HEAD"])
            .unwrap_or_else(|_| "Detached HEAD".into());
        let upstream = self
            .text(&["rev-parse", "--abbrev-ref", "@{upstream}"])
            .ok();
        let counts = upstream
            .as_ref()
            .and_then(|_| {
                self.text(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
                    .ok()
            })
            .unwrap_or_default();
        let mut counts = counts.split_whitespace().filter_map(|n| n.parse().ok());
        let mut files: BTreeMap<String, File> = nul_strings(&self.run(&[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            self.scope_arg(),
        ])?)?
        .into_iter()
        .filter(|p| supported(p))
        .map(|path| {
            (
                path.clone(),
                File {
                    path,
                    old_path: None,
                    status: String::new(),
                },
            )
        })
        .collect();
        // Read the entire status before filtering: rename pairs must stay together.
        for file in parse_status(&self.run(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ])?)? {
            if let Some(file) = self.scoped_file(file) {
                files.insert(file.path.clone(), file);
            }
        }
        let refs = self
            .text(&[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ])?
            .lines()
            .filter(|s| !s.ends_with("/HEAD"))
            .map(str::to_owned)
            .collect();
        let gitdir = PathBuf::from(self.text(&["rev-parse", "--absolute-git-dir"])?);
        let remotes = self.text(&["remote"])?.lines().map(str::to_owned).collect();
        let operation = [
            ("MERGE_HEAD", "Merge in progress"),
            ("rebase-merge", "Rebase in progress"),
            ("rebase-apply", "Rebase in progress"),
            ("CHERRY_PICK_HEAD", "Cherry-pick in progress"),
        ]
        .into_iter()
        .find(|(p, _)| gitdir.join(p).exists())
        .map(|(_, label)| label.to_owned());
        let commits = if head.is_some() {
            self.history(reference, count)?
        } else {
            vec![]
        };
        Ok(Snapshot {
            repository: self.clone(),
            branch,
            upstream,
            ahead: counts.next().unwrap_or(0),
            behind: counts.next().unwrap_or(0),
            head,
            files: files.into_values().collect(),
            commits,
            refs,
            remotes,
            operation,
        })
    }
    pub fn history(&self, reference: &str, count: usize) -> Result<Vec<Commit>, String> {
        self.log(reference, count, None)
    }
    pub fn file_history(
        &self,
        reference: &str,
        count: usize,
        path: &str,
    ) -> Result<Vec<Commit>, String> {
        self.check_path(path)?;
        self.log(reference, count, Some(path))
    }
    fn log(
        &self,
        reference: &str,
        count: usize,
        path: Option<&str>,
    ) -> Result<Vec<Commit>, String> {
        let oid = self.oid(reference)?;
        let count = format!("--max-count={}", count.clamp(1, 10000));
        let mut args = vec![
            "log",
            "-z",
            &count,
            "--format=%H%x00%P%x00%an%x00%aI%x00%s%x00%b%x00%D",
        ];
        if path.is_some() {
            args.push("--follow");
        }
        args.extend([&oid, "--", path.unwrap_or(self.scope_arg())]);
        let data = self.run(&args)?;
        let chunks = nul_strings(&data)?;
        if !chunks.len().is_multiple_of(7) {
            return Err("Unexpected Git log format".into());
        }
        Ok(chunks
            .chunks_exact(7)
            .map(|c| Commit {
                id: c[0].clone(),
                parents: c[1].split_whitespace().map(str::to_owned).collect(),
                author: c[2].clone(),
                date: c[3].clone(),
                subject: c[4].clone(),
                body: c[5].clone(),
                refs: c[6].clone(),
            })
            .collect())
    }
    pub fn commit_files(&self, revision: &str, parent: Option<&str>) -> Result<Vec<File>, String> {
        let id = self.oid(revision)?;
        let bytes = if let Some(parent) = parent {
            let parent = self.oid(parent)?;
            self.run(&[
                "diff",
                "--name-status",
                "-z",
                "--find-renames",
                &parent,
                &id,
                "--",
                self.scope_arg(),
            ])?
        } else {
            self.run(&[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "-r",
                "--name-status",
                "-z",
                &id,
                "--",
                self.scope_arg(),
            ])?
        };
        let parts = nul_strings(&bytes)?;
        let mut files = Vec::new();
        let mut i = 0;
        while i < parts.len() {
            let status = parts[i].clone();
            i += 1;
            let old = parts.get(i).ok_or("Truncated Git change")?.clone();
            i += 1;
            let rename = status.starts_with(['R', 'C']);
            let path = if rename {
                let p = parts.get(i).ok_or("Truncated Git rename")?.clone();
                i += 1;
                p
            } else {
                old.clone()
            };
            if let Some(file) = self.scoped_file(File {
                path,
                old_path: rename.then_some(old),
                status,
            }) {
                files.push(file);
            }
        }
        Ok(files)
    }
    pub fn blob(&self, revision: &str, path: &str) -> Result<Option<Vec<u8>>, String> {
        self.check_path(path)?;
        let oid = self.oid(revision)?;
        let list = self.run(&["ls-tree", "-z", &oid, "--", path])?;
        if list.is_empty() {
            return Ok(None);
        }
        let header = list
            .split(|b| *b == b'\t')
            .next()
            .ok_or("Invalid Git tree")?;
        let header = std::str::from_utf8(header).map_err(|_| "Invalid Git tree")?;
        let mut header = header.split_whitespace();
        let mode = header.next();
        let kind = header.next();
        let object = header.next().ok_or("Invalid Git object")?;
        if !matches!(mode, Some("100644" | "100755")) || kind != Some("blob") {
            return Err("Symlinks and submodules are not Altium documents".into());
        }
        let size: u64 = self
            .text(&["cat-file", "-s", object])?
            .parse()
            .map_err(|_| "Invalid blob size")?;
        if size > LIMIT {
            return Err("Git document exceeds 256 MiB".into());
        }
        self.run(&["cat-file", "blob", object]).map(Some)
    }
    pub fn working_blob(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        self.check_path(path)?;
        let p = self.root.join(path);
        if !p.exists() {
            return Ok(None);
        }
        let canonical = p.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(self.root.join(&self.scope))
            || std::fs::symlink_metadata(&p)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
        {
            return Err("Document symlink leaves project scope".into());
        }
        crate::altium::read_bounded(&p).map(Some)
    }
    /// A private index prevents unrelated staged files leaking into a commit.
    pub fn commit(
        &self,
        paths: &[String],
        message: &str,
        expected_head: Option<&str>,
    ) -> Result<String, String> {
        if paths.is_empty() || message.trim().is_empty() {
            return Err("Select changed files and enter a commit message".into());
        }
        for p in paths {
            self.check_path(p)?;
            let absolute = self.root.join(p);
            if absolute.exists()
                && (!absolute
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(self.root.join(&self.scope))
                    || std::fs::symlink_metadata(&absolute)
                        .map_err(|e| e.to_string())?
                        .file_type()
                        .is_symlink())
            {
                return Err("Document symlinks cannot be committed from the Git panel".into());
            }
        }
        let snapshot = self.snapshot("HEAD", 1)?;
        if snapshot.head.as_deref() != expected_head {
            return Err("HEAD changed; refresh and review the selection".into());
        }
        if snapshot.operation.is_some() {
            return Err("Finish the current merge/rebase/cherry-pick before committing".into());
        }
        if snapshot.files.iter().any(File::conflicted)
            || !self.run(&["ls-files", "--unmerged", "-z"])?.is_empty()
        {
            return Err("Resolve repository conflicts before committing".into());
        }
        let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let index = temp.path().join("index");
        let run = |args: &[&str]| {
            let mut command = self.command(args);
            command.env("GIT_INDEX_FILE", &index);
            execute(command)
        };
        if let Some(head) = &snapshot.head {
            run(&["read-tree", head])?;
        } else {
            run(&["read-tree", "--empty"])?;
        }
        let mut args = vec!["add", "-A", "--"];
        args.extend(paths.iter().map(String::as_str));
        run(&args)?;
        let message_file = temp.path().join("message");
        std::fs::write(&message_file, message).map_err(|e| e.to_string())?;
        let mut command = self.command(&["commit", "--file"]);
        if self.oid("HEAD").ok().as_deref() != expected_head {
            return Err("HEAD changed while preparing the commit; refresh and review again".into());
        }
        command.arg(&message_file).env("GIT_INDEX_FILE", &index);
        execute(command)?;
        // Reconcile only the committed paths in the user's index, keeping all other staging.
        let mut reset = self.command(&["reset", "--quiet", "HEAD", "--"]);
        reset.args(paths);
        let id = self.oid("HEAD")?;
        execute(reset).map_err(|e| {
            format!("Commit {id} succeeded, but refreshing the selected index paths failed: {e}")
        })?;
        Ok(id)
    }
    pub fn fetch(&self) -> Result<String, String> {
        self.text(&["fetch", "--all"])
    }
    pub fn push(&self) -> Result<String, String> {
        let branch = self
            .text(&["symbolic-ref", "--quiet", "--short", "HEAD"])
            .map_err(|_| "Cannot push a detached HEAD")?;
        let remote = self
            .text(&["config", "--get", &format!("branch.{branch}.remote")])
            .map_err(|_| "No upstream configured. Set the branch upstream in Git, then refresh.")?;
        let destination = self.text(&["config", "--get", &format!("branch.{branch}.merge")])?;
        if remote.starts_with('-') || !destination.starts_with("refs/heads/") {
            return Err("Invalid push upstream".into());
        }
        self.text(&[
            "push",
            "--porcelain",
            &remote,
            &format!("HEAD:{destination}"),
        ])
    }
    pub fn first_push(&self, remote: &str, branch: &str) -> Result<String, String> {
        if !self.text(&["remote"])?.lines().any(|r| r == remote) {
            return Err("Select an existing repository remote".into());
        }
        self.text(&["symbolic-ref", "--quiet", "HEAD"])
            .map_err(|_| "Cannot push a detached HEAD")?;
        self.run(&["check-ref-format", &format!("refs/heads/{branch}")])?;
        self.text(&[
            "push",
            "--porcelain",
            "--set-upstream",
            remote,
            &format!("HEAD:refs/heads/{branch}"),
        ])
    }
}
fn nul_strings(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut parts: Vec<_> = bytes.split(|b| *b == 0).collect();
    if parts.last() == Some(&&b""[..]) {
        parts.pop();
    }
    parts
        .into_iter()
        .map(|b| {
            String::from_utf8(b.to_vec())
                .map_err(|_| "Non-UTF8 file paths are not supported".into())
        })
        .collect()
}
fn parse_status(bytes: &[u8]) -> Result<Vec<File>, String> {
    let parts = nul_strings(bytes)?;
    let mut files = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let entry = &parts[i];
        i += 1;
        if entry.len() < 4 || !entry.is_char_boundary(3) {
            return Err("Invalid Git status".into());
        }
        let status = entry[..2].to_owned();
        let old_path = if status.contains(['R', 'C']) {
            let old = parts.get(i).ok_or("Truncated status rename")?.clone();
            i += 1;
            Some(old)
        } else {
            None
        };
        files.push(File {
            path: entry[3..].into(),
            old_path,
            status,
        });
    }
    Ok(files)
}

pub fn import_document(bytes: &[u8], path: &str) -> Result<eda_core::Document, String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "schdoc" => crate::altium::import_schematic(bytes, path),
        "pcbdoc" => crate::altium_pcb::import_pcb(bytes, path),
        "schlib" | "pcblib" => crate::library_documents::import(bytes, path, ext == "pcblib"),
        _ => Err("This project file has a text diff; it is not a SCH/PCB scene".into()),
    }
}

pub fn project_text(bytes: &[u8]) -> Result<String, String> {
    crate::project_files::decode(bytes)
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
