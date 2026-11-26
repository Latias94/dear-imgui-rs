use std::ffi::{OsStr, OsString};
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};

use crate::dialog_state::ClipboardOp;
use crate::fs::{FileSystem, FsEntry, FsEntryIdentity, ScanVisit, canonical_entry, file_name};

#[derive(Debug, thiserror::Error)]
pub(crate) enum PasteError {
    #[error(transparent)]
    Transfer(#[from] io::Error),
    #[error(transparent)]
    AfterInstall(io::Error),
}

pub(crate) fn entry_exists(fs: &dyn FileSystem, path: &Path) -> io::Result<bool> {
    match fs.inspect_entry(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub(crate) fn delete_entry(fs: &dyn FileSystem, path: &Path, recursive: bool) -> io::Result<()> {
    let entry = fs.inspect_entry(path)?;
    if entry.is_symlink || !entry.is_dir {
        fs.remove_file(path)
    } else if recursive {
        fs.remove_dir_all(path)
    } else {
        fs.remove_dir(path)
    }
}

fn require_vacant(fs: &dyn FileSystem, path: &Path) -> io::Result<()> {
    if entry_exists(fs, path)? {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            format!("target exists: {}", path.display()),
        ));
    }
    Ok(())
}

fn check_disjoint(from: &Path, to: &Path) -> io::Result<()> {
    if from.starts_with(to) || to.starts_with(from) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "source and target overlap: {} and {}",
                from.display(),
                to.display()
            ),
        ));
    }
    Ok(())
}

fn resolve_existing(fs: &dyn FileSystem, path: &Path, entry: &Path) -> io::Result<PathBuf> {
    if fs.inspect_entry(path)?.is_symlink {
        Ok(entry.to_owned())
    } else {
        fs.canonicalize(path)
    }
}

fn validate_target(
    fs: &dyn FileSystem,
    from: &Path,
    to: &Path,
    exists: bool,
) -> io::Result<PathBuf> {
    check_disjoint(from, to)?;
    let source_entry = canonical_entry(fs, from)?;
    let target_entry = canonical_entry(fs, to)?;
    check_disjoint(&source_entry, &target_entry)?;
    let source_path = resolve_existing(fs, from, &source_entry)?;
    let target_path = if exists {
        resolve_existing(fs, to, &target_entry)?
    } else {
        target_entry
    };
    check_disjoint(&source_path, &target_path)?;
    Ok(source_path)
}

pub(crate) fn paste_cancellable(
    fs: &dyn FileSystem,
    op: ClipboardOp,
    from: &Path,
    to: &Path,
    overwrite: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PasteError> {
    check_cancelled(cancelled)?;
    let exists = entry_exists(fs, to)?;
    let source_path = validate_target(fs, from, to, exists)?;
    if exists && !overwrite {
        return Err(Error::from(ErrorKind::AlreadyExists).into());
    }

    check_cancelled(cancelled)?;
    if op == ClipboardOp::Cut && !exists && try_move(fs, from, to)? {
        return Ok(());
    }

    let mut copied = None;
    install_target(fs, to, exists, |incoming| {
        check_cancelled(cancelled)?;
        if op == ClipboardOp::Cut && exists && try_move(fs, from, to)? {
            return Ok(());
        }
        copied = copy_tree(fs, from, incoming, op == ClipboardOp::Cut, cancelled)?;
        check_cancelled(cancelled)?;
        fs.rename_noreplace(incoming, to)
    })?;

    // Every removal follows publication of the complete destination.
    if let Some(root) = copied {
        root.remove_source_tree(fs, &source_path)
            .map_err(PasteError::AfterInstall)?;
    }
    Ok(())
}

fn check_cancelled(cancelled: &dyn Fn() -> bool) -> io::Result<()> {
    if cancelled() {
        Err(Error::new(
            ErrorKind::Interrupted,
            "file operation cancelled",
        ))
    } else {
        Ok(())
    }
}

fn try_move(fs: &dyn FileSystem, from: &Path, to: &Path) -> io::Result<bool> {
    match fs.rename_noreplace(from, to) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => Ok(false),
        Err(e) => Err(e),
    }
}

