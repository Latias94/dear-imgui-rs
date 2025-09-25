use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("wasm-bindgen");
    match cmd {
        "wasm-bindgen" => gen_wasm_bindings(args.get(1).map(|s| s.as_str()))?,
        "web-demo" => build_web_demo()?,
        "build-cimgui-provider" => build_cimgui_provider()?,
        _ => {
            eprintln!(
                "Unknown command: {}\nCommands:\n  wasm-bindgen [import_mod]\n  web-demo\n  build-cimgui-provider",
                cmd
            );
        }
    }
    Ok(())
}

fn gen_wasm_bindings(import_mod: Option<&str>) -> Result<()> {
    let root = project_root();
    let sys_root = root.join("dear-imgui-sys");
    let cimgui_root = sys_root.join("third-party").join("cimgui");
    let imgui_src = cimgui_root.join("imgui");
    let header = cimgui_root.join("cimgui.h");
    let out = sys_root.join("src").join("wasm_bindings_pregenerated.rs");
    let import_name = import_mod.unwrap_or("imgui-sys-v0");

    // Configure bindgen similar to build.rs, but target wasm imports
    let mut builder = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .clang_arg(format!("-I{}", cimgui_root.display()))
        .clang_arg(format!("-I{}", imgui_src.display()))
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS")
        .allowlist_function("ig.*")
        .allowlist_function("Im.*")
        .allowlist_type("Im.*")
        .allowlist_var("Im.*")
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .derive_eq(true)
        .derive_partialeq(true)
        .derive_hash(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .wasm_import_module_name(import_name);

    // WASM-friendly: disable platform/file functions in C++ headers
    builder = builder
        .clang_arg("-DIMGUI_DISABLE_FILE_FUNCTIONS")
        .clang_arg("-DIMGUI_DISABLE_OSX_FUNCTIONS")
        .clang_arg("-DIMGUI_DISABLE_WIN32_FUNCTIONS");

    eprintln!(
        "Generating wasm bindings to {} (import module: {})",
        out.display(),
        import_name
    );
    let bindings = builder.generate().context("bindgen generate() failed")?;
    bindings
        .write_to_file(&out)
        .with_context(|| format!("write bindings to {}", out.display()))?;
    eprintln!("Done.");
    Ok(())
}

fn build_web_demo() -> Result<()> {
    use std::fs;
    use std::process::Command;

    let root = project_root();
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());

    // 1) Build the web demo crate for wasm32-unknown-unknown
    eprintln!("Building dear-imgui-web-demo ({profile})...");
    let mut build_cmd = Command::new("cargo");
    build_cmd.env(
        "RUSTFLAGS",
        "-C link-arg=--import-memory -C link-arg=--export-table",
    );
    let status = build_cmd
        .args([
            "build",
            "-p",
            "dear-imgui-web-demo",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("cargo build failed");
    }

    // 2) Run wasm-bindgen on the produced wasm
    // Package name as declared in examples-wasm/Cargo.toml
    let pkg_name = "dear-imgui-web-demo";
    // Rust artifact file stem uses underscores
    let wasm_name = pkg_name.replace('-', "_");
    let wasm_path = root
        .join("target")
        .join("wasm32-unknown-unknown")
        .join(&profile)
        .join(format!("{}.wasm", wasm_name));
    if !wasm_path.exists() {
        anyhow::bail!("wasm artifact not found: {}", wasm_path.display());
    }

    let dist = root.join("target").join("web-demo");
    let _ = fs::create_dir_all(&dist);

    eprintln!("Running wasm-bindgen -> {}", dist.display());
    let status = Command::new("wasm-bindgen")
        .args([
            "--target",
            "web",
            "--no-typescript",
            "--out-name",
            pkg_name,
            "--out-dir",
            dist.to_str().unwrap(),
            wasm_path.to_str().unwrap(),
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("wasm-bindgen failed (install via `cargo install -f wasm-bindgen-cli`)");
    }

    // 3) Copy demo index.html
    let index_src = root.join("examples-wasm/web/index.html");
    let index_dst = dist.join("index.html");
    fs::copy(&index_src, &index_dst)?;

    // 3b) Patch generated JS to wire shared memory into wasm-bindgen init
    let js_main = dist.join(format!("{}.js", pkg_name));
    if js_main.exists() {
        let mut code = fs::read_to_string(&js_main)?;
        // Ensure a global 'memory' binding referencing our shared memory exists.
        if !code.contains("globalThis.__imgui_shared_memory") {
            // Insert right after the first import line
            if let Some(idx) = code.find("\nlet wasm;") {
                let inject = "\nconst memory = globalThis.__imgui_shared_memory || new WebAssembly.Memory({ initial: 256, maximum: 4096 });\n";
                code.insert_str(idx, inject);
                fs::write(&js_main, &code)?;
                eprintln!("Injected shared memory binding into {}", js_main.display());
            }
        }
    }

    eprintln!("Web demo built at: {}", dist.display());
    eprintln!(
        "Serve this dir via any static server, e.g.\n  python -m http.server -d {} 8080",
        dist.display()
    );
    eprintln!("Note: Runtime requires the 'imgui-sys-v0' module (cimgui) to be provided.\n      Without it, the page will fail to instantiate the WASM module.");
    Ok(())
}

fn find_emsdk_tools() -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    // 1) Prefer PATH discovery (works if user ran emsdk_env or added to PATH)
    let which = |name: &str| which::which(name).ok();
    if let (Some(empp), Some(emcc)) = (which("em++"), which("emcc")) {
        let emscripten_dir = emcc.parent().unwrap().to_path_buf();
        let em_config = emscripten_dir.join(".emscripten");
        return Ok((empp, emcc, em_config));
    }

    // 2) Fallback to EMSDK env var if provided
    if let Ok(root) = std::env::var("EMSDK") {
        let root = std::path::PathBuf::from(root);
        let emscripten = root.join("upstream").join("emscripten");
        let empp = if cfg!(windows) {
            emscripten.join("em++.bat")
        } else {
            emscripten.join("em++")
        };
        let emcc = if cfg!(windows) {
            emscripten.join("emcc.bat")
        } else {
            emscripten.join("emcc")
        };
        let em_config = emscripten.join(".emscripten");
        if empp.exists() && emcc.exists() {
            return Ok((empp, emcc, em_config));
        }
    }

    anyhow::bail!("Failed to locate emsdk tools (em++/emcc). On Windows, run emsdk_env.bat (or emsdk_env.ps1) to add them to PATH, or set EMSDK env var to your emsdk root.")
}

fn build_cimgui_provider() -> Result<()> {
    use std::fs;
    use std::process::Command;

    let root = project_root();
    let out_dir = root.join("target").join("web-demo");
    fs::create_dir_all(&out_dir)?;

    // 1) Locate emsdk tools and ensure emscripten config exists
    let (empp, emcc, em_config) = find_emsdk_tools()?;
    if !em_config.exists() {
        eprintln!("Generating Emscripten config at {}", em_config.display());
        let status = Command::new(&emcc)
            .arg("--generate-config")
            .arg(&em_config)
            .status()?;
        if !status.success() {
            anyhow::bail!("emcc --generate-config failed");
        }
    }

    // 2) Compose em++ command to build imgui-sys-v0.wasm (shared imported memory)
    let sys_root = root.join("dear-imgui-sys");
    let cimgui_root = sys_root.join("third-party").join("cimgui");
    let imgui_src = cimgui_root.join("imgui");

    let out_wasm = out_dir.join("imgui-sys-v0.wasm");

    let mut cmd = Command::new(&empp);
    cmd.arg("-std=c++17")
        .arg("-O2")
        .arg("--no-entry")
        .arg("-s")
        .arg("STANDALONE_WASM=1")
        .arg("-s")
        .arg("ENVIRONMENT=web")
        .arg("-s")
        .arg("IMPORTED_MEMORY=1")
        .arg("-s")
        .arg("ALLOW_MEMORY_GROWTH=1")
        .arg("-s")
        .arg("INITIAL_MEMORY=16777216")
        .arg("-s")
        .arg("FILESYSTEM=0")
        .arg("-s")
        .arg("EXPORT_ALL=1")
        .arg("-fno-exceptions")
        .arg("-fno-rtti")
        .arg("-DIMGUI_DISABLE_FILE_FUNCTIONS")
        .arg("-DIMGUI_DISABLE_OSX_FUNCTIONS")
        .arg("-DIMGUI_DISABLE_WIN32_FUNCTIONS")
        .arg("-DIMGUI_USE_WCHAR32")
        .arg("-I")
        .arg(&cimgui_root)
        .arg("-I")
        .arg(&imgui_src)
        .arg(cimgui_root.join("cimgui.cpp"))
        .arg(imgui_src.join("imgui.cpp"))
        .arg(imgui_src.join("imgui_draw.cpp"))
        .arg(imgui_src.join("imgui_widgets.cpp"))
        .arg(imgui_src.join("imgui_tables.cpp"))
        .arg(imgui_src.join("imgui_demo.cpp"))
        .arg("-o")
        .arg(&out_wasm);

    // Ensure tools can find config and binaries
    let emscripten_dir = emcc.parent().unwrap();
    let tool_bin = emscripten_dir.parent().unwrap().join("bin");
    let path = std::env::var_os("PATH").unwrap_or_default();
    let new_path = {
        let mut p = std::env::split_paths(&path).collect::<Vec<_>>();
        p.insert(0, emscripten_dir.to_path_buf());
        p.insert(0, tool_bin);
        std::env::join_paths(p).unwrap()
    };
    cmd.env("PATH", new_path);
    cmd.env("EM_CONFIG", &em_config);

    eprintln!("Building cimgui provider -> {}", out_wasm.display());
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("em++ failed; see output for details");
    }

    // 3) Generate ES module glue that re-exports wasm instance exports under static names
    let bindings = sys_root.join("src").join("wasm_bindings_pregenerated.rs");
    let content =
        fs::read_to_string(&bindings).with_context(|| format!("read {}", bindings.display()))?;
    let mut names = std::collections::BTreeSet::new();
    for line in content.lines() {
        // very simple extractor for `pub fn <name>(` lines
        if let Some(i) = line.find("pub fn ") {
            let rest = &line[i + 7..];
            if let Some(j) = rest.find('(') {
                let name = rest[..j].trim();
                // keep plausible cimgui symbols
                if name.starts_with("ig") || name.starts_with("Im") {
                    names.insert(name.to_string());
                }
            }
        }
    }
    // Ensure provider wasm exports all symbols required by rust imports
    // Generate an exports list for Emscripten: EXPORTED_FUNCTIONS=["_igTextUnformatted", ...]
    let mut exported: Vec<String> = names.iter().map(|n| format!("_{}", n)).collect();
    exported.sort();
    let exports_json = format!(
        "[{}]",
        exported
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(",")
    );
    let exports_path = out_dir.join("imgui_exports.json");
    fs::write(&exports_path, &exports_json)?;

    // Rebuild the command with export list (place before -o)
    let mut cmd = Command::new(&empp);
    cmd.arg("-std=c++17")
        .arg("-O2")
        .arg("--no-entry")
        .arg("-s")
        .arg("STANDALONE_WASM=1")
        .arg("-s")
        .arg("ENVIRONMENT=web")
        .arg("-s")
        .arg("IMPORTED_MEMORY=1")
        .arg("-s")
        .arg("ALLOW_MEMORY_GROWTH=1")
        .arg("-s")
        .arg("INITIAL_MEMORY=16777216")
        .arg("-s")
        .arg("FILESYSTEM=0")
        .arg("-s")
        .arg("EXPORT_ALL=1")
        .arg("-s")
        .arg(format!("EXPORTED_FUNCTIONS=@{}", exports_path.display()))
        .arg("-fno-exceptions")
        .arg("-fno-rtti")
        .arg("-DIMGUI_DISABLE_FILE_FUNCTIONS")
        .arg("-DIMGUI_DISABLE_OSX_FUNCTIONS")
        .arg("-DIMGUI_DISABLE_WIN32_FUNCTIONS")
        .arg("-DIMGUI_USE_WCHAR32")
        .arg("-I")
        .arg(&cimgui_root)
        .arg("-I")
        .arg(&imgui_src)
        .arg(cimgui_root.join("cimgui.cpp"))
        .arg(imgui_src.join("imgui.cpp"))
        .arg(imgui_src.join("imgui_draw.cpp"))
        .arg(imgui_src.join("imgui_widgets.cpp"))
        .arg(imgui_src.join("imgui_tables.cpp"))
        .arg(imgui_src.join("imgui_demo.cpp"))
        .arg("-o")
        .arg(&out_wasm);

    // Write module with top-level await
    // Use .js extension instead of .mjs for broader server MIME compatibility
    let js_path = out_dir.join("imgui-sys-v0.js");
    let mut js = String::new();
    js.push_str("// Auto-generated imgui-sys-v0 provider.\n");
    js.push_str("async function loadWasm(url, imports) {\n");
    js.push_str("  try {\n");
    js.push_str("    return await WebAssembly.instantiateStreaming(fetch(url), imports);\n");
    js.push_str("  } catch (_) {\n");
    js.push_str("    const resp = await fetch(url);\n");
    js.push_str("    const bytes = await resp.arrayBuffer();\n");
    js.push_str("    return await WebAssembly.instantiate(bytes, imports);\n");
    js.push_str("  }\n");
    js.push_str("}\n");
    // Minimal import stubs for Emscripten standalone WASM (WASI-like)
    js.push_str("const __return0 = (..._args) => 0;\n");
    js.push_str("const __noop = (..._args) => {};\n");
    js.push_str("const wasi = new Proxy({}, { get: (_t, prop) => {\n");
    js.push_str("  if (prop === 'proc_exit') return (code) => { console.warn('[wasi] proc_exit', code); return 0; };\n");
    js.push_str("  if (prop === 'fd_write') return __return0;\n");
    js.push_str("  if (prop === 'fd_seek') return __return0;\n");
    js.push_str("  if (prop === 'environ_sizes_get') return __return0;\n");
    js.push_str("  if (prop === 'environ_get') return __return0;\n");
    js.push_str("  if (prop === 'clock_time_get') return __return0;\n");
    js.push_str("  if (prop === 'random_get') return __return0;\n");
    js.push_str("  return __return0;\n");
    js.push_str("}});\n");
    js.push_str("const env = new Proxy({}, { get: (_t, prop) => {\n");
    js.push_str("  if (prop === 'abort') return () => { throw new Error('abort'); };\n");
    js.push_str("  if (prop === 'emscripten_notify_memory_growth') return __noop;\n");
    js.push_str("  if (prop === 'emscripten_memcpy_big') return __return0;\n");
    js.push_str("  return __return0;\n");
    js.push_str("}});\n");
    js.push_str("const emsc = new Proxy({}, { get: () => __return0 });\n");
    js.push_str("const memory = globalThis.__imgui_shared_memory || new WebAssembly.Memory({initial:256, maximum:4096});\n");
    js.push_str("const imports = { wasi_snapshot_preview1: wasi, wasi_unstable: wasi, env: Object.assign({ memory }, env), emsc };\n");
    js.push_str("const { instance } = await loadWasm('imgui-sys-v0.wasm', imports);\n");
    for n in &names {
        js.push_str(&format!(
            "export function {0}(...args) {{ return (instance.exports['{0}'] ?? instance.exports['_{0}'])(...args); }}\n",
            n
        ));
    }
    fs::write(&js_path, js)?;
    eprintln!(
        "Wrote provider module: {} ({} exports)",
        js_path.display(),
        names.len()
    );

    // 4) Ensure import map exists in index.html (map 'imgui-sys-v0' -> './imgui-sys-v0.mjs')
    let index = out_dir.join("index.html");
    if index.exists() {
        let mut html = fs::read_to_string(&index)?;
        // Desired importmap snippet (no escaping in HTML) pointing to .js for better MIME defaults
        let importmap = r#"<script type="importmap">{
  "imports": {
    "imgui-sys-v0": "./imgui-sys-v0.js"
  }
}</script>"#;

        // If an older, incorrectly-escaped importmap exists, normalize it by
        // replacing the escaped variant with the correct snippet.
        let escaped_marker = "type=\\\"importmap\\\"";
        // Best-effort fix: drop any lingering escaped sequences from a prior bad patch
        if html.contains(escaped_marker) || html.contains("\\\"") || html.contains("\\n") {
            if html.contains(escaped_marker) {
                html = html.replace(escaped_marker, "type=\"importmap\"");
            }
            html = html.replace("\\\"", "\"");
            html = html.replace("\\n", "\n");
            fs::write(&index, &html)?;
            eprintln!("Normalized previously escaped importmap in index.html");
        }

        // Replace legacy .mjs mapping to .js if present
        if html.contains("imgui-sys-v0.mjs") {
            html = html.replace("imgui-sys-v0.mjs", "imgui-sys-v0.js");
            fs::write(&index, &html)?;
            eprintln!("Updated importmap to use .js instead of .mjs for imgui-sys-v0");
        }

        // Inject importmap if it's still missing
        if !html.contains("imgui-sys-v0.js") {
            html = html.replace(
                "<canvas id=\"wasm-canvas\"></canvas>",
                &format!("<canvas id=\"wasm-canvas\"></canvas>\n{}", importmap),
            );
            fs::write(&index, html)?;
            eprintln!("Patched index.html with importmap for imgui-sys-v0");
        }
    } else {
        eprintln!(
            "Warning: {} not found; provider module generated, but importmap not injected.",
            index.display()
        );
    }

    eprintln!("cimgui provider ready -> open target/web-demo/index.html");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("xtask error: {e:?}");
        std::process::exit(1);
    }
}
