# Publishing Guide

This guide explains how to publish new versions of the dear-imgui-rs workspace crates to crates.io.

## Overview

The workspace uses a **unified release train** model where all crates share the same version number (e.g., 0.4.0). This simplifies dependency management and ensures compatibility across the ecosystem.

## Prerequisites

### 1. Environment Setup

- **Rust toolchain**: Latest stable Rust installed
- **Git**: For version control and submodule management
- **Python 3.7+**: For running automation scripts
- **Cargo login**: Must be logged in to crates.io
  ```bash
  cargo login <your-api-token>
  ```

### 2. Pre-publish Checklist

Before publishing, ensure:

- [ ] All tests pass on all platforms (Linux, Windows, macOS)
  ```bash
  cargo test --workspace
  ```

- [ ] All examples build successfully
  ```bash
  cargo build --examples
  ```

- [ ] Version numbers are updated in all `Cargo.toml` files
  - Update workspace version in root `Cargo.toml`
  - Update all crate versions to match
  - Update all internal dependencies to use the new version

- [ ] `CHANGELOG.md` is updated with release notes

- [ ] Documentation is up-to-date
  - [ ] Root `README.md` compatibility table
  - [ ] `docs/COMPATIBILITY.md` with new release train info
  - [ ] Individual crate READMEs if needed

- [ ] Pregenerated bindings are up-to-date for `-sys` crates
  ```bash
  python3 tools/update_submodule_and_bindings.py --crates all --profile release
  ```

- [ ] Verify `-sys` crates build in docs.rs offline mode
  ```bash
  # Windows PowerShell
  $env:DOCS_RS = '1'; cargo check -p dear-imgui-sys
  $env:DOCS_RS = '1'; cargo check -p dear-implot-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imnodes-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imguizmo-sys
  $env:DOCS_RS = '1'; cargo check -p dear-implot3d-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imguizmo-quat-sys
  
  # Linux/macOS
  DOCS_RS=1 cargo check -p dear-imgui-sys
  DOCS_RS=1 cargo check -p dear-implot-sys
  DOCS_RS=1 cargo check -p dear-imnodes-sys
  DOCS_RS=1 cargo check -p dear-imguizmo-sys
  DOCS_RS=1 cargo check -p dear-implot3d-sys
  DOCS_RS=1 cargo check -p dear-imguizmo-quat-sys
  ```

- [ ] CI is green on all platforms

- [ ] Git working tree is clean (commit all changes)

## Publishing Process

### Automated Publishing (Recommended)

Use the `tools/publish.py` script to publish all crates in the correct dependency order:

#### 1. Dry Run (Preview)

First, do a dry run to see what would be published:

```bash
python3 tools/publish.py --dry-run
```

This will show you:
- The order in which crates will be published
- Version numbers for each crate
- Any potential issues

#### 2. Publish All Crates

Once you've verified the dry run output:

```bash
python3 tools/publish.py
```

The script will:
1. Show a summary of what will be published
2. Ask for confirmation
3. Publish each crate in dependency order
4. Wait between publishes for crates.io to index
5. Check if crates are already published and skip if needed

#### 3. Advanced Options

**Publish specific crates:**
```bash
python3 tools/publish.py --crates dear-imgui-sys,dear-imgui-rs
```

**Skip verification (faster, but not recommended):**
```bash
python3 tools/publish.py --no-verify
```

**Adjust wait time between publishes:**
```bash
python3 tools/publish.py --wait 60  # Wait 60 seconds instead of default 30
```

**Resume from a specific crate:**
```bash
python3 tools/publish.py --start-from dear-implot-sys
```

This is useful if publishing was interrupted and you want to continue from where it stopped.

### Manual Publishing

If you prefer to publish manually or need to publish individual crates:

#### Publishing Order

**IMPORTANT**: Crates must be published in this exact order to satisfy dependencies:

1. **Core**
   ```bash
   cargo publish -p dear-imgui-sys
   # Wait for crates.io to index (~30 seconds)
   cargo publish -p dear-imgui-rs
   ```

2. **Backends**
   ```bash
   cargo publish -p dear-imgui-winit
   cargo publish -p dear-imgui-wgpu
   cargo publish -p dear-imgui-glow
   ```

3. **Extension Sys Crates**
   ```bash
   cargo publish -p dear-implot-sys
   cargo publish -p dear-imnodes-sys
   cargo publish -p dear-imguizmo-sys
   cargo publish -p dear-implot3d-sys
   cargo publish -p dear-imguizmo-quat-sys
   ```

