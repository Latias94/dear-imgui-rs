use super::*;
use crate::fs::{FsMetadata, StdFileSystem};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

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

// Only the source rename is injected. Backup, installation and rollback use
// ordinary same-volume operations inside the test directory.
pub(crate) struct MoveFs {
    source: PathBuf,
    read_only: bool,
    report_file_details: bool,
    rename_error: ErrorKind,
    pub(crate) after_listing: fn(&Path) -> io::Result<()>,
    after_copy: fn(&Path) -> io::Result<()>,
}

impl MoveFs {
    pub(crate) fn cross_device(source: &Path) -> Self {
        Self {
            source: source.to_owned(),
            read_only: false,
            report_file_details: true,
            rename_error: ErrorKind::CrossesDevices,
            after_listing: |_| Ok(()),
            after_copy: |_| Ok(()),
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
        let mut entry = StdFileSystem.inspect_entry(path)?;
        if !self.report_file_details {
            entry.size = None;
            entry.modified = None;
        }
        Ok(entry)
    }
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        StdFileSystem.create_dir(path)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        assert!(!self.read_only);
        if from == self.source.as_path() {
            return Err(Error::from(self.rename_error));
        }
        StdFileSystem.rename(from, to)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        assert!(!self.read_only);
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
    assert_eq!(
        keep_both_target(&StdFileSystem, &dir.join("a.tar.gz")).unwrap(),
        dir.join("a (copy 2).tar.gz")
    );
    assert_eq!(
        keep_both_target(&StdFileSystem, &dir.join(".env")).unwrap(),
        dir.join(".env (copy)")
    );
}

#[cfg(unix)]
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
                fs.rename_error = fault;
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
