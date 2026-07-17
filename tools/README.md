# Development Tools

This directory contains automation scripts for managing the dear-imgui-rs workspace.

## Overview

The workspace uses a **unified release train** model. All 27 publishable packages inherit `workspace.package.version`; examples and workspace-only tools keep independent package versions. Release tooling prepares, validates, and publishes that single version source.

## Quick Start

### Prepare a New Release

```bash
# Generate the complete 0.16.0 release diff.
python3 tools/tasks.py release-prepare 0.16.0

# Review and commit versions, bindings, lockfile, changelog, and docs.
git diff
git add -A
git commit -m "chore: prepare release v0.16.0"

# Validate the committed clean release candidate.
python3 tools/tasks.py release-check
```

`release-prepare` intentionally leaves changes in the working tree. `release-check` runs the strict clean-tree, changelog, locked dependency graph, reproducible binding, package/offline, documentation, and test gates. Keeping these phases separate prevents release preparation from failing its own clean-tree check.

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

# Create a release diff, then validate it after commit
python3 tools/tasks.py release-prepare 0.16.0
python3 tools/tasks.py release-check
```

### 2. `xtask release-version` - Unified Version Update

The workspace root is the single version source. Publishable manifests use `version.workspace = true`, and internal dependencies inherit their root workspace declarations. `[workspace.metadata.dear-imgui-release]` is the shared policy for the core package and private package paths/versions; Rust and Python release validators derive package counts from the actual workspace members. Update the release train with:

```bash
cargo run -p xtask -- release-version 0.16.0
```

The command updates the root release version and inherited internal dependency requirements as one validated workspace operation. It never offers partial crate selection. Documentation remains an explicit review step.

### 3. `publish.py` - Publishing Script

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

Print-only `--dry-run` validates metadata and shows commands without running the
expensive release gate. `--cargo-dry-run` and real uploads rerun the strict
clean-tree preflight, explicitly target the `crates-io` registry, and verify the
validated Git `HEAD` again before every Cargo publish command.

**Publishing Order:**
1. Build tooling: `dear-imgui-build-support`
2. Core: `dear-imgui-sys` → `dear-imgui-rs`
3. Platform/renderer backends except Bevy
4. All extension sys crates, including `dear-imgui-test-engine-sys`
5. All extension high-level crates, file browser, and reflection derive/runtime
6. `dear-imgui-bevy` after its optional ecosystem dependencies
7. `dear-app`

### 4. `pre_publish_check.py` - Validation

Runs pre-publish validation checks.

```bash
# Run all checks
python3 tools/pre_publish_check.py

# Skip specific checks
python3 tools/pre_publish_check.py \
  --skip-git-check --skip-doc-check --skip-package-check
```

**Checks performed:**
- Version consistency across all crates
- Exact source metadata and reproducible Windows/non-Windows/WASM core bindings
- Pregenerated bindings exist for extension sys crates
- Git working tree is clean
- `Cargo.lock` resolves with `--locked`
- Packaged core crates build from a clean clone and offline consumer
- Packaged sys crates contain required artifacts and build offline without `.git`
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
  --submodules auto \
  --profile release

# Regenerate core binding profiles and compile-check the fixed WASM provider contract
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules skip \
  --profile release \
  --wasm

# Regenerate core bindings plus selected extension WASM bindings
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys,dear-implot-sys,dear-implot3d-sys,dear-imnodes-sys,dear-imguizmo-sys,dear-imguizmo-quat-sys \
  --submodules skip \
  --profile release \
  --wasm \
  --wasm-ext implot,implot3d,imnodes,imguizmo,imguizmo-quat
```

### 6. CI Package and Submodule Helpers

Repository-maintained CI helpers are Python entry points and run on Windows,
macOS, and Linux without Bash.

Initialize the exact nested submodules needed by CI:

```bash
python3 tools/ci/update_submodules.py
```

Top-level repository submodules must already be initialized, as they are when
`actions/checkout` uses `submodules: true`. For a local clone, initialize the
top-level submodules before running this helper.

Run the complete clean-clone source-package, offline-consumer, and host-prebuilt
gate:

```bash
python3 tools/ci/verify_packaged_core.py full
```

Consume all prebuilt profiles for one target and optional CRT:

```bash
python3 tools/ci/verify_packaged_core.py prebuilt PACKAGE_DIR TARGET [CRT]
```

The no-argument form remains equivalent to `full`. The legacy
`--verify-prebuilt-packages PACKAGE_DIR TARGET [CRT]` spelling is accepted for
existing automation, but new callers should use the `prebuilt` command.

## Typical Release Workflow

### Option 1: Recommended Two-Phase Workflow

