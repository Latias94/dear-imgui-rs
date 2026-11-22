use std::ffi::{OsStr, OsString};
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};

use crate::dialog_state::ClipboardOp;
use crate::fs::{FileSystem, FsEntry, ScanVisit};

#[derive(Debug, thiserror::Error)]
pub(crate) enum PasteError {
    #[error(transparent)]
    Transfer(#[from] io::Error),
    #[error(transparent)]
    AfterInstall(io::Error),
}

fn file_name(path: &Path) -> io::Result<&OsStr> {
    path.file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "expected a named entry"))
}

pub(crate) fn entry_exists(fs: &dyn FileSystem, path: &Path) -> io::Result<bool> {
    match fs.inspect_entry(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
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

fn canonical_entry(fs: &dyn FileSystem, path: &Path) -> io::Result<PathBuf> {
    let name = file_name(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(fs.canonicalize(parent)?.join(name))
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
    match fs.canonicalize(path) {
        Err(e) if e.kind() == ErrorKind::NotFound && fs.inspect_entry(path)?.is_symlink => {
            Ok(entry.to_owned())
        }
        result => result,
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

pub(crate) fn paste(
    fs: &dyn FileSystem,
    op: ClipboardOp,
    from: &Path,
    to: &Path,
    overwrite: bool,
) -> Result<(), PasteError> {
    let exists = entry_exists(fs, to)?;
    let source_path = validate_target(fs, from, to, exists)?;
    if exists && !overwrite {
        return Err(Error::from(ErrorKind::AlreadyExists).into());
    }

    if op == ClipboardOp::Cut && !exists && try_move(fs, from, to)? {
        return Ok(());
    }

    let mut copied = Vec::new();
    install_target(fs, to, exists, |incoming| {
        if op == ClipboardOp::Cut && exists && try_move(fs, from, to)? {
            return Ok(());
        }
        copy_tree(
            fs,
            from,
            incoming,
            (op == ClipboardOp::Cut).then_some(&mut copied),
        )?;
        require_vacant(fs, to)?;
        fs.rename(incoming, to)
    })?;

    // Every removal follows publication of the complete destination.
    for entry in copied {
        remove_copied_entry(fs, &entry, from, &source_path).map_err(|e| {
            PasteError::AfterInstall(Error::new(
                e.kind(),
                format!("source cleanup stopped at {}: {e}", entry.path.display()),
            ))
        })?;
    }
    Ok(())
}

fn try_move(fs: &dyn FileSystem, from: &Path, to: &Path) -> io::Result<bool> {
    require_vacant(fs, to)?;
    match fs.rename(from, to) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => Ok(false),
        Err(e) => Err(e),
    }
}

fn remove_copied_entry(
    fs: &dyn FileSystem,
    entry: &FsEntry,
    from: &Path,
    source_path: &Path,
) -> io::Result<()> {
    let path = &entry.path;
    let relative = path.strip_prefix(from).map_err(|_| {
        Error::new(
            ErrorKind::InvalidData,
            "copied entry outside the source tree",
        )
    })?;
    let current = fs.inspect_entry(path)?;
    if current.is_symlink
        || current.is_dir != entry.is_dir
        || fs.canonicalize(path)? != source_path.join(relative)
    {
        return Err(Error::other("source path changed during move"));
    }
    if entry.is_dir {
        return fs.remove_dir(path);
    }
    if current.size != entry.size || current.modified != entry.modified {
        return Err(Error::other("source file changed during move"));
    }
    fs.remove_file(path)
}

fn copy_tree(
    fs: &dyn FileSystem,
    from: &Path,
    to: &Path,
    mut move_sources: Option<&mut Vec<FsEntry>>,
) -> io::Result<()> {
    let entry = fs.inspect_entry(from)?;
    if entry.path != from {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "inspection returned a different entry path",
        ));
    }
    reject_copy_link(&entry)?;
    if entry.is_dir {
        let mut children = Vec::new();
        fs.visit_dir(from, &mut |child| {
            children.push(child);
            ScanVisit::Continue
        })?;
        fs.create_dir(to)?;
        for child in children {
            reject_copy_link(&child)?;
            if child.path.parent() != Some(from) {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "expected an immediate child of the source directory",
                ));
            }
            copy_tree(
                fs,
                &child.path,
                &to.join(file_name(&child.path)?),
                move_sources.as_deref_mut(),
            )?;
        }
    } else {
        if move_sources.is_some() && entry.size.is_none() && entry.modified.is_none() {
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
    if let Some(entries) = move_sources {
        entries.push(entry);
    }
    Ok(())
}

fn reject_copy_link(entry: &FsEntry) -> io::Result<()> {
    if entry.is_symlink {
        return Err(Error::new(
            ErrorKind::Unsupported,
            format!(
                "copying symbolic links is unsupported: {}",
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
    if replace && let Err(e) = fs.rename(to, &previous) {
        let _ = fs.remove_dir(&work);
        return Err(e.into());
    }
    let result = transfer(&work.join("incoming"));
    if let Err(e) = &result
        && replace
        && let Err(restore) = require_vacant(fs, to).and_then(|()| fs.rename(&previous, to))
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
    let mut base = base.as_os_str();
    let mut tail = OsString::new();
    if !extensions.is_empty() && base.as_encoded_bytes().starts_with(b".") {
        tail.push(base);
        base = OsStr::new("");
    }
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
