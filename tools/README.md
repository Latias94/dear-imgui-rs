# Development Tools

This directory contains automation scripts for managing the dear-imgui-rs workspace.

## Overview

The workspace uses a **unified release train** model where all crates share the same version number. These tools help automate common tasks like version bumping, publishing, and validation.

## Quick Start

### Prepare a New Release

```bash
# All-in-one command to prepare a release
python3 tools/tasks.py release-prep 0.7.1
```

This will:
1. Bump version to 0.7.1 across all crates
2. Update pregenerated bindings for -sys crates
3. Run tests
4. Run pre-publish validation checks

### Publish to crates.io

```bash
# Dry run first (recommended)
python3 tools/tasks.py publish --dry-run

# Actual publish
python3 tools/tasks.py publish
```

## Available Scripts

### 1. `tasks.py` - Task Runner (Recommended)

Convenient shortcuts for common tasks.

```bash
# Run pre-publish checks
python3 tools/tasks.py check

# Bump version
python3 tools/tasks.py bump 0.7.1

# Update pregenerated bindings
python3 tools/tasks.py bindings

# Publish crates
python3 tools/tasks.py publish

# Run tests
python3 tools/tasks.py test

# Build documentation
python3 tools/tasks.py doc

# Clean build artifacts
python3 tools/tasks.py clean

# All-in-one release preparation
python3 tools/tasks.py release-prep 0.7.1
```

### 2. `publish.py` - Publishing Script

Publishes all crates in the correct dependency order.

```bash
# Dry run (show what would be published)
python3 tools/publish.py --dry-run

# Publish all crates
python3 tools/publish.py

# Publish specific crates
python3 tools/publish.py --crates dear-imgui-sys,dear-imgui-rs

# Resume from a specific crate
python3 tools/publish.py --start-from dear-implot-sys

# Adjust wait time between publishes
python3 tools/publish.py --wait 60
```

**Publishing Order:**
1. Core: `dear-imgui-sys` → `dear-imgui-rs`
2. Backends: `dear-imgui-winit`, `dear-imgui-wgpu`, `dear-imgui-glow`, `dear-imgui-sdl3`
3. Extension sys: `dear-implot-sys`, `dear-imnodes-sys`, etc.
4. Extension high-level: `dear-implot`, `dear-imnodes`, etc.
5. Application: `dear-app`

### 3. `bump_version.py` - Version Bumping

Updates version numbers across all crates and README files.

```bash
# Bump to a specific version (updates Cargo.toml and README files)
python3 tools/bump_version.py 0.7.1

# Dry run (show what would change)
python3 tools/bump_version.py 0.7.1 --dry-run

# Specify old version manually
python3 tools/bump_version.py 0.7.1 --old-version 0.7.0

# Bump only specific crates
python3 tools/bump_version.py 0.7.1 --crates dear-imgui-sys,dear-imgui-rs

# Skip README updates
python3 tools/bump_version.py 0.7.1 --skip-readme
```

**Note**: This script now automatically updates README files in addition to Cargo.toml files.

### 4. `pre_publish_check.py` - Validation

Runs pre-publish validation checks.

```bash
# Run all checks
python3 tools/pre_publish_check.py

# Skip specific checks
python3 tools/pre_publish_check.py --skip-git-check --skip-doc-check
```

**Checks performed:**
- Version consistency across all crates
- Pregenerated bindings exist for -sys crates
- Git working tree is clean
- Cargo.lock is up-to-date
- Documentation builds in offline mode
- Tests pass

### 5. `update_submodule_and_bindings.py` - Bindings Generation

Updates third-party submodules and regenerates pregenerated bindings for `-sys` crates (including optional WASM pregenerated bindings).

```bash
# Update all submodules and regenerate native bindings (all -sys crates)
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules update \
  --profile release

# Regenerate bindings only (no submodule updates)
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules skip \
  --profile release

# Update specific crate (e.g. dear-imgui-sys only)
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules update \
  --profile release

# Additionally generate WASM pregenerated bindings:
# - Core ImGui only (dear-imgui-sys, import module = imgui-sys-v0)
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules skip \
  --profile release \
  --wasm \
  --wasm-import imgui-sys-v0

# - Core ImGui + selected extensions (ImPlot, ImPlot3D, ImNodes, ImGuizmo, ImGuIZMO.quat)
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys,dear-implot-sys,dear-implot3d-sys,dear-imnodes-sys,dear-imguizmo-sys,dear-imguizmo-quat-sys \
  --submodules skip \
  --profile release \
  --wasm \
  --wasm-import imgui-sys-v0 \
  --wasm-ext implot,implot3d,imnodes,imguizmo,imguizmo-quat
```

### 6. `update_readme_versions.py` - README Version Updater

Updates version numbers in README files (compatibility tables and examples).

```bash
# Update to a specific version
python3 tools/update_readme_versions.py 0.7.1

# Dry run (show what would change)
python3 tools/update_readme_versions.py 0.7.1 --dry-run

# Specify old version manually
python3 tools/update_readme_versions.py 0.7.1 --old-version 0.7.0
```

