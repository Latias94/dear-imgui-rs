use flate2::read::GzDecoder;
use std::{env, fs, path::Path, path::PathBuf};

#[derive(Clone, Debug)]
struct BuildConfig {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
    target_os: String,
    target_env: String,
    docs_rs: bool,
}

impl BuildConfig {
    fn new() -> Self {
        Self {
            manifest_dir: PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()),
            out_dir: PathBuf::from(env::var("OUT_DIR").unwrap()),
            target_os: env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
            target_env: env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default(),
            docs_rs: env::var("DOCS_RS").is_ok(),
        }
    }
    fn is_msvc(&self) -> bool {
        self.target_env == "msvc"
    }
    fn is_windows(&self) -> bool {
        self.target_os == "windows"
    }
    fn use_static_crt(&self) -> bool {
        self.is_msvc()
            && self.is_windows()
            && env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap_or_default()
                .split(',')
                .any(|f| f == "crt-static")
    }
}

fn resolve_imgui_includes(cfg: &BuildConfig) -> (PathBuf, PathBuf) {
    // Prefer paths exported by dear-imgui-sys build script (prefix comes from links = "dear-imgui")
    let imgui_src = env::var_os("DEP_DEAR_IMGUI_IMGUI_INCLUDE_PATH")
        .or_else(|| env::var_os("DEP_DEAR_IMGUI_THIRD_PARTY"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            cfg.manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui/imgui")
        });
    let cimgui_root = env::var_os("DEP_DEAR_IMGUI_CIMGUI_INCLUDE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            cfg.manifest_dir
                .join("../../dear-imgui-sys/third-party/cimgui")
        });
    (imgui_src, cimgui_root)
}

fn use_pregenerated_bindings(out_dir: &Path) -> bool {
    let preg = Path::new("src").join("bindings_pregenerated.rs");
    if preg.exists() {
        match std::fs::read_to_string(&preg).and_then(|content| {
            let sanitized = sanitize_bindings_string(&content);
            std::fs::write(out_dir.join("bindings.rs"), sanitized)
        }) {
            Ok(()) => {
                println!(
                    "cargo:warning=Using pregenerated bindings: {}",
                    preg.display()
                );
                true
            }
            Err(e) => {
                println!("cargo:warning=Failed to write pregenerated bindings: {}", e);
                false
            }
        }
    } else {
        false
    }
}

fn sanitize_bindings_file(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let sanitized = sanitize_bindings_string(&content);
        let _ = std::fs::write(path, sanitized);
    }
}

