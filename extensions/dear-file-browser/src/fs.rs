use std::path::{Path, PathBuf};

/// Minimal file metadata used by the in-UI file browser.
#[derive(Clone, Debug)]
pub struct FsMetadata {
    /// Whether the path refers to a directory.
    pub is_dir: bool,
    /// Whether the path itself is a symbolic link.
    pub is_symlink: bool,
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
    /// Create a new directory. Return [`std::io::ErrorKind::AlreadyExists`]
    /// if any entry occupies the path. Paste uses this to claim its work directory.
    fn create_dir(&self, path: &Path) -> std::io::Result<()>;
    /// Rename/move a path. Return [`std::io::ErrorKind::CrossesDevices`] when a
    /// move requires copying between filesystems.
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    /// Remove a file.
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
            if visit(FsEntry::from_metadata(e.path(), e.metadata()?)) == ScanVisit::Stop {
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

    fn create_dir(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
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