**Note**: This script is automatically called by `bump_version.py`, so you usually don't need to run it manually.

## Typical Release Workflow

### Option 1: Using the All-in-One Command

```bash
# 1. Prepare release (bump version, update bindings, test, check)
python3 tools/tasks.py release-prep 0.7.1

# 2. Review changes
git diff

# 3. Update documentation
# - Edit CHANGELOG.md
# - Update README.md compatibility table
# - Update docs/COMPATIBILITY.md

# 4. Commit changes
git add -A
git commit -m "chore: prepare release v0.7.1"

# 5. Publish (dry run first)
python3 tools/tasks.py publish --dry-run
python3 tools/tasks.py publish

# 6. Tag and push
git tag -a v0.7.1 -m "Release v0.7.1"
git push origin main
git push origin v0.7.1

# 7. Create GitHub release
# Go to GitHub and create a release from the tag
```

### Option 2: Step-by-Step

```bash
# 1. Update submodules and bindings
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules update \
  --profile release

# 2. Bump version
python3 tools/bump_version.py 0.7.1

# 3. Update Cargo.lock
cargo update

# 4. Run tests
cargo test --workspace

# 5. Run pre-publish checks
python3 tools/pre_publish_check.py

# 6. Update documentation
# - CHANGELOG.md
# - README.md
# - docs/COMPATIBILITY.md

# 7. Commit changes
git add -A
git commit -m "chore: prepare release v0.7.1"

# 8. Publish
python3 tools/publish.py --dry-run  # Dry run first
python3 tools/publish.py            # Actual publish

# 9. Tag and push
git tag -a v0.7.1 -m "Release v0.7.1"
git push origin main
git push origin v0.7.1

# 10. Create GitHub release
```

## Common Tasks

### Update Bindings After Upstream Changes

```bash
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules update \
  --profile release \
  --cimgui-branch docking_inter \
  --cimplot-branch master \
  --cimnodes-branch master \
  --cimguizmo-branch master
```

### Verify docs.rs Offline Builds

```bash
# Windows PowerShell
$env:DOCS_RS = '1'
cargo check -p dear-imgui-sys
cargo check -p dear-implot-sys
cargo check -p dear-imnodes-sys
cargo check -p dear-imguizmo-sys
cargo check -p dear-implot3d-sys
cargo check -p dear-imguizmo-quat-sys

# Linux/macOS
DOCS_RS=1 cargo check -p dear-imgui-sys
DOCS_RS=1 cargo check -p dear-implot-sys
DOCS_RS=1 cargo check -p dear-imnodes-sys
DOCS_RS=1 cargo check -p dear-imguizmo-sys
DOCS_RS=1 cargo check -p dear-implot3d-sys
DOCS_RS=1 cargo check -p dear-imguizmo-quat-sys
```

### Resume Publishing After Failure

If publishing fails partway through:

```bash
# Resume from the failed crate
python3 tools/publish.py --start-from dear-implot-sys
```

### Publish Only Specific Crates

```bash
# Publish only backends
python3 tools/publish.py --crates dear-imgui-winit,dear-imgui-wgpu,dear-imgui-glow,dear-imgui-sdl3
```

## Requirements

All scripts require:
- **Python 3.7+**
- **cargo** in PATH
- **git** in PATH (for submodule management)
- **Logged in to crates.io**: `cargo login <token>`

## Troubleshooting

### "Command not found: python"

Try using `python3` instead:
```bash
python3 tools/tasks.py check
```

### "Permission denied"

Make scripts executable:
```bash
chmod +x tools/*.py
```

### Publishing Fails with "already published"

The script will detect this and ask if you want to skip. If you need to republish:
```bash
cargo yank --vers 0.4.0 dear-imgui-sys
python3 tools/publish.py --start-from dear-imgui-sys
```

### docs.rs Build Failures

Ensure pregenerated bindings are up-to-date:
```bash
python3 tools/update_submodule_and_bindings.py --crates all --profile release
```

Then verify locally:
```bash
DOCS_RS=1 cargo check -p dear-imgui-sys
```

## Related Documentation

- [docs/PUBLISHING.md](../docs/PUBLISHING.md) - Detailed publishing guide
- [docs/RELEASING.md](../docs/RELEASING.md) - Technical details about sys crate bindings
- [docs/COMPATIBILITY.md](../docs/COMPATIBILITY.md) - Version compatibility matrix

## Contributing

When adding new crates to the workspace:

1. Add the crate to `PUBLISH_ORDER` in `publish.py`
2. Add the crate to `ALL_CRATES` in `pre_publish_check.py`
3. Add the crate to `WORKSPACE_CRATES` in `bump_version.py`
4. If it's a `-sys` crate, add it to `SYS_CRATES` in `pre_publish_check.py`
5. Update this README with any new requirements

## License

These tools are part of the dear-imgui-rs project and are licensed under MIT OR Apache-2.0.
