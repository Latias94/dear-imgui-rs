use std::path::PathBuf;

use dear_file_browser::{FileSystem, StdFileSystem};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    p.push(format!("dear-file-browser-{prefix}-{pid}-{t}"));
    p
}

#[test]
fn std_fs_rename_and_remove_file() {
    let fs = StdFileSystem;
    let dir = unique_temp_dir("fs_ops");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let from = dir.join("a.txt");
    let to = dir.join("b.txt");
    std::fs::write(&from, b"hello").unwrap();

    fs.rename(&from, &to).unwrap();
    assert!(!from.exists());
    assert!(to.exists());

    fs.remove_file(&to).unwrap();
    assert!(!to.exists());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn std_fs_remove_dir() {
    let fs = StdFileSystem;
    let dir = unique_temp_dir("remove_dir");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let sub = dir.join("empty");
    std::fs::create_dir(&sub).unwrap();

    fs.remove_dir(&sub).unwrap();
    assert!(!sub.exists());

    std::fs::remove_dir_all(&dir).unwrap();
}
