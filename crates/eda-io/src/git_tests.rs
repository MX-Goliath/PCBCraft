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
