use super::*;
use crate::fs::{FsMetadata, StdFileSystem};
use std::cell::Cell;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

fn paste(
    fs: &dyn FileSystem,
    op: ClipboardOp,
    from: &Path,
    to: &Path,
    overwrite: bool,
) -> Result<(), PasteError> {
    paste_cancellable(fs, op, from, to, overwrite, &|| false)
}

pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "dear-file-browser-paste-test-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("could not create test directory: {e}"),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// The source rename can cross devices. Backup, installation and rollback use
// the native same-volume primitives inside the test directory.
pub(crate) struct MoveFs {
    source: PathBuf,
    read_only: bool,
    report_file_details: bool,
    rename_error: Cell<Option<ErrorKind>>,
    pub(crate) after_listing: fn(&Path) -> io::Result<()>,
    after_copy: fn(&Path) -> io::Result<()>,
    before_rename: fn(&Path, &Path) -> io::Result<()>,
    before_unlink: fn(&Path) -> io::Result<()>,
}

impl MoveFs {
    pub(crate) fn cross_device(source: &Path) -> Self {
        Self {
            source: source.to_owned(),
            read_only: false,
            report_file_details: true,
            rename_error: Cell::new(Some(ErrorKind::CrossesDevices)),
            after_listing: |_| Ok(()),
            after_copy: |_| Ok(()),
            before_rename: |_, _| Ok(()),
            before_unlink: |_| Ok(()),
        }
    }
}

impl FileSystem for MoveFs {
    fn visit_dir(&self, dir: &Path, visit: &mut dyn FnMut(FsEntry) -> ScanVisit) -> io::Result<()> {
        StdFileSystem.visit_dir(dir, &mut |mut entry| {
            // Native paths are distinct even when their display names coincide.
            entry.name = "display name".to_owned();
            visit(entry)
        })?;
        if dir == self.source.as_path() {
            (self.after_listing)(dir)?;
        }
        Ok(())
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        StdFileSystem.canonicalize(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<FsMetadata> {
        StdFileSystem.metadata(path)
    }
    fn inspect_entry(&self, path: &Path) -> io::Result<FsEntry> {
        StdFileSystem.inspect_entry(path)
    }
    fn inspect_move_source(&self, path: &Path) -> io::Result<(FsEntry, Option<FsEntryIdentity>)> {
        let (mut entry, id) = StdFileSystem.inspect_move_source(path)?;
        if !self.report_file_details {
            entry.size = None;
            entry.modified = None;
        }
        Ok((entry, id))
    }
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        StdFileSystem.create_dir(path)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.rename_noreplace(from, to)
    }
    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        if from == self.source.as_path()
            && let Some(error) = self.rename_error.take()
        {
            return Err(Error::from(error));
        }
        (self.before_rename)(from, to)?;
        StdFileSystem.rename_noreplace(from, to)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        (self.before_unlink)(&self.source)?;
        StdFileSystem.remove_file(path)
    }
    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        StdFileSystem.remove_dir(path)
    }
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        assert!(!self.source.starts_with(path) && !path.starts_with(&self.source));
        StdFileSystem.remove_dir_all(path)
    }
    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<u64> {
        assert!(!self.read_only);
        let count = StdFileSystem.copy_file(from, to)?;
        (self.after_copy)(from)?;
        Ok(count)
    }
}

#[test]
fn ordinary_copy_rename_and_cross_device_move_preserve_contents() {
    for (op, cross_device) in [
        (ClipboardOp::Copy, false),
        (ClipboardOp::Cut, false),
        (ClipboardOp::Cut, true),
    ] {
        let temp = TempDir::new();
        let src = temp.path().join("source");
        let dst = temp.path().join("target");
        fs::create_dir_all(src.join("nested/empty")).unwrap();
        fs::write(src.join("one"), b"first").unwrap();
        fs::write(src.join("nested/two"), b"second").unwrap();

        let mounted = MoveFs::cross_device(&src);
        let fs: &dyn FileSystem = if cross_device {
            &mounted
        } else {
            &StdFileSystem
        };
        paste(fs, op, &src, &dst, false).unwrap();
        assert_eq!(fs::read(dst.join("one")).unwrap(), b"first");
        assert_eq!(fs::read(dst.join("nested/two")).unwrap(), b"second");
        assert!(dst.join("nested/empty").is_dir());
        assert_eq!(src.exists(), op == ClipboardOp::Copy);
    }
}

