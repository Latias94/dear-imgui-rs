use std::borrow::Cow;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod windows;

pub(crate) fn display_path(path: &Path) -> Cow<'_, str> {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    if let Some(std::path::Component::Prefix(prefix)) = path.components().next() {
        match prefix.kind() {
            std::path::Prefix::VerbatimDisk(_) => {
                if let Some(standard) = text.strip_prefix(r"\\?\") {
                    return Cow::Owned(standard.to_owned());
                }
            }
            std::path::Prefix::VerbatimUNC(_, _) => {
                if let Some(standard) = text.strip_prefix(r"\\?\UNC\") {
                    return Cow::Owned(format!(r"\\{standard}"));
                }
            }
            _ => {}
        }
    }
    text
}

#[cfg(all(test, windows))]
#[test]
fn path_labels_omit_only_known_verbatim_wrappers() {
    for (native, display) in [
        (r"\\?\C:\path", r"C:\path"),
        (r"\\?\UNC\server\share\path", r"\\server\share\path"),
        (r"\\?\Volume{guid}\path", r"\\?\Volume{guid}\path"),
    ] {
        let path = Path::new(native);
        assert_eq!(display_path(path), display);
        assert_eq!(path.as_os_str(), native);
    }
}

pub(crate) fn file_name(path: &Path) -> io::Result<&OsStr> {
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "expected a named entry"))
}

pub(crate) fn canonical_entry(fs: &dyn FileSystem, path: &Path) -> io::Result<PathBuf> {
    let name = file_name(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(fs.canonicalize(parent)?.join(name))
}

/// Minimal file metadata used by the in-UI file browser.
#[derive(Clone, Debug)]
pub struct FsMetadata {
    /// Whether the path refers to a directory.
    pub is_dir: bool,
    /// Whether the path itself is a symbolic link or a Windows name-surrogate reparse point.
    pub is_symlink: bool,
}

/// Identity of a filesystem object across name lookups during a cross-device cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsEntryIdentity {
    /// Identifier of the filesystem containing the object.
    pub volume: u64,
    /// Object identifier within that filesystem.
    pub file: u128,
}

/// Directory entry yielded by [`FileSystem::visit_dir`].
#[derive(Clone, Debug)]
pub struct FsEntry {
    /// Display name. Use the file name from `path` for filesystem operations.
    pub name: String,
    /// Full path of the immediate child, preserving its native file name.
    pub path: PathBuf,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Whether this entry itself is a symbolic link or a Windows name-surrogate reparse point.
    pub is_symlink: bool,
    /// File size in bytes (files and file-links only; `None` for directories or when unavailable).
    pub size: Option<u64>,
    /// Last modified timestamp (when available).
    pub modified: Option<std::time::SystemTime>,
}

impl FsEntry {
    fn from_metadata(path: PathBuf, metadata: std::fs::Metadata) -> Self {
        Self {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path,
            is_dir: metadata.is_dir(),
            is_symlink: metadata.is_symlink(),
            size: metadata.is_file().then_some(metadata.len()),
            modified: metadata.modified().ok(),
        }
    }
}

/// Flow control returned from a [`FileSystem::visit_dir`] visitor.
///
/// Directory implementations must stop enumerating as soon as the visitor returns
/// [`Self::Stop`]. This lets a background scan observe cancellation without giving
/// filesystem implementations access to internal worker state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScanVisit {
    /// Continue enumerating directory entries.
    #[default]
    Continue,
    /// Stop enumerating the current directory.
    Stop,
}