struct CopiedEntry {
    entry: FsEntry,
    identity: Option<FsEntryIdentity>,
    children: Vec<CopiedEntry>,
}

impl CopiedEntry {
    fn check_source(&self, fs: &dyn FileSystem, path: &Path) -> io::Result<()> {
        let (current, identity) = fs.inspect_move_source(path)?;
        if identity != self.identity
            || current.path != path
            || current.is_symlink
            || current.is_dir != self.entry.is_dir
        {
            return Err(Error::other("source path changed during move"));
        }
        if !current.is_dir
            && (current.size != self.entry.size || current.modified != self.entry.modified)
        {
            return Err(Error::other("source file changed during move"));
        }
        Ok(())
    }

    fn remove_source_tree(&self, fs: &dyn FileSystem, canonical: &Path) -> io::Result<()> {
        let path = &self.entry.path;
        let work = create_work_dir(fs, path).map_err(|e| {
            Error::new(
                e.kind(),
                format!("source cleanup stopped at {}: {e}", path.display()),
            )
        })?;
        let result = self.remove(fs, canonical, &work.join("removing"));
        // A failed restore may leave the only copy of an entry here.
        if let Err(cleanup) = fs.remove_dir(&work) {
            let message = format!("could not clean up {}: {cleanup}", work.display());
            return Err(match result {
                Ok(()) => Error::new(cleanup.kind(), message),
                Err(cause) => Error::new(cause.kind(), format!("{cause}. {message}")),
            });
        }
        result?;
        if entry_exists(fs, path)? {
            return Err(Error::other(format!(
                "source name was recreated during cleanup: {}",
                path.display()
            )));
        }
        Ok(())
    }

    fn remove(&self, fs: &dyn FileSystem, canonical: &Path, holding: &Path) -> io::Result<()> {
        let path = &self.entry.path;
        let context = |e: Error| {
            Error::new(
                e.kind(),
                format!("source cleanup stopped at {}: {e}", path.display()),
            )
        };
        self.check_source(fs, path).map_err(context)?;
        if fs.canonicalize(path).map_err(context)? != canonical {
            return Err(context(Error::other("source path changed during move")));
        }
        for child in &self.children {
            let name = file_name(&child.entry.path)?;
            child.remove(fs, &canonical.join(name), holding)?;
        }

        // Check the object we actually claimed. A new arrival at the original
        // name stays there even if it appeared immediately before the unlink.
        fs.rename_noreplace(path, holding).map_err(context)?;
        let remove = self.check_source(fs, holding).and_then(|()| {
            if self.entry.is_dir {
                fs.remove_dir(holding)
            } else {
                fs.remove_file(holding)
            }
        });
        if let Err(e) = remove {
            if let Err(restore) = fs.rename_noreplace(holding, path) {
                return Err(context(Error::new(
                    e.kind(),
                    format!(
                        "{e}. Could not restore source: {restore}. Entry retained at {}",
                        holding.display()
                    ),
                )));
            }
            return Err(context(e));
        }
        Ok(())
    }
}

fn copy_tree(
    fs: &dyn FileSystem,
    from: &Path,
    to: &Path,
    moving: bool,
    cancelled: &dyn Fn() -> bool,
) -> io::Result<Option<CopiedEntry>> {
    check_cancelled(cancelled)?;
    let (entry, identity) = if moving {
        fs.inspect_move_source(from)?
    } else {
        (fs.inspect_entry(from)?, None)
    };
    if entry.path != from {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "inspection returned a different entry path",
        ));
    }
    reject_copy_link(&entry)?;
    let mut copied_children = Vec::new();
    if entry.is_dir {
        let mut children = Vec::new();
        fs.visit_dir(from, &mut |child| {
            if cancelled() {
                return ScanVisit::Stop;
            }
            children.push(child);
            ScanVisit::Continue
        })?;
        check_cancelled(cancelled)?;
        fs.create_dir(to)?;
        for child in children {
            reject_copy_link(&child)?;
            if child.path.parent() != Some(from) {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "expected an immediate child of the source directory",
                ));
            }
            let child_target = to.join(file_name(&child.path)?);
            if let Some(copied) = copy_tree(fs, &child.path, &child_target, moving, cancelled)? {
                copied_children.push(copied);
            }
        }
    } else {
        if moving && entry.size.is_none() && entry.modified.is_none() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "cannot move {} across filesystems without a file size or modification time",
                    from.display()
                ),
            ));
        }
        // Distinct native names can still collide on the destination filesystem.
        require_vacant(fs, to)?;
        let bytes = fs.copy_file(from, to)?;
        if entry.size.is_some_and(|size| size != bytes) {
            return Err(Error::other(format!(
                "source size changed during copy: {}",
                from.display()
            )));
        }
    }
    Ok(moving.then_some(CopiedEntry {
        entry,
        identity,
        children: copied_children,
    }))
}