```bash
# 1. Generate versions, bindings, provenance, and lockfile changes.
python3 tools/tasks.py release-prepare 0.16.0

# 2. Review generated and hand-written release changes.
git diff
# - Edit CHANGELOG.md
#   - Keep changelog prose soft-wrapped; do not hard-wrap bullet text to a fixed column.
# - Update README.md compatibility table
# - Update docs/COMPATIBILITY.md

# 3. Commit the release candidate.
git add -A
git commit -m "chore: prepare release v0.16.0"

# 4. Run strict checks against the clean committed tree.
python3 tools/tasks.py release-check

# 5. Publish (dry run first).
python3 tools/tasks.py publish --dry-run
python3 tools/tasks.py publish

# 6. Tag and push.
git tag -a v0.16.0 -m "Release v0.16.0"
git push origin main
git push origin v0.16.0

# 7. GitHub release.
# Pushing the v* tag triggers .github/workflows/release.yml, which uses the matching CHANGELOG.md section as the release body.
```

### Option 2: Step-by-Step

```bash
# 1. Update the single workspace release version
cargo run -p xtask -- release-version 0.16.0

# 2. Regenerate bindings without moving third-party submodules
python3 tools/tasks.py bindings

# 3. Verify the version command left a locked dependency graph
cargo metadata --locked --format-version 1 --no-deps > /dev/null

# 4. Run the repository's feature-safe test matrix
python3 tools/tasks.py test

# 5. Verify core binding drift
cargo run -p xtask -- verify-bindings

# 6. Update documentation
# - CHANGELOG.md
#   - Keep prose soft-wrapped
# - README.md
# - docs/COMPATIBILITY.md

# 7. Commit changes
git add -A
git commit -m "chore: prepare release v0.16.0"

# 8. Run the strict clean-tree release gate
python3 tools/tasks.py release-check

# 9. Publish
python3 tools/publish.py --dry-run  # Dry run first
python3 tools/publish.py            # Actual publish

# 10. Tag and push
git tag -a v0.16.0 -m "Release v0.16.0"
git push origin main
git push origin v0.16.0

# 11. GitHub release is created/updated automatically from CHANGELOG.md by .github/workflows/release.yml
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
  --cimplot3d-branch main \
  --cimnodes-branch master \
  --cimnodes-editor-branch main \
  --cimguizmo-branch master \
  --imgui-test-engine-branch main
```

### Verify docs.rs Offline Builds

```bash
# Windows PowerShell
$env:DOCS_RS = '1'
cargo check -p dear-imgui-sys
cargo check -p dear-implot-sys
cargo check -p dear-imnodes-sys
cargo check -p dear-node-editor-sys
cargo check -p dear-imguizmo-sys
cargo check -p dear-implot3d-sys
cargo check -p dear-imguizmo-quat-sys
cargo check -p dear-imgui-test-engine-sys

# Linux/macOS
DOCS_RS=1 cargo check -p dear-imgui-sys
DOCS_RS=1 cargo check -p dear-implot-sys
DOCS_RS=1 cargo check -p dear-imnodes-sys
DOCS_RS=1 cargo check -p dear-node-editor-sys
DOCS_RS=1 cargo check -p dear-imguizmo-sys
DOCS_RS=1 cargo check -p dear-implot3d-sys
DOCS_RS=1 cargo check -p dear-imguizmo-quat-sys
DOCS_RS=1 cargo check -p dear-imgui-test-engine-sys
```

### Resume Publishing After Failure

If publishing fails partway through:

```bash
# Resume from the failed crate
python3 tools/publish.py --start-from dear-implot-sys
```

### Publish Only Specific Crates

```bash
# Publish only backend crates
python3 tools/publish.py --crates dear-imgui-winit,dear-imgui-wgpu,dear-imgui-glow,dear-imgui-ash,dear-imgui-sdl3,dear-imgui-bevy
```

## Requirements

All scripts require:
- **Python 3.11+**
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

Published versions cannot be overwritten. The script can skip an already
published crate; if the release is defective, prepare and publish a new patch:
```bash
cargo yank --registry crates-io --vers 0.16.0 dear-imgui-sys
python3 tools/tasks.py release-prepare 0.16.1
# Review and commit, then run the strict gate from the clean tree.
python3 tools/tasks.py release-check
python3 tools/publish.py
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

1. Set `version.workspace = true` for a publishable package, or keep an explicit independent version for a workspace-only package
2. Add publishable crates to the shared `PUBLISH_ORDER` in `release_metadata.py`; metadata discovery supplies the version and package inventory automatically
3. If it's a `-sys` crate, add it to `SYS_CRATES` in `pre_publish_check.py`
4. Update this README with any new requirements

## License

These tools are part of the dear-imgui-rs project and are licensed under MIT OR Apache-2.0.