#[test]
fn vanished_child_prevents_copy_and_cross_device_cut_from_replacing_the_target() {
    for op in [ClipboardOp::Copy, ClipboardOp::Cut] {
        let temp = TempDir::new();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("moving"), b"original").unwrap();
        fs::write(source.join("survivor"), b"still here").unwrap();
        fs::write(&target, b"previous").unwrap();
        let mut filesystem = MoveFs::cross_device(&source);
        filesystem.after_listing = |dir| fs::rename(dir.join("moving"), dir.join("moved"));

        assert!(matches!(
            paste(&filesystem, op, &source, &target, true),
            Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::NotFound
        ));
        assert_eq!(fs::read(&target).unwrap(), b"previous");
        assert_eq!(fs::read(source.join("moved")).unwrap(), b"original");
        assert_eq!(fs::read(source.join("survivor")).unwrap(), b"still here");
    }
}

#[test]
fn checks_both_directions_of_ancestry_before_a_transfer() {
    let temp = TempDir::new();
    let dir = temp.path().join("folder");
    let alias = temp.path().join("other").join("..").join("folder");
    fs::create_dir(&dir).unwrap();
    fs::create_dir(temp.path().join("other")).unwrap();
    fs::write(dir.join("note"), b"note").unwrap();

    let mut fs = MoveFs::cross_device(&dir);
    fs.read_only = true;
    for (source, target) in [
        (dir.join("note"), dir.clone()),
        (alias.join("note"), dir.clone()),
        (dir.clone(), alias.join("new child")),
        (alias.join("note"), dir.join("note")),
    ] {
        assert!(
            matches!(paste(&fs, ClipboardOp::Copy, &source, &target, true), Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::InvalidInput)
        );
    }
    let sibling = keep_both_target(&StdFileSystem, &dir).unwrap();
    paste(
        &StdFileSystem,
        ClipboardOp::Copy,
        &dir.join("note"),
        &sibling,
        false,
    )
    .unwrap();
    assert_eq!(fs::read(sibling).unwrap(), b"note");
}

#[test]
fn keep_both_preserves_extensions_and_skips_occupied_names() {
    let temp = TempDir::new();
    let dir = temp.path();
    fs::write(dir.join("a.tar.gz"), b"original").unwrap();
    fs::write(dir.join("a (copy).tar.gz"), b"occupied").unwrap();
    fs::write(dir.join(".env"), b"original").unwrap();
    fs::write(dir.join(".env.local"), b"original").unwrap();
    assert_eq!(
        keep_both_target(&StdFileSystem, &dir.join("a.tar.gz")).unwrap(),
        dir.join("a (copy 2).tar.gz")
    );
    assert_eq!(
        keep_both_target(&StdFileSystem, &dir.join(".env")).unwrap(),
        dir.join(".env (copy)")
    );
    assert_eq!(
        keep_both_target(&StdFileSystem, &dir.join(".env.local")).unwrap(),
        dir.join(".env (copy).local")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn directory_copy_and_keep_both_preserve_distinct_non_utf8_names() {
    use std::os::unix::ffi::OsStrExt;

    let temp = TempDir::new();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    fs::create_dir(&source).unwrap();
    let a = OsStr::from_bytes(b"report\xff.txt");
    let b = OsStr::from_bytes(b"report\xfe.txt");
    fs::write(source.join(a), b"A").unwrap();
    fs::write(source.join(b), b"B").unwrap();

    paste(&StdFileSystem, ClipboardOp::Copy, &source, &target, false).unwrap();
    assert_eq!(fs::read(target.join(a)).unwrap(), b"A");
    assert_eq!(fs::read(target.join(b)).unwrap(), b"B");
    assert_eq!(
        keep_both_target(&StdFileSystem, &target.join(a)).unwrap(),
        target.join(OsStr::from_bytes(b"report\xff (copy).txt")),
    );
}

#[test]
fn overwrite_replaces_a_complete_directory() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&dst).unwrap();
    fs::write(src.join("new"), b"new contents").unwrap();
    fs::write(dst.join("old"), b"old contents").unwrap();

    paste(&StdFileSystem, ClipboardOp::Copy, &src, &dst, true).unwrap();
    assert_eq!(fs::read(dst.join("new")).unwrap(), b"new contents");
    assert!(!dst.join("old").exists());
    assert_eq!(fs::read(src.join("new")).unwrap(), b"new contents");
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
}

