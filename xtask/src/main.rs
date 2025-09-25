use anyhow::{Context, Result};
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
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

    // Ensure pregenerated wasm bindings exist for import-style linking (imgui-sys-v0)
    // If missing, generate them now so dear-imgui-sys does not attempt to bindgen for wasm.
    {
        let preg = root
            .join("dear-imgui-sys")
            .join("src")
            .join("wasm_bindings_pregenerated.rs");
        if !preg.exists() {
            eprintln!("Generating pregenerated wasm bindings (import module: imgui-sys-v0)...");
            gen_wasm_bindings(Some("imgui-sys-v0"))?;
        }
    }

    // 1) Build the web demo crate for wasm32-unknown-unknown
    eprintln!("Building dear-imgui-web-demo ({profile})...");
    let mut build_cmd = Command::new("cargo");
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
    // Clean old outputs to avoid stale/mismatched JS/WASM pairs
    if dist.exists() {
        eprintln!("Cleaning old web-demo dir: {}", dist.display());
        let _ = fs::remove_dir_all(&dist);
    }
    fs::create_dir_all(&dist)?;

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

    // 2b) Rewrite the generated wasm to import memory from `env` so we can share memory
    // with the Emscripten-built provider (imgui-sys-v0). wasm-bindgen 0.2.104 no longer
    // exposes `--import-memory`, so we do a small WAT roundtrip:
    //   - Insert `(import "env" "memory" (memory ...))` right after `(module` (imports must be first)
    //   - Insert `(export "memory" (memory 0))`
    //   - Remove the original `(memory ...)` and its existing export if present
    // wasm-bindgen may emit either hyphen or underscore versioned filenames in JS.
    // Patch both variants if present, prioritizing the one referenced by the JS (hyphenated).
    let main_bg_wasm_hyphen = dist.join(format!("{}{}_bg.wasm", pkg_name, ""));
    let main_bg_wasm_underscore = dist.join(format!("{}_bg.wasm", pkg_name.replace('-', "_")));

    let mut candidates = vec![];
    if main_bg_wasm_hyphen.exists() {
        candidates.push(main_bg_wasm_hyphen.clone());
    }
    if main_bg_wasm_underscore.exists() {
        candidates.push(main_bg_wasm_underscore.clone());
    }

    if candidates.is_empty() {
        anyhow::bail!("wasm-bindgen bg.wasm not found (checked hyphen and underscore variants)");
    }

    for main_bg_wasm in candidates {
        let wat_path = dist.join("__wasm_tmp.wat");
        let patched_wat_path = dist.join("__wasm_tmp_patched.wat");
        // Print to WAT
        let ok = Command::new("wasm-tools")
            .args([
                "print",
                main_bg_wasm.to_str().unwrap(),
                "-o",
                wat_path.to_str().unwrap(),
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            let mut wat = std::fs::read_to_string(&wat_path).unwrap_or_default();
            if !wat.contains("(import \"env\" \"memory\" (memory") {
                // Find original memory line to derive initial/max
                let mut original_mem_line_idx: Option<usize> = None;
                // Use safe defaults to avoid starving the provider (too-small max leads to OOM)
                let init_pages: String = "256".to_string();
                let max_pages: String = "4096".to_string();
                for (idx, line) in wat.lines().enumerate() {
                    let t = line.trim_start();
                    if t.starts_with("(memory") {
                        original_mem_line_idx = Some(idx);
                        break;
                    }
                }

                if let Some(mem_idx) = original_mem_line_idx {
                    let mut lines: Vec<String> = wat.lines().map(|s| s.to_string()).collect();
                    // Remove original memory line
                    lines.remove(mem_idx);
                    // Also remove any existing memory export to avoid duplicate
                    lines.retain(|l| l.trim() != "(export \"memory\" (memory 0))");
                    // Insert import + export right after the `(module` line (imports must come first)
                    if let Some(module_idx) = lines
                        .iter()
                        .position(|l| l.trim_start().starts_with("(module"))
                    {
                        let insert_at = module_idx + 1;
                        lines.insert(
                            insert_at,
                            format!(
                                "  (import \"env\" \"memory\" (memory (;0;) {} {}))",
                                init_pages, max_pages
                            ),
                        );
                        lines.insert(
                            insert_at + 1,
                            "  (export \"memory\" (memory 0))".to_string(),
                        );
                        wat = lines.join("\n");
                        std::fs::write(&patched_wat_path, &wat)?;
                        let ok2 = Command::new("wasm-tools")
                            .args([
                                "parse",
                                patched_wat_path.to_str().unwrap(),
                                "-o",
                                main_bg_wasm.to_str().unwrap(),
                            ])
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false);
                        if ok2 {
                            eprintln!(
                                "Patched {} to import memory from env",
                                main_bg_wasm.display()
                            );
                        } else {
                            eprintln!(
                                "Warning: failed to assemble patched WAT; leaving wasm unmodified"
                            );
                        }
                    } else {
                        eprintln!("Warning: failed to locate (module ...) header; skipping memory import patch");
                    }
                } else {
                    eprintln!("Warning: failed to find a (memory ...) declaration to patch");
                }
            }
            let _ = std::fs::remove_file(&wat_path);
            let _ = std::fs::remove_file(&patched_wat_path);
        } else {
            anyhow::bail!(
                "wasm-tools not found or failed to print WAT; cannot patch memory import.\nInstall with: cargo install wasm-tools"
            );
        }
    }

    // 3) Copy demo index.html
    let index_src = root.join("examples-wasm/web/index.html");
    let index_dst = dist.join("index.html");
    fs::copy(&index_src, &index_dst)?;

    // 3b) Patch generated JS to wire shared memory into wasm-bindgen init
    let js_main = dist.join(format!("{}.js", pkg_name));
    if js_main.exists() {
        let mut code = fs::read_to_string(&js_main)?;
        // Ensure we hand a shared memory to the module as `env.memory` so the provider (emscripten)
        // and main module use the same memory.
        if !code.contains("__imgui_shared_memory") {
            // Try to find the wasm-bindgen imports function header across versions
            let header_pos = code
                .find("function __wbg_get_imports()")
                .or_else(|| code.find("function getImports()"));

            // Choose insertion point: right after "const imports = {};" inside the function body
            let mut insert_at: Option<usize> = None;
            if let Some(h) = header_pos {
                // Search for the first occurrence of the marker after the header
                if let Some(rel) = code[h..].find("const imports = {};") {
                    insert_at = Some(h + rel + "const imports = {};".len());
                } else if let Some(open_idx) = code[h..].find("{\n") {
                    // Fallback: insert right after opening brace
                    insert_at = Some(h + open_idx + 2);
                }
            }
            // Last resort: search globally for the marker
            if insert_at.is_none() {
                if let Some(global_rel) = code.find("const imports = {};") {
                    insert_at = Some(global_rel + "const imports = {};".len());
                }
            }

            if let Some(pos) = insert_at {
                let inject = r#"
        // Inject shared memory for import-style provider (imgui-sys-v0)
        const __shared_mem = globalThis.__imgui_shared_memory || new WebAssembly.Memory({ initial: 256, maximum: 4096 });
        if (!imports.env) imports.env = {};
        if (!imports.env.memory) imports.env.memory = __shared_mem;
"#;
                code.insert_str(pos, inject);
                eprintln!(
                    "Patched wasm-bindgen imports in {} to provide env.memory",
                    js_main.display()
                );
            } else {
                anyhow::bail!(
                    "Failed to locate insertion point in {} for memory injection (searched for __wbg_get_imports/getImports and 'const imports = {{}};')",
                    js_main.display()
                );
            }
        }
        fs::write(&js_main, &code)?;
    }

    eprintln!("Web demo built at: {}", dist.display());
    eprintln!(
        "Serve this dir via any static server, e.g.\n  python -m http.server -d {} 8080",
        dist.display()
    );
    // Import-style build: remember to run `xtask build-cimgui-provider` to generate the provider.
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

    // 2) Prepare export list from pregenerated bindings, then compile provider once with that list
    let sys_root = root.join("dear-imgui-sys");
    let cimgui_root = sys_root.join("third-party").join("cimgui");
    let imgui_src = cimgui_root.join("imgui");
    let out_js = out_dir.join("imgui-sys-v0.js"); // Output to .js, not .wasm

    // Generate ES module glue export names by scanning pregenerated wasm bindings
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

    // 2b) Compose em++ command to build imgui-sys-v0.wasm (shared imported memory) with explicit exports

    let mut cmd = Command::new(&empp);
    cmd.arg("-std=c++17")
        .arg("-O2")
        .arg("-s")
        .arg("MODULARIZE=1")  // Generate a module function
        .arg("-s")
        .arg("EXPORT_ES6=1")  // Export as ES6 module
        .arg("-s")
        .arg("ENVIRONMENT=web")
        .arg("-s")
        .arg("GLOBAL_BASE=67108864") // Place provider static data high to avoid overlap with main module
        .arg("-s")
        .arg("IMPORTED_MEMORY=1")
        .arg("-s")
        .arg("ALLOW_MEMORY_GROWTH=1")
        .arg("-s")
        .arg("INITIAL_MEMORY=134217728")
        .arg("-s")
        .arg("FILESYSTEM=0")
        .arg("-s")
        .arg("NO_EXIT_RUNTIME=1")
        .arg("-s")
        .arg("MALLOC=emmalloc")
        .arg("-s")
        .arg("ASSERTIONS=1")
        .arg("-s")
        .arg("STACK_SIZE=1048576")
        .arg("-s")
        .arg("EXPORTED_RUNTIME_METHODS=[\"ccall\",\"cwrap\",\"allocate\",\"stackSave\",\"stackRestore\",\"stackAlloc\",\"UTF8ToString\",\"stringToUTF8\",\"lengthBytesUTF8\"]")
        .arg("-s")
        .arg(format!(
            "EXPORTED_FUNCTIONS=@{}",
            exports_path
                .to_string_lossy()
                .replace('\\', "/") // emscripten on Windows accepts fwd slashes
        ))
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
        .arg(&out_js); // Output to .js file for MODULARIZE mode

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

    eprintln!("Building cimgui provider -> {}", out_js.display());
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("em++ failed; see output for details");
    }

    // 3) MODULARIZE=1 generates both .js and .wasm files
    // The .js file is already created, now create the wrapper
    let emscripten_js = out_dir.join("imgui-sys-v0.js");
    if !emscripten_js.exists() {
        anyhow::bail!("Emscripten output not found: {}", emscripten_js.display());
    }

    // Create wrapper module
    let js_path = out_dir.join("imgui-sys-v0-wrapper.js");
    let mut js = String::new();
    js.push_str("// Auto-generated wrapper for imgui-sys-v0 provider\n");
    js.push_str("import createModule from './imgui-sys-v0.js';\n");
    js.push_str("\n");
    js.push_str("// Use shared memory if available\n");
    js.push_str("const memory = globalThis.__imgui_shared_memory || new WebAssembly.Memory({initial:256, maximum:4096});\n");
    js.push_str("\n");
    js.push_str("// Initialize the module with shared memory\n");
    js.push_str("const Module = await createModule({\n");
    js.push_str("  wasmMemory: memory,\n");
    js.push_str("  printErr: (text) => console.warn('[imgui-sys-v0]', text),\n");
    js.push_str("  print: (text) => console.log('[imgui-sys-v0]', text),\n");
    js.push_str("});\n");
    js.push_str(
        "console.log('[imgui-sys-v0] Shared memory pages=', (memory.buffer.byteLength>>>16));\n",
    );
    js.push_str(
        "console.log('[imgui-sys-v0] Module.wasmMemory===memory', Module.wasmMemory===memory);\n",
    );
    js.push_str("\n");
    js.push_str("// Export all the functions\n");
    for n in &names {
        js.push_str(&format!(
            "export const {} = Module._{} || Module.{};\n",
            n, n, n
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
    "imgui-sys-v0": "./imgui-sys-v0-wrapper.js"
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

        // Replace legacy .mjs or .js mapping to wrapper if present
        if html.contains("imgui-sys-v0.mjs") || html.contains("imgui-sys-v0.js") {
            html = html.replace("imgui-sys-v0.mjs", "imgui-sys-v0-wrapper.js");
            html = html.replace("imgui-sys-v0.js", "imgui-sys-v0-wrapper.js");
            fs::write(&index, &html)?;
            eprintln!("Updated importmap to use wrapper for imgui-sys-v0");
        }

        // Inject importmap if it's still missing
        // Import map must be placed BEFORE any module scripts
        if !html.contains("imgui-sys-v0-wrapper.js") && !html.contains("type=\"importmap\"") {
            // Insert import map right after the closing </style> tag, before the body
            html = html.replace(
                "</style>\n  </head>",
                &format!("</style>\n{}\n  </head>", importmap),
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
