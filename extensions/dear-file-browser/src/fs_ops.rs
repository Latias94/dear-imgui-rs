use std::path::{Path, PathBuf};

use crate::fs::FileSystem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExistingTargetPolicy {
    Overwrite,
    Skip,
    KeepBoth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExistingTargetDecision {
    Continue(PathBuf),
    Skip,
}

pub(crate) fn copy_tree(fs: &dyn FileSystem, from: &Path, to: &Path) -> std::io::Result<()> {
    let md = fs.metadata(from)?;
    if !md.is_dir {
        fs.copy_file(from, to)?;
        return Ok(());
    }

    fs.create_dir(to)?;
    let entries = fs.read_dir(from)?;
    for e in entries {
        let child_from = e.path;
        let child_to = to.join(&e.name);
        copy_tree(fs, &child_from, &child_to)?;
    }
    Ok(())
}

pub(crate) fn move_tree(fs: &dyn FileSystem, from: &Path, to: &Path) -> std::io::Result<()> {
    match fs.rename(from, to) {
        Ok(()) => return Ok(()),
        Err(_) => {}
    }

    let md = fs.metadata(from)?;
    if md.is_dir {
        copy_tree(fs, from, to)?;
        fs.remove_dir_all(from)?;
        Ok(())
    } else {
        fs.copy_file(from, to)?;
        fs.remove_file(from)?;
        Ok(())
    }
}

pub(crate) fn unique_child_name(
    fs: &dyn FileSystem,
    dir: &Path,
    desired: &str,
) -> std::io::Result<String> {
    if !child_exists(fs, dir, desired)? {
        return Ok(desired.to_string());
    }

    let (base, ext) = split_base_and_full_ext(desired);
    for i in 1usize..=10_000 {
        let suffix = if i == 1 {
            " (copy)".to_string()
        } else {
            format!(" (copy {i})")
        };
        let cand = format!("{base}{suffix}{ext}");
        if !child_exists(fs, dir, &cand)? {
            return Ok(cand);
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to find a free target name",
    ))
}

pub(crate) fn apply_existing_target_policy(
    fs: &dyn FileSystem,
    dest_dir: &Path,
    desired_name: &str,
    policy: ExistingTargetPolicy,
) -> std::io::Result<ExistingTargetDecision> {
    let dest = dest_dir.join(desired_name);
    if !child_exists(fs, dest_dir, desired_name)? {
        return Ok(ExistingTargetDecision::Continue(dest));
    }

    match policy {
        ExistingTargetPolicy::Skip => Ok(ExistingTargetDecision::Skip),
        ExistingTargetPolicy::KeepBoth => {
            let name = unique_child_name(fs, dest_dir, desired_name)?;
            Ok(ExistingTargetDecision::Continue(dest_dir.join(name)))
        }
        ExistingTargetPolicy::Overwrite => {
            remove_existing_path(fs, &dest)?;
            Ok(ExistingTargetDecision::Continue(dest))
        }
    }
}

pub(crate) fn remove_existing_path(fs: &dyn FileSystem, path: &Path) -> std::io::Result<()> {
    match fs.metadata(path) {
        Ok(md) => {
            if md.is_dir {
                fs.remove_dir_all(path)
            } else {
                fs.remove_file(path)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn child_exists(fs: &dyn FileSystem, dir: &Path, name: &str) -> std::io::Result<bool> {
    let p = dir.join(name);
    match fs.metadata(&p) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

fn split_base_and_full_ext(name: &str) -> (&str, &str) {
    if name.starts_with('.') && name[1..].find('.').is_none() {
        return (name, "");
    }
    name.find('.')
        .map(|i| (&name[..i], &name[i..]))
        .unwrap_or((name, ""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::StdFileSystem;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        p.push(format!("dear-file-browser-fs-ops-{prefix}-{pid}-{t}"));
        p
    }

    #[test]
    fn copy_tree_recursively_copies_a_directory() {
        let fs = StdFileSystem;
        let root = unique_temp_dir("copy_tree_dir");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let src = root.join("src");
        let src_nested = src.join("nested");
        std::fs::create_dir_all(&src_nested).unwrap();
        std::fs::write(src.join("a.txt"), b"hello").unwrap();
        std::fs::write(src_nested.join("b.txt"), b"world").unwrap();

        let dst = root.join("dst");
        copy_tree(&fs, &src, &dst).unwrap();

        assert!(dst.join("a.txt").exists());
        assert!(dst.join("nested").join("b.txt").exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn move_tree_falls_back_to_copy_and_delete() {
        let fs = StdFileSystem;
        let root = unique_temp_dir("move_tree_file");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let src = root.join("a.txt");
        let dst = root.join("b.txt");
        std::fs::write(&src, b"hello").unwrap();

        move_tree(&fs, &src, &dst).unwrap();
        assert!(!src.exists());
        assert!(dst.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unique_child_name_preserves_multi_layer_extension() {
        let fs = StdFileSystem;
        let root = unique_temp_dir("unique_name");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let desired = "a.tar.gz";
        std::fs::write(root.join(desired), b"x").unwrap();

        let out = unique_child_name(&fs, &root, desired).unwrap();
        assert!(out.starts_with("a (copy)"));
        assert!(out.ends_with(".tar.gz"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn apply_existing_target_policy_keep_both_allocates_new_name() {
        let fs = StdFileSystem;
        let root = unique_temp_dir("existing_keep_both");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        std::fs::write(root.join("a.txt"), b"x").unwrap();

        let out = apply_existing_target_policy(&fs, &root, "a.txt", ExistingTargetPolicy::KeepBoth)
            .unwrap();

        let ExistingTargetDecision::Continue(p) = out else {
            panic!("expected continue")
        };
        assert_ne!(p, root.join("a.txt"));
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "a (copy).txt");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn apply_existing_target_policy_overwrite_removes_existing() {
        let fs = StdFileSystem;
        let root = unique_temp_dir("existing_overwrite");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let d = root.join("d");
        std::fs::create_dir_all(d.join("nested")).unwrap();
        std::fs::write(d.join("nested").join("x.txt"), b"x").unwrap();

        let out =
            apply_existing_target_policy(&fs, &root, "d", ExistingTargetPolicy::Overwrite).unwrap();

        assert!(matches!(out, ExistingTargetDecision::Continue(_)));
        assert!(!d.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn apply_existing_target_policy_skip_returns_skip() {
        let fs = StdFileSystem;
        let root = unique_temp_dir("existing_skip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        std::fs::write(root.join("a.txt"), b"x").unwrap();

        let out =
            apply_existing_target_policy(&fs, &root, "a.txt", ExistingTargetPolicy::Skip).unwrap();
        assert_eq!(out, ExistingTargetDecision::Skip);

        std::fs::remove_dir_all(&root).unwrap();
    }
}