fn reject_copy_link(entry: &FsEntry) -> io::Result<()> {
    if entry.is_symlink {
        return Err(Error::new(
            ErrorKind::Unsupported,
            format!(
                "copying links or directory reparse points is unsupported: {}",
                entry.path.display()
            ),
        ));
    }
    Ok(())
}

// Copy in an exclusively created sibling directory. Even a previously vacant
// target can acquire user content while the copy is in progress.
fn install_target(
    fs: &dyn FileSystem,
    to: &Path,
    replace: bool,
    transfer: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), PasteError> {
    let work = create_work_dir(fs, to)?;
    let previous = work.join("previous");
    if replace && let Err(e) = fs.rename_noreplace(to, &previous) {
        let _ = fs.remove_dir(&work);
        return Err(e.into());
    }
    let result = transfer(&work.join("incoming"));
    if let Err(e) = &result
        && replace
        && let Err(restore) = fs.rename_noreplace(&previous, to)
    {
        return Err(Error::new(
            e.kind(),
            format!(
                "{e}. Could not restore {}: {restore}. Previous target retained at {}",
                to.display(),
                previous.display()
            ),
        )
        .into());
    }
    let cleanup = if result.is_ok() && !replace {
        fs.remove_dir(&work)
    } else {
        fs.remove_dir_all(&work)
    };
    if let Err(e) = cleanup {
        let message = format!("could not clean up {}: {e}", work.display());
        return Err(match result {
            Ok(()) => PasteError::AfterInstall(Error::new(e.kind(), message)),
            Err(cause) => Error::new(cause.kind(), format!("{cause}. {message}")).into(),
        });
    }
    result.map_err(Into::into)
}

fn create_work_dir(fs: &dyn FileSystem, to: &Path) -> io::Result<PathBuf> {
    let target_name = file_name(to)?;
    for index in 0..=10_000 {
        let name = format!(".dear-file-browser-paste-{index}");
        if target_name.eq_ignore_ascii_case(OsStr::new(&name)) {
            continue;
        }
        let dir = to.with_file_name(name);
        if entry_exists(fs, &dir)? {
            continue;
        }
        match fs.create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::from(ErrorKind::AlreadyExists))
}

pub(crate) fn keep_both_target(fs: &dyn FileSystem, desired: &Path) -> io::Result<PathBuf> {
    if !entry_exists(fs, desired)? {
        return Ok(desired.to_owned());
    }
    let name = file_name(desired)?;
    let mut base = Path::new(name);
    let mut extensions = Vec::new();
    while let (Some(stem), Some(extension)) = (base.file_stem(), base.extension()) {
        extensions.push(extension);
        base = Path::new(stem);
    }
    let base = base.as_os_str();
    let mut tail = OsString::new();
    for extension in extensions.into_iter().rev() {
        tail.push(".");
        tail.push(extension);
    }
    for i in 1..=10_000 {
        let mut candidate = base.to_owned();
        if i == 1 {
            candidate.push(" (copy)");
        } else {
            candidate.push(format!(" (copy {i})"));
        }
        candidate.push(&tail);
        let target = desired.with_file_name(candidate);
        if !entry_exists(fs, &target)? {
            return Ok(target);
        }
    }
    Err(Error::new(
        ErrorKind::AlreadyExists,
        "failed to find a free target name",
    ))
}

#[cfg(test)]
pub(crate) mod tests;
