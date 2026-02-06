use std::path::{Path, PathBuf};

/// Minimal file metadata used by the in-UI file browser.
#[derive(Clone, Debug)]
pub struct FsMetadata {
    /// Whether the path refers to a directory.
    pub is_dir: bool,
    /// Whether the path itself is a symbolic link.
    pub is_symlink: bool,
}

/// Directory entry returned by [`FileSystem::read_dir`].
#[derive(Clone, Debug)]
pub struct FsEntry {
    /// Base name (no parent path)
    pub name: String,
    /// Full path
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

/// File system abstraction (IGFD `IFileSystem`-like).
///
/// This is a first incremental step; the API will expand as Places/devices,
/// directory creation, symlink support, and async enumeration are added.
pub trait FileSystem {
    /// List entries of a directory.
    fn read_dir(&self, dir: &Path) -> std::io::Result<Vec<FsEntry>>;
    /// Canonicalize a path (best-effort absolute normalization).
    fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf>;
    /// Fetch minimal metadata for a path.
    fn metadata(&self, path: &Path) -> std::io::Result<FsMetadata>;
    /// Create a directory.
    fn create_dir(&self, path: &Path) -> std::io::Result<()>;
    /// Rename/move a path.
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    /// Remove a file.
    fn remove_file(&self, path: &Path) -> std::io::Result<()>;
    /// Remove an empty directory.
    fn remove_dir(&self, path: &Path) -> std::io::Result<()>;
    /// Remove a directory and all of its contents (recursive).
    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()>;
    /// Copy a file.
    ///
    /// Returns the number of bytes copied (mirrors `std::fs::copy`).
    fn copy_file(&self, from: &Path, to: &Path) -> std::io::Result<u64>;
}

/// Default filesystem implementation using `std::fs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn read_dir(&self, dir: &Path) -> std::io::Result<Vec<FsEntry>> {
        let mut out = Vec::new();
        let rd = std::fs::read_dir(dir)?;
        for e in rd {
            let e = match e {
                Ok(v) => v,
                Err(_) => continue,
            };
            let ft = match e.file_type() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let name = e.file_name().to_string_lossy().to_string();
            let path = e.path();
            let meta = e.metadata().ok();
            let modified = meta.as_ref().and_then(|m| m.modified().ok());
            let is_dir = ft.is_dir();
            let is_symlink = ft.is_symlink();
            let size = if is_dir {
                None
            } else {
                meta.as_ref().filter(|m| m.is_file()).map(|m| m.len())
            };
            out.push(FsEntry {
                name,
                path,
                is_dir,
                is_symlink,
                size,
                modified,
            });
        }
        Ok(out)
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
        std::fs::copy(from, to)
    }
}