#[test]
fn rename_listing_and_copy_errors_preserve_source_and_previous_target() {
    for fault in [
        ErrorKind::PermissionDenied,
        ErrorKind::Interrupted,
        ErrorKind::StorageFull,
        ErrorKind::Unsupported,
    ] {
        let temp = TempDir::new();
        let src = temp.path().join("source");
        let dst = temp.path().join("target");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("note"), b"source").unwrap();
        fs::write(&dst, b"previous").unwrap();
        let mut fs = MoveFs::cross_device(&src);
        match fault {
            ErrorKind::PermissionDenied => {
                fs.rename_error.set(Some(fault));
                fs.after_copy = |_| panic!("unexpected copy after a denied rename");
            }
            ErrorKind::Interrupted => fs.after_listing = |_| Err(ErrorKind::Interrupted.into()),
            ErrorKind::Unsupported => fs.report_file_details = false,
            _ => fs.after_copy = |_| Err(ErrorKind::StorageFull.into()),
        }
        assert!(
            matches!(paste(&fs, ClipboardOp::Cut, &src, &dst, true), Err(PasteError::Transfer(e)) if e.kind() == fault)
        );
        assert_eq!(fs::read(src.join("note")).unwrap(), b"source");
        assert_eq!(fs::read(&dst).unwrap(), b"previous");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
    }
}

#[test]
fn a_vacant_target_stays_available_until_the_copy_is_complete() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join(".dear-file-browser-paste-0");
    let occupied = temp.path().join(".dear-file-browser-paste-1");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("note"), b"existing contents").unwrap();
    fs::write(&src, b"source").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.after_copy = |_| Err(ErrorKind::StorageFull.into());
    assert!(matches!(
        paste(&fs, ClipboardOp::Copy, &src, &dst, false),
        Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::StorageFull
    ));
    assert!(!dst.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);

    fs.after_copy = |source| {
        let mut arrival =
            fs::File::create_new(source.with_file_name(".dear-file-browser-paste-0"))?;
        std::io::Write::write_all(&mut arrival, b"new arrival")
    };
    assert!(matches!(
        paste(&fs, ClipboardOp::Cut, &src, &dst, false),
        Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::AlreadyExists
    ));
    assert_eq!(fs::read(src).unwrap(), b"source");
    assert_eq!(fs::read(dst).unwrap(), b"new arrival");
    assert_eq!(
        fs::read(occupied.join("note")).unwrap(),
        b"existing contents"
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 3);
}

#[test]
fn occupied_rollback_path_retains_the_previous_target() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::write(&src, b"source").unwrap();
    fs::write(&dst, b"previous").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.after_copy = |source| {
        let mut new_target = fs::File::create_new(source.with_file_name("target"))?;
        std::io::Write::write_all(&mut new_target, b"new arrival")?;
        Err(ErrorKind::Interrupted.into())
    };
    let error = paste(&fs, ClipboardOp::Cut, &src, &dst, true).unwrap_err();
    let work = fs::read_dir(temp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir())
        .unwrap();
    let previous = work.join("previous");
    assert!(
        error
            .to_string()
            .contains(previous.to_string_lossy().as_ref())
    );
    assert_eq!(fs::read(previous).unwrap(), b"previous");
    assert_eq!(fs::read(dst).unwrap(), b"new arrival");
    assert_eq!(fs::read(src).unwrap(), b"source");
}

#[test]
fn changed_source_file_is_retained_after_cross_device_copy() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::write(&src, b"original").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.after_copy = |path| fs::write(path, b"new version");

    assert!(matches!(
        paste(&fs, ClipboardOp::Cut, &src, &dst, false),
        Err(PasteError::AfterInstall(_))
    ));
    assert_eq!(fs::read(&src).unwrap(), b"new version");
    assert_eq!(fs::read(&dst).unwrap(), b"original");
}

