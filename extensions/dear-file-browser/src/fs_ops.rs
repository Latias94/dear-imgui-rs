use std::path::Path;

use crate::fs::FileSystem;

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
    use std::path::PathBuf;

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
}
