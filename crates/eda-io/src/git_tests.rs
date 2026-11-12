use super::*;
fn git(dir: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn repo() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Git Panel Test"]);
    git(
        dir.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    let repo = Repository::discover(dir.path()).unwrap();
    (dir, repo)
}
#[test]
fn selected_commit_preserves_unrelated_staging_and_browses_root_rename_delete() {
    let (dir, r) = repo();
    std::fs::write(dir.path().join("board.PcbDoc"), "old").unwrap();
    std::fs::write(dir.path().join("secret.txt"), "unrelated").unwrap();
    git(dir.path(), &["add", "secret.txt"]);
    let first = r.commit(&["board.PcbDoc".into()], "First", None).unwrap();
    assert_eq!(
        git(dir.path(), &["diff", "--cached", "--name-only"]),
        "secret.txt"
    );
    assert_eq!(r.snapshot("HEAD", 100).unwrap().commits[0].subject, "First");
    assert_eq!(
        r.commit_files(&first, None).unwrap()[0].path,
        "board.PcbDoc"
    );
    assert_eq!(r.blob(&first, "board.PcbDoc").unwrap().unwrap(), b"old");
    assert!(r.blob(&first, "missing.SchDoc").unwrap().is_none());
    std::fs::rename(
        dir.path().join("board.PcbDoc"),
        dir.path().join("new name.PcbDoc"),
    )
    .unwrap();
    let second = r
        .commit(
            &["board.PcbDoc".into(), "new name.PcbDoc".into()],
            "Rename",
            Some(&first),
        )
        .unwrap();
    let files = r.commit_files(&second, Some(&first)).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].old_path.as_deref(), Some("board.PcbDoc"));
    std::fs::remove_file(dir.path().join("new name.PcbDoc")).unwrap();
    let third = r
        .commit(&["new name.PcbDoc".into()], "Delete", Some(&second))
        .unwrap();
    assert_eq!(
        r.commit_files(&third, Some(&second)).unwrap()[0].status,
        "D"
    );
    assert!(
        r.commit(&["secret.txt".into()], "No", Some(&third))
            .is_err()
    );
}
#[test]
fn paths_scope_conflicts_and_failed_commit_are_safe() {
    let (dir, r) = repo();
    std::fs::create_dir(dir.path().join("project")).unwrap();
    let scoped = Repository::discover(&dir.path().join("project")).unwrap();
    assert!(scoped.check_path("other.SchDoc").is_err());
    assert!(r.check_path("../evil.SchDoc").is_err());
    assert!(r.check_path(":(glob)*.SchDoc").is_err());
    std::fs::write(dir.path().join("project/test.SchDoc"), b"a").unwrap();
    let first = scoped
        .commit(&["project/test.SchDoc".into()], "One", None)
        .unwrap();
    assert_eq!(scoped.snapshot("HEAD", 100).unwrap().files.len(), 1);
    std::fs::write(dir.path().join("project/test.SchDoc"), b"b").unwrap();
    let index_before = git(dir.path(), &["ls-files", "--stage"]);
    assert!(
        scoped
            .commit(&["project/test.SchDoc".into()], "stale", None)
            .is_err()
    );
    assert_eq!(git(dir.path(), &["ls-files", "--stage"]), index_before);
    assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), first);
}
#[test]
fn local_push_uses_exact_upstream_and_never_force() {
    let (dir, r) = repo();
    let remote = tempfile::tempdir().unwrap();
    git(remote.path(), &["init", "--bare"]);
    std::fs::write(dir.path().join("test.SchDoc"), b"a").unwrap();
    r.commit(&["test.SchDoc".into()], "One", None).unwrap();
    git(
        dir.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    assert!(r.push().is_err());
    r.first_push("origin", "main").unwrap();
    std::fs::write(dir.path().join("test.SchDoc"), b"b").unwrap();
    let head = r.oid("HEAD").unwrap();
    let second = r
        .commit(&["test.SchDoc".into()], "Two", Some(&head))
        .unwrap();
    r.push().unwrap();
    assert_eq!(
        git(remote.path(), &["rev-parse", "refs/heads/main"]),
        second
    );
    assert!(r.first_push("not-a-remote", "main").is_err());
    assert!(r.first_push("origin", "../invalid").is_err());
}

fn pull_fixture() -> (
    tempfile::TempDir,
    Repository,
    tempfile::TempDir,
    tempfile::TempDir,
    Repository,
) {
    let (dir, repo) = repo();
    let remote = tempfile::tempdir().unwrap();
    git(remote.path(), &["init", "--bare"]);
    std::fs::write(dir.path().join("test.SchDoc"), b"a").unwrap();
    repo.commit(&["test.SchDoc".into()], "Initial", None)
        .unwrap();
    git(
        dir.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    repo.first_push("origin", "main").unwrap();
    let (peer_dir, peer) = self::repo();
    git(
        peer_dir.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    peer.fetch().unwrap();
    git(peer_dir.path(), &["checkout", "-B", "main", "origin/main"]);
    git(
        peer_dir.path(),
        &["branch", "--set-upstream-to=origin/main"],
    );
    (dir, repo, remote, peer_dir, peer)
}

#[test]
fn pull_fetches_and_fast_forwards_whole_repository_without_merge_commits() {
    let (dir, repo, _remote, peer_dir, peer) = pull_fixture();
    std::fs::create_dir(dir.path().join("project")).unwrap();
    let scoped = Repository::discover(&dir.path().join("project")).unwrap();
    let head = repo.oid("HEAD").unwrap();
    std::fs::write(peer_dir.path().join("test.SchDoc"), b"remote").unwrap();
    std::fs::write(peer_dir.path().join("outside.txt"), b"outside filter").unwrap();
    git(peer_dir.path(), &["add", "."]);
    git(peer_dir.path(), &["commit", "-m", "Remote update"]);
    peer.push().unwrap();
    // Configured rebase/autostash must not change the panel's semantics.
    git(dir.path(), &["config", "pull.rebase", "true"]);
    git(dir.path(), &["config", "merge.autostash", "true"]);
    let paths = scoped.pull(&head).unwrap();
    assert_eq!(paths, ["outside.txt", "test.SchDoc"]);
    assert_eq!(repo.oid("HEAD").unwrap(), peer.oid("HEAD").unwrap());
    assert_eq!(
        std::fs::read(dir.path().join("test.SchDoc")).unwrap(),
        b"remote"
    );
    assert_eq!(repo.snapshot("HEAD", 1).unwrap().behind, 0);
    assert!(repo.pull(&repo.oid("HEAD").unwrap()).unwrap().is_empty());
}

#[test]
fn pull_rejects_dirty_staged_untracked_and_stale_state_outside_project_scope() {
    let (dir, repo, _remote, _peer_dir, _peer) = pull_fixture();
    std::fs::create_dir(dir.path().join("project")).unwrap();
    let scoped = Repository::discover(&dir.path().join("project")).unwrap();
    let head = repo.oid("HEAD").unwrap();
    std::fs::write(dir.path().join("test.SchDoc"), b"local unsaved to git").unwrap();
    assert!(scoped.pull(&head).unwrap_err().contains("Commit or stash"));
    git(dir.path(), &["add", "test.SchDoc"]);
    let index = git(dir.path(), &["ls-files", "--stage"]);
    assert!(scoped.pull(&head).is_err());
    assert_eq!(git(dir.path(), &["ls-files", "--stage"]), index);
    git(
        dir.path(),
        &["restore", "--staged", "--worktree", "test.SchDoc"],
    );
    std::fs::write(dir.path().join("notes.txt"), b"untracked").unwrap();
    assert!(scoped.pull(&head).is_err());
    assert_eq!(
        std::fs::read(dir.path().join("notes.txt")).unwrap(),
        b"untracked"
    );
    std::fs::remove_file(dir.path().join("notes.txt")).unwrap();
    assert!(
        scoped
            .pull("0000000000000000000000000000000000000000")
            .unwrap_err()
            .contains("HEAD changed")
    );
    std::fs::write(dir.path().join(".git/MERGE_HEAD"), &head).unwrap();
    assert!(
        scoped
            .pull(&head)
            .unwrap_err()
            .contains("Finish the current")
    );
    assert_eq!(repo.oid("HEAD").unwrap(), head);
}

#[test]
fn pull_rejects_divergence_missing_upstream_and_detached_head() {
    let (dir, repo, _remote, peer_dir, peer) = pull_fixture();
    let head = repo.oid("HEAD").unwrap();
    std::fs::write(dir.path().join("local.SchDoc"), b"local").unwrap();
    let local = repo
        .commit(&["local.SchDoc".into()], "Local", Some(&head))
        .unwrap();
    std::fs::write(peer_dir.path().join("test.SchDoc"), b"remote").unwrap();
    peer.commit(&["test.SchDoc".into()], "Remote", Some(&head))
        .unwrap();
    peer.push().unwrap();
    assert!(repo.pull(&local).unwrap_err().contains("fast-forward"));
    assert_eq!(repo.oid("HEAD").unwrap(), local);
    assert_eq!(std::fs::read(dir.path().join("test.SchDoc")).unwrap(), b"a");
    assert!(!dir.path().join(".git/MERGE_HEAD").exists());
    git(dir.path(), &["branch", "--unset-upstream"]);
    assert!(repo.pull(&local).unwrap_err().contains("No upstream"));
    git(dir.path(), &["checkout", "--detach"]);
    assert!(repo.pull(&local).unwrap_err().contains("detached HEAD"));
}

#[test]
fn pull_does_not_overwrite_ignored_local_files() {
    let (dir, repo, _remote, peer_dir, peer) = pull_fixture();
    let head = repo.oid("HEAD").unwrap();
    std::fs::write(dir.path().join(".git/info/exclude"), "private.txt\n").unwrap();
    std::fs::write(dir.path().join("private.txt"), b"keep me").unwrap();
    std::fs::write(peer_dir.path().join("private.txt"), b"remote").unwrap();
    git(peer_dir.path(), &["add", "private.txt"]);
    git(peer_dir.path(), &["commit", "-m", "New file"]);
    peer.push().unwrap();
    assert!(repo.pull(&head).is_err());
    assert_eq!(repo.oid("HEAD").unwrap(), head);
    assert_eq!(
        std::fs::read(dir.path().join("private.txt")).unwrap(),
        b"keep me"
    );
}
#[test]
fn nul_status_handles_unicode_newlines_and_rename_pairs() {
    let files =
        parse_status("R  новое\nимя.SchDoc\0old.SchDoc\0?? 新.PcbDoc\0".as_bytes()).unwrap();
    assert_eq!(files[0].old_path.as_deref(), Some("old.SchDoc"));
    assert_eq!(files[1].path, "新.PcbDoc");
}

#[test]
fn real_native_pcb_imports_compare_semantics_instead_of_byte_derived_ids() {
    use eda_core::document_diff::{Kind, compare};
    let bytes = include_str!("../../../fixtures/pcb-update/synthetic.PcbDoc");
    let before = import_document(bytes.as_bytes(), "board.PcbDoc").unwrap();
    let changed = bytes.replace(
        "X1=10mm|Y1=20mm|X2=25mm|Y2=20mm",
        "X1=11mm|Y1=22mm|X2=26mm|Y2=22mm",
    );
    let after = import_document(changed.as_bytes(), "board.PcbDoc").unwrap();
    assert_ne!(before.id, after.id);
    let diff = compare(Some(&before), Some(&after));
    assert_eq!(diff.changes.len(), 1, "{diff:?}");
    assert_eq!(diff.changes[0].kind, Kind::Moved);
}

#[test]
fn native_schematic_uid_survives_insertions_and_deleted_records() {
    use eda_core::document_diff::{Kind, compare};
    use eda_core::schematic::SchPrimitiveKind as K;
    use eda_geom::Point;
    let mut d = crate::altium::new_schematic("review.SchDoc").unwrap();
    for x in [30, 60, 90] {
        d.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(eda_core::schematic_edit::primitive(
                K::Rectangle {
                    a: Point::mm(x, 50),
                    b: Point::mm(x + 10, 60),
                    radius: Point::default(),
                },
                None,
            ));
    }
    let bytes = crate::altium::writer::serialize(&d).unwrap();
    let before = import_document(&bytes, "review.SchDoc").unwrap();
    let mut next = before.clone();
    next.schematic.as_mut().unwrap().primitives.remove(0);
    if let K::Rectangle { a, b, .. } = &mut next.schematic.as_mut().unwrap().primitives[0].kind {
        a.y += 1_000_000;
        b.y += 1_000_000;
    }
    let bytes = crate::altium::writer::serialize(&next).unwrap();
    let after = import_document(&bytes, "review.SchDoc").unwrap();
    let diff = compare(Some(&before), Some(&after));
    assert_eq!(diff.changes.len(), 2, "{diff:?}");
    assert!(diff.changes.iter().any(|c| c.kind == Kind::Removed));
    assert!(diff.changes.iter().any(|c| c.kind == Kind::Moved));
}