4. **Extension High-Level Crates**
   ```bash
   cargo publish -p dear-implot
   cargo publish -p dear-imnodes
   cargo publish -p dear-imguizmo
   cargo publish -p dear-implot3d
   cargo publish -p dear-imguizmo-quat
   cargo publish -p dear-file-browser
   cargo publish -p dear-imgui-reflect-derive
   cargo publish -p dear-imgui-reflect
   ```

5. **Application Runner**
   ```bash
   cargo publish -p dear-app
   ```

**Note**: Wait 30-60 seconds between publishes to allow crates.io to index the crates.

## Post-Publishing

After successful publishing:

### 1. Create Git Tag

Tag the release in git:

```bash
git tag -a v0.4.0 -m "Release v0.4.0"
git push origin v0.4.0
```

### 2. Create GitHub Release

1. Go to GitHub repository releases page
2. Click "Draft a new release"
3. Select the tag you just created
4. Copy changelog content for this version
5. Publish the release

### 3. Trigger Prebuilt Binaries Workflow (Optional)

If you want to provide prebuilt binaries for the `-sys` crates:

1. Go to Actions → "Prebuilt Binaries" workflow
2. Click "Run workflow"
3. Select the tag (e.g., `v0.4.0`)
4. Select crates to build (or `all`)
5. Run the workflow

The workflow will build prebuilt binaries for Windows (MD/MT variants) and upload them as release assets.

### 4. Verify Published Crates

Check that all crates are available on crates.io:

```bash
# Check a specific crate
cargo search dear-imgui-rs --limit 1

# Or visit crates.io directly
# https://crates.io/crates/dear-imgui-rs
```

### 5. Update Documentation

Ensure docs.rs has successfully built documentation for all crates:
- Visit https://docs.rs/dear-imgui-rs
- Check that the latest version is shown
- Verify documentation builds without errors

## Troubleshooting

### Crate Already Published

If a crate version is already published, you have two options:

1. **Skip it**: The script will detect this and ask if you want to skip
2. **Yank and republish**: 
   ```bash
   cargo yank --vers 0.4.0 dear-imgui-sys
   cargo publish -p dear-imgui-sys
   ```
   **Warning**: Only do this immediately after publishing if you found a critical issue.

### Publishing Failed

If publishing fails for a crate:

1. **Check the error message**: Often it's a missing dependency or version mismatch
2. **Fix the issue**: Update Cargo.toml or fix the code
3. **Resume publishing**: Use `--start-from` to continue from the failed crate
   ```bash
   python3 tools/publish.py --start-from dear-implot-sys
   ```

### Dependency Version Mismatch

If you get errors about dependency versions:

1. Ensure all internal dependencies use the correct version
2. Check that the dependency was successfully published to crates.io
3. Wait a bit longer for crates.io to index the dependency

### docs.rs Build Failures

If docs.rs fails to build a `-sys` crate:

1. Verify pregenerated bindings exist:
   ```bash
   ls -la dear-imgui-sys/src/bindings_pregenerated.rs
   ```

2. Test offline build locally:
   ```bash
   DOCS_RS=1 cargo doc -p dear-imgui-sys --no-deps
   ```

3. If bindings are missing or outdated, regenerate them:
   ```bash
   python3 tools/update_submodule_and_bindings.py --crates dear-imgui-sys --profile release
   ```

## Version Bump Checklist

When preparing a new version:

- [ ] Update version in `Cargo.toml` for all crates
- [ ] Update internal dependency versions
- [ ] Update `CHANGELOG.md`
- [ ] Update `README.md` compatibility table
- [ ] Update `docs/COMPATIBILITY.md`
- [ ] Run `cargo update` to update `Cargo.lock`
- [ ] Commit all changes
- [ ] Run full test suite
- [ ] Generate pregenerated bindings
- [ ] Verify docs.rs offline builds
- [ ] Publish using the script
- [ ] Create git tag
- [ ] Create GitHub release
- [ ] Trigger prebuilt binaries workflow

## Release Cadence

The project follows a **release train** model:

- **Major versions** (0.x → 1.0, 1.x → 2.0): Breaking changes, major upstream updates
- **Minor versions** (0.4 → 0.5): New features, non-breaking changes, upstream updates
- **Patch versions** (0.4.0 → 0.4.1): Bug fixes, documentation updates

All crates in the workspace are versioned together, even if some crates haven't changed.

## Related Documentation

- [RELEASING.md](./RELEASING.md) - Technical details about sys crate bindings
- [COMPATIBILITY.md](./COMPATIBILITY.md) - Version compatibility matrix
- [README.md](../README.md) - Main project documentation