#[test]
fn overwrite_install_and_rollback_preserve_an_arrival_at_rename_time() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::write(&src, b"source").unwrap();
    fs::write(&dst, b"previous").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.before_rename = |from, to| {
        if from.file_name() == Some(OsStr::new("incoming")) {
            fs::write(to, b"new arrival")?;
        }
        Ok(())
    };

    let error = paste(&fs, ClipboardOp::Cut, &src, &dst, true).unwrap_err();
    let previous = temp
        .path()
        .join(".dear-file-browser-paste-0")
        .join("previous");
    assert!(
        error
            .to_string()
            .contains(previous.to_string_lossy().as_ref())
    );
    assert_eq!(fs::read(previous).unwrap(), b"previous");
    assert_eq!(fs::read(dst).unwrap(), b"new arrival");
    assert_eq!(fs::read(src).unwrap(), b"source");
}

#[cfg(any(unix, windows))]
#[test]
fn cross_device_cut_keeps_a_replacement_with_the_same_size_and_timestamp() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::write(&src, b"old").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.before_rename = |path, to| {
        if path.file_name() != Some(OsStr::new("source"))
            || to.file_name() != Some(OsStr::new("removing"))
        {
            return Ok(());
        }
        let modified = fs::metadata(path)?.modified()?;
        fs::rename(path, path.with_file_name("original"))?;
        fs::write(path, b"new")?;
        fs::File::options()
            .write(true)
            .open(path)?
            .set_modified(modified)
    };

    assert!(matches!(
        paste(&fs, ClipboardOp::Cut, &src, &dst, false),
        Err(PasteError::AfterInstall(_))
    ));
    assert_eq!(fs::read(src).unwrap(), b"new");
    assert_eq!(fs::read(dst).unwrap(), b"old");
}

#[cfg(any(unix, windows))]
#[test]
fn cross_device_cut_checks_the_parent_identity_before_removing_its_children() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("child"), b"original").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.after_copy = |path| {
        let parent = path.parent().unwrap();
        let old_parent = parent.with_file_name("old-parent");
        fs::rename(parent, &old_parent)?;
        fs::create_dir(parent)?;
        fs::rename(old_parent.join("child"), path)
    };

    assert!(matches!(
        paste(&fs, ClipboardOp::Cut, &src, &dst, false),
        Err(PasteError::AfterInstall(_))
    ));
    assert_eq!(fs::read(src.join("child")).unwrap(), b"original");
    assert_eq!(fs::read(dst.join("child")).unwrap(), b"original");
}

#[test]
fn cross_device_cut_keeps_a_new_file_arriving_at_the_last_unlink() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::write(&src, b"old").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.before_unlink = |source| fs::write(source, b"new");
    assert!(matches!(
        paste(&fs, ClipboardOp::Cut, &src, &dst, false),
        Err(PasteError::AfterInstall(_))
    ));
    assert_eq!(fs::read(src).unwrap(), b"new");
    assert_eq!(fs::read(dst).unwrap(), b"old");
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
}

#[test]
fn a_reoccupied_source_name_keeps_both_directories_on_cleanup_failure() {
    let temp = TempDir::new();
    let src = temp.path().join("source");
    let dst = temp.path().join("target");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("original"), b"moved").unwrap();
    let mut fs = MoveFs::cross_device(&src);
    fs.after_listing = |dir| fs::write(dir.join("later"), b"keep");
    fs.before_rename = |from, to| {
        if from.file_name() == Some(OsStr::new("removing"))
            && to.file_name() == Some(OsStr::new("source"))
        {
            fs::create_dir(to)?;
            fs::write(to.join("new"), b"new")?;
        }
        Ok(())
    };
    let error = paste(&fs, ClipboardOp::Cut, &src, &dst, false).unwrap_err();
    let retained = temp
        .path()
        .join(".dear-file-browser-paste-0")
        .join("removing");
    assert!(matches!(&error, PasteError::AfterInstall(_)));
    assert!(
        error
            .to_string()
            .contains(retained.to_string_lossy().as_ref())
    );
    assert_eq!(fs::read(retained.join("later")).unwrap(), b"keep");
    assert_eq!(fs::read(src.join("new")).unwrap(), b"new");
    assert_eq!(fs::read(dst.join("original")).unwrap(), b"moved");
}