/// File system abstraction (IGFD `IFileSystem`-like).
///
/// The enumeration boundary is deliberately streaming and object-safe. A
/// background scan can stop a cooperative filesystem between entries without
/// constructing an eager directory snapshot on the UI thread.
pub trait FileSystem {
    /// Visit entries of a directory one at a time. Copies and cross-device moves
    /// require an error if an entry cannot be enumerated or classified.
    ///
    /// Stop promptly when `visit` returns [`ScanVisit::Stop`].
    fn visit_dir(
        &self,
        dir: &Path,
        visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
    ) -> std::io::Result<()>;
    /// Visit entries for display. A backend may skip an enumerated child that
    /// disappears before its metadata is read. Directory and other lookup errors
    /// should be returned. The default delegates to [`Self::visit_dir`].
    ///
    /// The visitor runs on the scan worker for background scans and on the caller
    /// thread for [`crate::ScanPolicy::Blocking`]. Stop promptly on [`ScanVisit::Stop`]
    /// because destroying a background dialog waits for its worker to return.
    fn visit_dir_for_scan(
        &self,
        dir: &Path,
        visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
    ) -> io::Result<()> {
        self.visit_dir(dir, visit)
    }
    /// Resolve an existing path to its canonical absolute path, including links.
    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf>;
    /// Fetch minimal metadata for a path.
    fn metadata(&self, path: &Path) -> std::io::Result<FsMetadata>;
    /// Inspect the named entry without following its final symbolic link.
    /// A dangling link is an existing entry. Backends that follow links in
    /// `metadata` must override this method if they support symbolic links.
    /// Cross-device cuts need a size or modification time for each file to
    /// check for changes before removing the source. The default supplies neither.
    fn inspect_entry(&self, path: &Path) -> std::io::Result<FsEntry> {
        let metadata = self.metadata(path)?;
        Ok(FsEntry {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path: path.to_owned(),
            is_dir: metadata.is_dir,
            is_symlink: metadata.is_symlink,
            size: None,
            modified: None,
        })
    }
    /// Inspect a source for a cross-device cut. The optional identity identifies the
    /// filesystem object independently of its name, size and modification time.
    /// Backends where other writers can replace entries should return an identity
    /// from the same metadata snapshot. A single-writer backend can use the default.
    fn inspect_move_source(&self, path: &Path) -> io::Result<(FsEntry, Option<FsEntryIdentity>)> {
        Ok((self.inspect_entry(path)?, None))
    }
    /// Create a new directory. Return [`std::io::ErrorKind::AlreadyExists`]
    /// if any entry occupies the path. Paste uses this to claim its work directory.
    fn create_dir(&self, path: &Path) -> std::io::Result<()>;
    /// Rename/move a path. Return [`std::io::ErrorKind::CrossesDevices`] when a
    /// move requires copying between filesystems.
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    /// Move an entry to a vacant name. The default suits single-writer filesystems.
    /// Backends shared with other writers should check the destination atomically
    /// with the rename so a concurrent arrival cannot be replaced.
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        match self.inspect_entry(to) {
            Ok(_) => Err(std::io::ErrorKind::AlreadyExists.into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.rename(from, to),
            Err(e) => Err(e),
        }
    }
    /// Remove a file or the link itself, including a Windows junction.
    fn remove_file(&self, path: &Path) -> std::io::Result<()>;
    /// Remove an empty directory.
    fn remove_dir(&self, path: &Path) -> std::io::Result<()>;
    /// Remove a directory and all of its contents without traversing symbolic links.
    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()>;
    /// Copy a file. The destination may be truncated if it already exists.
    ///
    /// Returns the number of bytes copied (mirrors `std::fs::copy`).
    fn copy_file(&self, from: &Path, to: &Path) -> std::io::Result<u64>;
}

/// Default filesystem implementation using native filesystem operations.
///
/// On Unix, `rename_noreplace` prefers a native exclusive rename. Where that
/// is unavailable, regular files can be hard-linked to a vacant destination
/// and unlinked from the source. This fallback requires a stable source name
/// and can leave both names on error. Other entry types require native support.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdFileSystem;

#[derive(PartialEq, Eq)]
enum ListingMode {
    Browse,
    Strict,
}

fn visit_native_entries(
    entries: impl Iterator<Item = io::Result<std::fs::DirEntry>>,
    mode: ListingMode,
    visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
) -> io::Result<()> {
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        // Windows supplies metadata with the directory listing. Only strict enumeration
        // needs to check that the name still exists. Unix DirEntry::metadata already does so.
        let metadata = if cfg!(windows) && mode == ListingMode::Strict {
            std::fs::symlink_metadata(&path)
        } else {
            entry.metadata()
        };
        let metadata = match metadata {
            Ok(metadata) => metadata,
            Err(e) if mode == ListingMode::Browse && e.kind() == io::ErrorKind::NotFound => {
                continue;
            }
            Err(e) => return Err(e),
        };
        let mut entry = FsEntry::from_metadata(path, metadata);
        if entry.is_symlink
            && let Ok(target) = std::fs::metadata(&entry.path)
        {
            entry.is_dir = target.is_dir();
            entry.size = target.is_file().then_some(target.len());
        }
        if visit(entry) == ScanVisit::Stop {
            break;
        }
    }
    Ok(())
}

impl FileSystem for StdFileSystem {
    fn visit_dir(
        &self,
        dir: &Path,
        visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
    ) -> std::io::Result<()> {
        visit_native_entries(std::fs::read_dir(dir)?, ListingMode::Strict, visit)
    }