fn sanitize_bindings_string(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut skip_next_blank = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#![") {
            skip_next_blank = true;
            continue;
        }
        if skip_next_blank {
            if trimmed.is_empty() {
                continue;
            }
            skip_next_blank = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn docsrs_build(cfg: &BuildConfig, cimguizmo_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    println!("cargo:warning=DOCS_RS detected: generating bindings, skipping native build");
    println!("cargo:rustc-cfg=docsrs");

    if use_pregenerated_bindings(&cfg.out_dir) {
        return;
    }

    // Fallback: try to generate bindings from headers if available
    if !imgui_src.exists() || !cimgui_root.exists() || !cimguizmo_root.exists() {
        panic!(
            "DOCS_RS build: Required headers not found and no pregenerated bindings present.\n\
             Please add src/bindings_pregenerated.rs (full bindgen output) to enable docs.rs builds.\n\
             Run: cargo build -p dear-imguizmo-sys && cp target/debug/build/dear-imguizmo-sys-*/out/bindings.rs extensions/dear-imguizmo-sys/src/bindings_pregenerated.rs"
        );
    }

    let bindings = bindgen::Builder::default()
        .header(cimguizmo_root.join("cimguizmo.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("ImGuizmo_.*")
        .allowlist_function("Style_.*")
        .allowlist_type("(Style|COLOR|MODE|OPERATION)")
        .allowlist_var("(COLOR|MODE|OPERATION|COUNT|TRANSLATE.*|ROTATE.*|SCALE.*|UNIVERSAL)")
        .blocklist_type("ImVec2")
        .blocklist_type("ImVec4")
        .blocklist_type("ImGuiContext")
        .blocklist_type("ImDrawList")
        .blocklist_type("ImGuiID")
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg(format!("-I{}", cimguizmo_root.display()))
        .clang_arg(format!("-I{}", cimguizmo_root.join("ImGuizmo").display()))
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17")
        .generate()
        .expect("Unable to generate cimguizmo bindings");
    let out = cfg.out_dir.join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("Couldn't write cimguizmo bindings!");
    sanitize_bindings_file(&out);
}

fn try_link_prebuilt_all(cfg: &BuildConfig) -> bool {
    let target_env = &cfg.target_env;
    if let Ok(dir) = env::var("IMGUIZMO_SYS_LIB_DIR") {
        if try_link_prebuilt(PathBuf::from(dir.clone()), target_env) {
            return true;
        }
        println!(
            "cargo:warning=IMGUIZMO_SYS_LIB_DIR set but library not found in {}",
            dir
        );
    }
    if let Ok(url) = env::var("IMGUIZMO_SYS_PREBUILT_URL") {
        let cache_root = prebuilt_cache_root(cfg);
        if let Ok(dir) = try_download_prebuilt(&cache_root, &url, target_env)
            && try_link_prebuilt(dir.clone(), target_env)
        {
            return true;
        }
    } else if let Some(dir) = try_download_prebuilt_from_release(cfg)
        && try_link_prebuilt(dir.clone(), target_env)
    {
        return true;
    }
    false
}

fn build_with_cc(cfg: &BuildConfig, cimguizmo_root: &Path, imgui_src: &Path, cimgui_root: &Path) {
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17");
    for (k, v) in env::vars() {
        if let Some(suffix) = k.strip_prefix("DEP_DEAR_IMGUI_DEFINE_") {
            build.define(suffix, v.as_str());
        }
    }
    build.include(imgui_src);
    build.include(cimgui_root);
    build.include(cimguizmo_root);
    build.include(cimguizmo_root.join("ImGuizmo"));
    build.file(cimguizmo_root.join("cimguizmo.cpp"));
    build.file(cimguizmo_root.join("ImGuizmo/ImGuizmo.cpp"));

    if cfg.is_msvc() && cfg.is_windows() {
        build.flag("/EHsc");
        let use_static = cfg.use_static_crt();
        build.static_crt(use_static);
        if use_static {
            build.flag("/MT");
        } else {
            build.flag("/MD");
        }
        let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
        if profile == "debug" {
            build.debug(true).opt_level(0);
        } else {
            build.debug(false).opt_level(2);
        }
        build.flag("/D_ITERATOR_DEBUG_LEVEL=0");
    }
    build.compile("dear_imguizmo");
}

fn main() {
    let cfg = BuildConfig::new();

    // Rerun hints
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third-party/cimguizmo/cimguizmo.h");
    println!("cargo:rerun-if-changed=third-party/cimguizmo/cimguizmo.cpp");
    println!("cargo:rerun-if-changed=third-party/cimguizmo/ImGuizmo/ImGuizmo.cpp");
    println!("cargo:rerun-if-changed=../../dear-imgui-sys");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_SKIP_CC");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_FORCE_BUILD");
    println!("cargo:rerun-if-env-changed=IMGUIZMO_SYS_USE_CMAKE");

    let (imgui_src, cimgui_root) = resolve_imgui_includes(&cfg);
    let cimguizmo_root = cfg.manifest_dir.join("third-party/cimguizmo");
    if !imgui_src.exists() {
        panic!("ImGui include not found at {:?}", imgui_src);
    }
    if !cimgui_root.exists() {
        panic!("cimgui root not found at {:?}", cimgui_root);
    }
    if !cimguizmo_root.exists() {
        panic!(
            "cimguizmo root not found at {:?}. Did you init submodules?",
            cimguizmo_root
        );
    }

    // Generate bindings
    docsrs_build(&cfg, &cimguizmo_root, &imgui_src, &cimgui_root);

    // Link/build native
    let force_build =
        cfg!(feature = "build-from-source") || env::var("IMGUIZMO_SYS_FORCE_BUILD").is_ok();
    let linked_prebuilt = if force_build {
        false
    } else {
        try_link_prebuilt_all(&cfg)
    };
    if !cfg.docs_rs && !linked_prebuilt && env::var("IMGUIZMO_SYS_SKIP_CC").is_err() {
        build_with_cc(&cfg, &cimguizmo_root, &imgui_src, &cimgui_root);
    } else if cfg.docs_rs {
        docsrs_build(&cfg, &cimguizmo_root, &imgui_src, &cimgui_root);
    }
}

fn expected_lib_name(target_env: &str) -> &'static str {
    if target_env == "msvc" {
        "dear_imguizmo.lib"
    } else {
        "libdear_imguizmo.a"
    }
}

fn try_link_prebuilt(dir: PathBuf, target_env: &str) -> bool {
    let lib_name = expected_lib_name(target_env);
    let lib_path = dir.join(lib_name);
    if !lib_path.exists() {
        return false;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=dear_imguizmo");
    true
}

fn try_download_prebuilt(
    cache_root: &Path,
    url: &str,
    target_env: &str,
) -> Result<PathBuf, String> {
    let lib_name = expected_lib_name(target_env);
    let dl_dir = cache_root.join("download");
    let _ = fs::create_dir_all(&dl_dir);
    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        let fname = url.split('/').next_back().unwrap_or("prebuilt.tar.gz");
        let archive_path = dl_dir.join(fname);
        if !archive_path.exists() {
            println!(
                "cargo:warning=Downloading prebuilt dear_imguizmo archive from {}",
                url
            );
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| format!("http client: {}", e))?;
            let resp = client
                .get(url)
                .send()
                .map_err(|e| format!("http get: {}", e))?;
            if !resp.status().is_success() {
                return Err(format!("http status {}", resp.status()));
            }
            let bytes = resp.bytes().map_err(|e| format!("read body: {}", e))?;
            fs::write(&archive_path, &bytes)
                .map_err(|e| format!("write {}: {}", archive_path.display(), e))?;
        }
        let extract_dir = prebuilt_extract_dir_env(cache_root, target_env);
        if extract_dir.exists() {
            let _ = std::fs::remove_dir_all(&extract_dir);
        }
        fs::create_dir_all(&extract_dir)
            .map_err(|e| format!("create dir {}: {}", extract_dir.display(), e))?;
        let file = fs::File::open(&archive_path)
            .map_err(|e| format!("open {}: {}", archive_path.display(), e))?;
        let mut archive = tar::Archive::new(GzDecoder::new(file));
        archive
            .unpack(&extract_dir)
            .map_err(|e| format!("unpack {}: {}", archive_path.display(), e))?;
        let lib_dir = extract_dir.join("lib");
        if lib_dir.join(lib_name).exists() {
            return Ok(lib_dir);
        }
        if extract_dir.join(lib_name).exists() {
            return Ok(extract_dir);
        }
        return Err("extracted archive did not contain expected library".into());
    }

    let dst = dl_dir.join(lib_name);
    if dst.exists() {
        return Ok(dl_dir);
    }
    println!(
        "cargo:warning=Downloading prebuilt dear_imguizmo from {}",
        url
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("http get: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("http status {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("read body: {}", e))?;
    fs::write(&dst, &bytes).map_err(|e| format!("write {}: {}", dst.display(), e))?;
    Ok(dl_dir)
}

fn try_download_prebuilt_from_release(cfg: &BuildConfig) -> Option<PathBuf> {
    let offline = matches!(
        env::var("CARGO_NET_OFFLINE").ok().as_deref(),
        Some("true") | Some("1") | Some("yes")
    );
    if offline {
        return None;
    }

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let link_type = "static";
    let crt_suffix = if cfg.is_windows() && cfg.is_msvc() {
        if cfg.use_static_crt() { "-mt" } else { "-md" }
    } else {
        ""
    };
    let archive_name = format!(
        "dear-imguizmo-prebuilt-{}-{}-{}{}.tar.gz",
        version,
        env::var("TARGET").unwrap_or_default(),
        link_type,
        crt_suffix
    );
    let archive_name_no_crt = format!(
        "dear-imguizmo-prebuilt-{}-{}-{}.tar.gz",
        version,
        env::var("TARGET").unwrap_or_default(),
        link_type
    );
    let tags = [
        format!("dear-imguizmo-sys-v{}", version),
        format!("v{}", version),
    ];
    if let Ok(pkg_dir) = env::var("IMGUIZMO_SYS_PACKAGE_DIR") {
        let pkg_dir = PathBuf::from(pkg_dir);
        for cand in [archive_name.clone(), archive_name_no_crt.clone()] {
            let archive_path = pkg_dir.join(&cand);
            if archive_path.exists() {
                let cache_root = prebuilt_cache_root(cfg);
                if let Ok(lib_dir) = extract_archive_to_cache(
                    &archive_path,
                    &cache_root,
                    expected_lib_name(&cfg.target_env),
                ) {
                    return Some(lib_dir);
                }
            }
        }
    }
    let cache_root = prebuilt_cache_root(cfg);
    for tag in &tags {
        for name in [&archive_name, &archive_name_no_crt] {
            let url = format!(
                "https://github.com/Latias94/dear-imgui/releases/download/{}/{}",
                tag, name
            );
            if let Ok(lib_dir) = try_download_prebuilt(&cache_root, &url, &cfg.target_env) {
                return Some(lib_dir);
            }
        }
    }
    None
}

fn prebuilt_cache_root(cfg: &BuildConfig) -> PathBuf {
    if let Ok(dir) = env::var("IMGUIZMO_SYS_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| cfg.manifest_dir.parent().unwrap().join("target"));
    target_dir.join("dear-imguizmo-prebuilt")
}

fn prebuilt_extract_dir_env(cache_root: &Path, target_env: &str) -> PathBuf {
    let target = env::var("TARGET").unwrap_or_default();
    let crt_suffix = if target_env == "msvc" {
        let tf = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
        if tf.split(',').any(|f| f == "crt-static") {
            "-mt"
        } else {
            "-md"
        }
    } else {
        ""
    };
    cache_root
        .join(target)
        .join(format!("static{}", crt_suffix))
}

fn extract_archive_to_cache(
    archive_path: &Path,
    cache_root: &Path,
    lib_name: &str,
) -> Result<PathBuf, String> {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let extract_dir = prebuilt_extract_dir_env(cache_root, &target_env);
    if extract_dir.exists() {
        let lib_dir = extract_dir.join("lib");
        if lib_dir.join(lib_name).exists() || extract_dir.join(lib_name).exists() {
            return Ok(lib_dir);
        }
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("create dir {}: {}", extract_dir.display(), e))?;
    let file = fs::File::open(archive_path)
        .map_err(|e| format!("open {}: {}", archive_path.display(), e))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    archive
        .unpack(&extract_dir)
        .map_err(|e| format!("unpack {}: {}", archive_path.display(), e))?;
    let lib_dir = extract_dir.join("lib");
    if lib_dir.join(lib_name).exists() {
        return Ok(lib_dir);
    }
    if extract_dir.join(lib_name).exists() {
        return Ok(extract_dir);
    }
    Err("extracted archive did not contain expected library".into())
}
