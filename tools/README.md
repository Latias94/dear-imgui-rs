# Development Tools

This directory contains automation scripts for managing the dear-imgui-rs workspace.

## Overview

The workspace uses a **unified release train** model. All 27 publishable packages inherit `workspace.package.version`; examples and workspace-only tools keep independent package versions. Release tooling prepares, validates, and publishes that single version source.

## Quick Start

### Prepare a New Release

```bash
# Generate the complete 0.16.0-alpha.1 release diff.
python3 tools/tasks.py release-prepare 0.16.0-alpha.1

# Review and commit versions, bindings, lockfile, changelog, and docs.
git diff
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.1"

# Validate the committed clean release candidate.
python3 tools/tasks.py release-check

# Record this exact 40-hex commit and dispatch release-gate.yml with it as
# candidate_sha. Do not substitute a branch or tag that can move.
git rev-parse HEAD
gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
```

`release-prepare` intentionally leaves changes in the working tree. `release-check` runs the strict clean-tree, changelog, locked dependency graph, reproducible binding, package/offline, documentation, and test gates. Keeping these phases separate prevents release preparation from failing its own clean-tree check.

Local success is necessary but not sufficient for release. The remote release
gate must return `Go` for the same candidate SHA across all 14 required cells.
Download that run's authoritative `gate-result.json`; crates.io upload and the
GitHub Release both verify its exact SHA and complete inventory.

### Publish to crates.io

```bash
# Dry run first (recommended)
python3 tools/tasks.py publish --dry-run

# Actual upload requires the same-SHA remote aggregate.
python3 tools/tasks.py publish \
  --release-gate-result artifacts/release-gate/gate-result.json
```

## Available Scripts

### 1. `tasks.py` - Task Runner (Recommended)

Convenient shortcuts for common tasks.

```bash
# Run pre-publish checks
python3 tools/tasks.py check

# Update pregenerated bindings
python3 tools/tasks.py bindings

# Preview publishing; actual upload also needs --release-gate-result as shown above
python3 tools/tasks.py publish --dry-run

# Run tests
python3 tools/tasks.py test

# Build documentation
python3 tools/tasks.py doc

# Clean build artifacts
python3 tools/tasks.py clean

# Create a release diff, then validate it after commit
python3 tools/tasks.py release-prepare 0.16.0-alpha.1
python3 tools/tasks.py release-check
```

### 2. `xtask release-version` - Unified Version Update

The workspace root is the single version source. Publishable manifests use `version.workspace = true`, and internal dependencies inherit their root workspace declarations. `[workspace.metadata.dear-imgui-release]` is the shared policy for the core package and private package paths/versions; Rust and Python release validators derive package counts from the actual workspace members. Update the release train with:

```bash
cargo run -p xtask -- release-version 0.16.0-alpha.1 --allow-prerelease-relabel
```

The command updates the root release version and inherited internal dependency requirements as one validated workspace operation. It never offers partial crate selection. Documentation remains an explicit review step.

### 3. `publish.py` - Publishing Script

Publishes all crates in the correct dependency order.

```bash
# Dry run (show what would be published)
python3 tools/publish.py --dry-run

# Publish all crates after authoritative evidence verification
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json

# Publish specific crates
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --crates dear-imgui-sys,dear-imgui-rs

# Resume from a specific crate
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --start-from dear-implot-sys

# Adjust wait time between publishes
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --wait 60
```

Print-only `--dry-run` validates metadata and shows commands without running the
expensive release gate. `--cargo-dry-run` reruns the local clean-tree preflight.
Real uploads additionally require `--release-gate-result`, verify its exact
`HEAD` and 14-cell `Go` decision before any network command, explicitly target
the `crates-io` registry, and verify the validated Git fingerprint again before
every Cargo publish command.

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
- Python workflow/release contracts and the public API policy pass
- Native source and explicit WASM-safe feature routes pass without using workspace `--all-features`
- Targeted Rust tests pass

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

`tools/ci/run_contract.py` centralizes expected-failure diagnostics, feature
conflict checks, Clippy allowance expansion, default dependency audits, and
release-note preparation. Workflow files contain no repository-owned Bash
control flow; platform package installation remains a runner command.

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

Consume all prebuilt profiles for one target, exact candidate SHA, and optional CRT:

```bash
python3 tools/ci/verify_packaged_core.py prebuilt PACKAGE_DIR TARGET CANDIDATE_SHA [CRT]
```

`CANDIDATE_SHA` is required because both core and extension manifests bind to
the exact release commit. The literal `HEAD` is accepted for a local checkout
and is resolved before validation; release workflows pass the full lowercase
40-hex SHA explicitly.

The no-argument form remains equivalent to `full`. The legacy
`--verify-prebuilt-packages PACKAGE_DIR TARGET CANDIDATE_SHA [CRT]` spelling is
accepted for existing automation, but new callers should use the `prebuilt`
command.

