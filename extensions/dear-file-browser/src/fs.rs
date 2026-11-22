use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod windows;

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
    /// Whether the path itself is a symbolic link.
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
    /// Whether this entry itself is a symbolic link.
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
            is_symlink: metadata.file_type().is_symlink(),
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
    /// Visit entries of a directory one at a time.
    ///
    /// Implementations must stop when `visit` returns [`ScanVisit::Stop`] and
    /// should return promptly afterward. The visitor is invoked on the scan
    /// worker for background scans and on the caller thread for
    /// [`crate::ScanPolicy::Blocking`] scans. Destroying a background dialog
    /// waits for its worker to return after cancellation. Return an error if an
    /// entry cannot be enumerated or classified, since callers also use this
    /// method to copy directory contents.
    fn visit_dir(
        &self,
        dir: &Path,
        visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
    ) -> std::io::Result<()>;
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
    /// Remove a file or the symbolic link itself, including a link to a directory.
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

/// Default filesystem implementation using `std::fs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn visit_dir(
        &self,
        dir: &Path,
        visit: &mut dyn FnMut(FsEntry) -> ScanVisit,
    ) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let mut entry = FsEntry::from_metadata(e.path(), e.metadata()?);
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

    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }

    fn metadata(&self, path: &Path) -> std::io::Result<FsMetadata> {
        let md = std::fs::metadata(path)?;
        let link_md = std::fs::symlink_metadata(path)?;
        Ok(FsMetadata {
            is_dir: md.is_dir(),
            is_symlink: link_md.file_type().is_symlink(),
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

    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        use rustix::fs::{CWD, RenameFlags, renameat_with};

        Ok(renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE)?)
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
