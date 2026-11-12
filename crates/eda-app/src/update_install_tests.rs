use super::*;

fn put(root: &Path, path: &str, bytes: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn archive(root: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
    let path = root.join("update.zip");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    path
}

#[test]
fn portable_replaces_the_entire_payload_preserving_data_and_removing_obsolete_files() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("portable with spaces ' and Юникод");
    let cache = root.path().join("cache");
    let payload = root.path().join("payload");
    fs::create_dir_all(&cache).unwrap();
    fs::create_dir_all(&payload).unwrap();
    put(&target, "pcbcraft.exe", b"old executable");
    put(&target, "old-runtime.dll", b"old runtime");
    put(&target, "licenses/obsolete.txt", b"old notices");
    put(
        &target,
        "build-info.json",
        br#"{"dependencies":{"pcbcraft.exe":[],"old-runtime.dll":[]}}"#,
    );
    put(&target, "Data/app.ron", b"user settings");
    put(&target, "Data/project.PcbDoc", b"user work");
    put(&target, "portable.flag", b"user marker");
    put(&target, "my-project.PcbDoc", b"user project");
    let zip = archive(
        root.path(),
        &[
            ("release/pcbcraft.exe", b"new executable"),
            ("release/new-runtime.dll", b"new runtime"),
            ("release/licenses/current.txt", b"new notices"),
            ("release/Data/app.ron", b"must never replace settings"),
            ("release/portable.flag", b"must never replace marker"),
        ],
    );
    extract_portable(&zip, &payload).unwrap();
    replace_portable(&payload, &target, &cache).unwrap();
    assert_eq!(
        fs::read(target.join("pcbcraft.exe")).unwrap(),
        b"new executable"
    );
    assert_eq!(
        fs::read(target.join("new-runtime.dll")).unwrap(),
        b"new runtime"
    );
    assert!(!target.join("old-runtime.dll").exists());
    assert!(!target.join("licenses/obsolete.txt").exists());
    assert_eq!(
        fs::read(target.join("Data/app.ron")).unwrap(),
        b"user settings"
    );
    assert_eq!(
        fs::read(target.join("Data/project.PcbDoc")).unwrap(),
        b"user work"
    );
    assert_eq!(
        fs::read(target.join("portable.flag")).unwrap(),
        b"user marker"
    );
    assert_eq!(
        fs::read(target.join("my-project.PcbDoc")).unwrap(),
        b"user project"
    );
    assert!(!cache.join("transaction.json").exists());
    // The next version removes files installed by this version via inventory.
    let next = root.path().join("next");
    put(&next, "pcbcraft.exe", b"third executable");
    replace_portable(&next, &target, &cache).unwrap();
    assert!(!target.join("new-runtime.dll").exists());
    assert!(!target.join("licenses/current.txt").exists());
}

#[test]
fn rejects_archive_traversal_aliases_multiple_roots_and_missing_executable() {
    let root = tempfile::tempdir().unwrap();
    for entries in [
        vec![("../pcbcraft.exe", &b"evil"[..])],
        vec![("release/../pcbcraft.exe", &b"evil"[..])],
        vec![("release/C:/pcbcraft.exe", &b"evil"[..])],
        vec![("release/file:stream", &b"evil"[..])],
        vec![("release/CON.txt", &b"evil"[..])],
        vec![("release/file. ", &b"evil"[..])],
        vec![("release/readme", &b"no executable"[..])],
        vec![
            ("one/pcbcraft.exe", &b"one"[..]),
            ("two/extra", &b"two"[..]),
        ],
        vec![
            ("release/pcbcraft.exe", &b"one"[..]),
            ("release/PCBCRAFT.EXE", &b"two"[..]),
        ],
    ] {
        let destination = tempfile::tempdir().unwrap();
        let zip = archive(root.path(), &entries);
        assert!(
            extract_portable(&zip, destination.path()).is_err(),
            "{entries:?}"
        );
    }
}