    fn visit_dir_for_scan(
        &self,
        dir: &Path,
        visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
    ) -> io::Result<()> {
        visit_native_entries(std::fs::read_dir(dir)?, ListingMode::Browse, visit)
    }

    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }

    fn metadata(&self, path: &Path) -> std::io::Result<FsMetadata> {
        let md = std::fs::metadata(path)?;
        let link_md = std::fs::symlink_metadata(path)?;
        Ok(FsMetadata {
            is_dir: md.is_dir(),
            is_symlink: link_md.is_symlink(),
        })
    }

    fn inspect_entry(&self, path: &Path) -> std::io::Result<FsEntry> {
        Ok(FsEntry::from_metadata(
            path.to_owned(),
            std::fs::symlink_metadata(path)?,
        ))
    }

    #[cfg(unix)]
    fn inspect_move_source(&self, path: &Path) -> io::Result<(FsEntry, Option<FsEntryIdentity>)> {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::symlink_metadata(path)?;
        let id = FsEntryIdentity {
            volume: metadata.dev(),
            file: metadata.ino().into(),
        };
        Ok((FsEntry::from_metadata(path.to_owned(), metadata), Some(id)))
    }

    #[cfg(windows)]
    fn inspect_move_source(&self, path: &Path) -> io::Result<(FsEntry, Option<FsEntryIdentity>)> {
        windows::inspect_move_source(path)
    }

    fn create_dir(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    #[cfg(unix)]
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
        {
            use rustix::fs::{CWD, RenameFlags, renameat_with};
            use rustix::io::Errno;

            match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let cause = io::Error::from(error);
                    if !matches!(error, Errno::NOSYS | Errno::INVAL | Errno::OPNOTSUPP)
                        && cause.kind() != io::ErrorKind::Unsupported
                    {
                        return Err(cause);
                    }
                }
            }
        }
        link_unlink_noreplace(from, to)
    }

    #[cfg(not(any(unix, windows)))]
    fn rename_noreplace(&self, _from: &Path, _to: &Path) -> std::io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace rename is unavailable on this platform",
        ))
    }

    #[cfg(windows)]
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        windows::rename_noreplace(from, to)
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileTypeExt;

            if std::fs::symlink_metadata(path)?
                .file_type()
                .is_symlink_dir()
            {
                return std::fs::remove_dir(path);
            }
        }
        std::fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_dir(path)
    }

    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> std::io::Result<u64> {
        if !std::fs::metadata(from)?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "expected a regular file",
            ));
        }
        std::fs::copy(from, to)
    }
}

#[cfg(unix)]
fn link_unlink_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    use std::fs::{self, Metadata};
    use std::os::unix::fs::MetadataExt;

    let source = fs::symlink_metadata(from)?;
    if !source.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "moving non-regular entries requires atomic no-replace rename",
        ));
    }

    fs::hard_link(from, to)?;
    let remove_source = || {
        let same_file = |metadata: Metadata| {
            metadata.is_file() && metadata.dev() == source.dev() && metadata.ino() == source.ino()
        };
        if !same_file(fs::symlink_metadata(to)?) || !same_file(fs::symlink_metadata(from)?) {
            return Err(io::Error::other(
                "source or destination changed during move",
            ));
        }
        fs::remove_file(from)
    };
    remove_source().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "destination {} was linked but source {} could not be removed: {error}",
                to.display(),
                from.display()
            ),
        )
    })
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;
    use crate::fs_ops::tests::TempDir;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn link_unlink_fallback_preserves_an_occupied_destination_and_moves_to_a_vacant_one() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let missing = temp.path().join("missing");
        fs::write(&source, b"source").unwrap();
        fs::write(&target, b"target").unwrap();

        assert_eq!(
            link_unlink_noreplace(&source, &target).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&target).unwrap(), b"target");
        fs::remove_file(&target).unwrap();
        symlink(&missing, &target).unwrap();
        assert_eq!(
            link_unlink_noreplace(&source, &target).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_link(&target).unwrap(), missing);

        link_unlink_noreplace(&source, &missing).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&missing).unwrap(), b"source");
    }

    #[cfg(unix)]
    #[test]
    fn link_unlink_fallback_leaves_directories_and_symlinks_untouched() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let source = temp.path().join("directory");
        let link = temp.path().join("link");
        let target = temp.path().join("target");
        fs::create_dir(&source).unwrap();
        symlink(&source, &link).unwrap();
        for entry in [&source, &link] {
            assert_eq!(
                link_unlink_noreplace(entry, &target).unwrap_err().kind(),
                io::ErrorKind::Unsupported
            );
        }
        assert!(!target.exists());
        assert!(source.is_dir());
        assert_eq!(fs::read_link(&link).unwrap(), source);
    }

    #[test]
    fn native_scan_and_strict_enumeration_handle_a_vanished_child() {
        let temp = TempDir::new();
        let dir = temp.path();
        for name in ["before", "moving", "survivor"] {
            fs::write(dir.join(name), name).unwrap();
        }
        let entries = || {
            let mut entries = fs::read_dir(dir)
                .unwrap()
                .collect::<io::Result<Vec<_>>>()
                .unwrap();
            entries.sort_by_key(fs::DirEntry::file_name);
            entries.into_iter().map(Ok)
        };
        // Both readers have seen the same names. Rename one before processing either list.
        let scan = entries();
        let transfer = entries();
        fs::rename(dir.join("moving"), dir.join("moved")).unwrap();

        let mut seen = Vec::new();
        visit_native_entries(scan, ListingMode::Browse, &mut |entry| {
            seen.push(entry.name);
            ScanVisit::Continue
        })
        .unwrap();
        #[cfg(windows)]
        assert_eq!(seen, ["before", "moving", "survivor"]);
        #[cfg(unix)]
        assert_eq!(seen, ["before", "survivor"]);

        let mut seen = Vec::new();
        let error = visit_native_entries(transfer, ListingMode::Strict, &mut |entry| {
            seen.push(entry.name);
            ScanVisit::Continue
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(seen, ["before"]);
    }
}