#[cfg(unix)]
#[test]
fn directory_links_are_renamed_as_entries_and_never_copied_recursively() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let src = temp.path().join("source");
    let real = temp.path().join("real");
    let link = src.join("link");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&real).unwrap();
    fs::write(real.join("note"), b"original").unwrap();
    symlink(&real, &link).unwrap();
    assert_eq!(
        validate_target(&StdFileSystem, &real, &link.join("new child"), false)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidInput
    );

    let fs = MoveFs::cross_device(&src);
    assert!(
        matches!(paste(&fs, ClipboardOp::Cut, &src, &temp.path().join("target"), false), Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::Unsupported)
    );
    let moved = temp.path().join("moved link");
    paste(&StdFileSystem, ClipboardOp::Cut, &link, &moved, false).unwrap();
    assert_eq!(fs::read_link(moved).unwrap(), real);
    assert_eq!(fs::read(real.join("note")).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn moves_and_deletes_act_on_the_link_even_when_its_referent_is_an_ancestor() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let dir = temp.path();
    let link = dir.join("link");
    let moved = dir.join("moved");
    symlink(dir, &link).unwrap();
    StdFileSystem
        .visit_dir(dir, &mut |entry| {
            assert!(entry.is_dir && entry.is_symlink);
            ScanVisit::Continue
        })
        .unwrap();
    paste(&StdFileSystem, ClipboardOp::Cut, &link, &moved, false).unwrap();
    assert_eq!(fs::read_link(&moved).unwrap(), dir);
    delete_entry(&StdFileSystem, &moved, false).unwrap();
    assert!(!entry_exists(&StdFileSystem, &moved).unwrap());

    let source = dir.join("file");
    fs::write(&source, b"source").unwrap();
    symlink(dir.join("missing"), &link).unwrap();
    assert_eq!(
        StdFileSystem
            .rename_noreplace(&source, &link)
            .unwrap_err()
            .kind(),
        ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&source).unwrap(), b"source");
    assert_eq!(fs::read_link(&link).unwrap(), dir.join("missing"));
    fs::remove_file(&link).unwrap();
    symlink(dir, &link).unwrap();
    paste(&StdFileSystem, ClipboardOp::Copy, &source, &link, true).unwrap();
    assert_eq!(fs::read(link).unwrap(), b"source");
    assert_eq!(fs::read(source).unwrap(), b"source");
}

#[cfg(windows)]
#[test]
fn deleting_a_windows_directory_link_preserves_its_target() {
    let temp = TempDir::new();
    let target = temp.path().join("directory");
    let link = temp.path().join("link");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("child"), b"child").unwrap();
    if let Err(e) = std::os::windows::fs::symlink_dir(&target, &link) {
        if e.raw_os_error() == Some(windows_sys::Win32::Foundation::ERROR_PRIVILEGE_NOT_HELD as i32)
        {
            return;
        }
        panic!("could not create directory link: {e}");
    }
    delete_entry(&StdFileSystem, &link, false).unwrap();
    assert!(!entry_exists(&StdFileSystem, &link).unwrap());
    assert_eq!(fs::read(target.join("child")).unwrap(), b"child");
}

#[cfg(windows)]
#[test]
fn windows_junctions_are_never_traversed_during_copy_or_deletion() {
    let temp = TempDir::new();
    let source = temp.path().join("source");
    let referent = temp.path().join("referent");
    let junction = source.join("junction");
    let destination = temp.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&referent).unwrap();
    fs::write(referent.join("child"), b"original").unwrap();

    // Use fixed relative arguments so cmd.exe never expands a user-supplied path.
    let output = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J", r"source\junction", "referent"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "mklink /J failed: {output:?}");

    let fs = StdFileSystem;
    assert!(fs.inspect_entry(&junction).unwrap().is_symlink);
    assert!(fs.inspect_move_source(&junction).unwrap().0.is_symlink);
    fs.visit_dir(&source, &mut |entry| {
        assert!(entry.is_dir && entry.is_symlink);
        ScanVisit::Continue
    })
    .unwrap();
    assert!(matches!(
        paste(&fs, ClipboardOp::Copy, &source, &destination, false),
        Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::Unsupported
    ));
    assert!(!entry_exists(&fs, &destination).unwrap());

    let cross_device = MoveFs::cross_device(&junction);
    assert!(matches!(
        paste(&cross_device, ClipboardOp::Cut, &junction, &destination, false),
        Err(PasteError::Transfer(e)) if e.kind() == ErrorKind::Unsupported
    ));
    assert!(!entry_exists(&fs, &destination).unwrap());

    delete_entry(&fs, &junction, false).unwrap();
    assert!(!entry_exists(&fs, &junction).unwrap());
    assert_eq!(std::fs::read(referent.join("child")).unwrap(), b"original");
}