### 7. Release Evidence

`.github/workflows/release-gate.yml` is the authoritative cross-platform gate.
It checks out one explicit 40-hex candidate SHA and requires exactly these 14
cells:

- Linux Test Engine runtime plus real Winit/WGPU and SDL3/Glow multi-viewport
  smokes
- Linux `wasm32-unknown-unknown` feature and binding routes
- Windows vcpkg, MSVC `/MD`, MSVC `/MT`, and MinGW import checks
- macOS native build
- five prebuilt producer/consumer cells: Linux x86_64, macOS x86_64/aarch64,
  and Windows MSVC `/MD`/`/MT`

A failed, skipped, cancelled, timed-out, missing, duplicate, malformed, or
wrong-SHA cell makes the aggregate `No-Go`. The workflow retains the aggregate,
stdout/stderr, runtime/display/renderer data, target/CRT/vcpkg/MinGW metadata,
binding hashes, manifests, candidate SHA, and SHA256 evidence for approximately
30 days.

Verify a downloaded aggregate against the local committed `HEAD` with:

```bash
python3 tools/ci/release_evidence.py verify \
  --repo-root . \
  --candidate-sha CANDIDATE_SHA \
  --gate-result artifacts/release-gate/gate-result.json
```

The production verifier owns the required cell inventory; callers cannot pass
a smaller list.

## Typical Release Workflow

### Option 1: Recommended Two-Phase Workflow

```bash
# 1. Generate versions, bindings, provenance, and lockfile changes.
python3 tools/tasks.py release-prepare 0.16.0-alpha.1

# 2. Review generated and hand-written release changes.
git diff
# - Edit CHANGELOG.md
#   - Keep changelog prose soft-wrapped; do not hard-wrap bullet text to a fixed column.
# - Update README.md compatibility table
# - Update docs/COMPATIBILITY.md

# 3. Commit the release candidate.
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.1"

# 4. Run strict checks against the clean committed tree.
python3 tools/tasks.py release-check

# 5. Dispatch .github/workflows/release-gate.yml for the exact SHA printed here.
git rev-parse HEAD
gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
# Wait for its complete 14-cell Go result, then download the aggregate.
gh run download RELEASE_GATE_RUN_ID \
  --name release-gate-FULL_40_HEX_SHA \
  --dir artifacts/release-gate

# 6. Publish (dry run first).
python3 tools/tasks.py publish --dry-run
python3 tools/tasks.py publish \
  --release-gate-result artifacts/release-gate/gate-result.json

# 7. Tag and push the already-verified commit.
git tag -a v0.16.0-alpha.1 -m "Release v0.16.0-alpha.1"
git push origin main
git push origin v0.16.0-alpha.1

# 8. Create the GitHub Release only through the verified workflow.
gh workflow run release.yml \
  -f tag=v0.16.0-alpha.1 \
  -f candidate_sha=FULL_40_HEX_SHA \
  -f gate_run_id=RELEASE_GATE_RUN_ID
```

### Option 2: Step-by-Step

```bash
# 1. Update the single workspace release version
cargo run -p xtask -- release-version 0.16.0-alpha.1 --allow-prerelease-relabel

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
git commit -m "chore: prepare release v0.16.0-alpha.1"

# 8. Run the strict clean-tree release gate
python3 tools/tasks.py release-check

# 9. Run the remote release gate for this exact committed HEAD and download its
# complete Go gate-result.json.
git rev-parse HEAD
gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
gh run download RELEASE_GATE_RUN_ID \
  --name release-gate-FULL_40_HEX_SHA \
  --dir artifacts/release-gate

# 10. Publish
python3 tools/publish.py --dry-run  # Dry run first
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json

# 11. Tag and push
git tag -a v0.16.0-alpha.1 -m "Release v0.16.0-alpha.1"
git push origin main
git push origin v0.16.0-alpha.1

# 12. Dispatch release.yml with all three required inputs.
gh workflow run release.yml \
  -f tag=v0.16.0-alpha.1 \
  -f candidate_sha=FULL_40_HEX_SHA \
  -f gate_run_id=RELEASE_GATE_RUN_ID
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
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --start-from dear-implot-sys
```

### Publish Only Specific Crates

```bash
# Publish only backend crates
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --crates dear-imgui-winit,dear-imgui-wgpu,dear-imgui-glow,dear-imgui-ash,dear-imgui-sdl3,dear-imgui-bevy
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
published crate; if the release is defective, prepare and publish a new version:
```bash
cargo yank --registry crates-io --vers <published-version> dear-imgui-sys
python3 tools/tasks.py release-prepare <next-version>
# Review and commit, then run the strict gate from the clean tree.
python3 tools/tasks.py release-check
# Run the new patch candidate through release-gate.yml, download its Go result, then:
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json
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