#[test]
fn rejects_zip_symlinks_without_creating_a_link_on_the_host() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("link.zip");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    zip.add_symlink(
        "release/pcbcraft.exe",
        "../../outside",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.finish().unwrap();
    let destination = tempfile::tempdir().unwrap();
    assert!(
        extract_portable(&path, destination.path())
            .unwrap_err()
            .contains("symlink")
    );
}

#[test]
fn interrupted_replacement_rolls_back_without_removing_untouched_originals() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("install");
    let backup = target.join(".pcbcraft-backup-test");
    let cache = root.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    put(&backup, "pcbcraft.exe", b"old exe");
    put(&target, "pcbcraft.exe", b"incomplete new exe");
    put(&target, "untouched.dll", b"old untouched dll");
    put(&target, "new.dll", b"new dll");
    let tx = Transaction {
        backup,
        old: ["pcbcraft.exe".into(), "untouched.dll".into()].into(),
        new: [
            "pcbcraft.exe".into(),
            "untouched.dll".into(),
            "new.dll".into(),
        ]
        .into(),
        committed: false,
    };
    write_json(&cache.join("transaction.json"), &tx).unwrap();
    recover_transaction(&target, &cache).unwrap();
    assert_eq!(fs::read(target.join("pcbcraft.exe")).unwrap(), b"old exe");
    assert_eq!(
        fs::read(target.join("untouched.dll")).unwrap(),
        b"old untouched dll"
    );
    assert!(!target.join("new.dll").exists());
}

#[test]
fn file_replacement_error_restores_all_old_application_files() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("install");
    let payload = root.path().join("payload");
    let cache = root.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    put(&target, "pcbcraft.exe", b"original");
    // A directory blocks writing the new file after old files have been moved.
    fs::create_dir_all(target.join("z-blocked.dll")).unwrap();
    put(&payload, "pcbcraft.exe", b"new");
    put(&payload, "z-blocked.dll", b"blocked");
    assert!(replace_portable(&payload, &target, &cache).is_err());
    assert_eq!(fs::read(target.join("pcbcraft.exe")).unwrap(), b"original");
    assert!(!target.join("update-files.json").exists());
}

#[test]
fn tampering_after_download_is_rejected_before_replacing_anything() {
    let root = tempfile::tempdir().unwrap();
    let exe = root.path().join("pcbcraft.exe");
    fs::write(&exe, b"old application").unwrap();
    let package = root.path().join("package.zip");
    fs::write(&package, b"verified package").unwrap();
    let pending = Pending {
        package: super::super::tests::select(super::super::tests::release(
            "v0.2.0",
            Distribution::WindowsInstaller,
            "x86_64",
        ))
        .unwrap()
        .1
        .unwrap(),
        path: package.clone(),
        digest: hash_file(&package).unwrap(),
        executable: exe.clone(),
    };
    write_json(&root.path().join("pending.json"), &pending).unwrap();
    assert_eq!(read_pending(root.path()).unwrap().digest, pending.digest);
    fs::write(&package, b"tampered package").unwrap();
    assert!(
        apply(root.path(), &pending)
            .unwrap_err()
            .contains("SHA-256")
    );
    assert_eq!(fs::read(exe).unwrap(), b"old application");
}

#[test]
fn appimage_replaces_bytes_in_place_without_touching_adjacent_files() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("My PCBCraft.AppImage");
    let package = root.path().join("new.AppImage");
    fs::write(&target, b"old").unwrap();
    fs::write(&package, b"new").unwrap();
    fs::write(root.path().join("settings"), b"keep").unwrap();
    replace_appimage(&package, &target).unwrap();
    assert_eq!(fs::read(target).unwrap(), b"new");
    assert_eq!(fs::read(root.path().join("settings")).unwrap(), b"keep");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.path().join("My PCBCraft.AppImage"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
}
