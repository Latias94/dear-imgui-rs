# Development Tools

This directory contains automation scripts for managing the dear-imgui-rs workspace.

## Overview

The workspace uses a **unified release train** model. All 27 publishable packages inherit `workspace.package.version`; examples and workspace-only tools keep independent package versions. Release tooling prepares, validates, and publishes that single version source.

## Quick Start

### Prepare a New Release

```bash
# Generate the complete release diff.
python3 tools/tasks.py release-prepare 0.16.0-alpha.2

# Review and commit versions, bindings, lockfile, changelog, and docs.
git diff
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.2"

# Validate the committed clean release candidate.
python3 tools/tasks.py release-check

# After merging to main and normal CI passes, run the complete release.
gh workflow run release.yml --ref main -f tag=v0.16.0-alpha.2
```

`release-prepare` intentionally leaves changes in the working tree. `release-check` runs the strict clean-tree, changelog, locked dependency graph, reproducible binding, package/offline, documentation, and test gates. Keeping these phases separate prevents release preparation from failing its own clean-tree check.

Local success is necessary but not sufficient. `release.yml` binds the tag to
the exact `main` commit, requires successful normal CI, builds and consumes all
five prebuilt targets, publishes the complete 27-crate train through Trusted
Publishing, and only then creates the tag and GitHub Release.

## Available Scripts

### 1. `tasks.py` - Task Runner (Recommended)

Convenient shortcuts for common tasks.

```bash
# Run pre-publish checks
python3 tools/tasks.py check

# Update pregenerated bindings
python3 tools/tasks.py bindings

# Preview publishing
python3 tools/tasks.py publish --dry-run

# Run tests
python3 tools/tasks.py test

# Build documentation
python3 tools/tasks.py doc

# Clean build artifacts
python3 tools/tasks.py clean

# Create a release diff, then validate it after commit
python3 tools/tasks.py release-prepare 0.16.0-alpha.2
python3 tools/tasks.py release-check
```

### 2. `xtask release-version` - Unified Version Update

The workspace root is the single version source. Publishable manifests use `version.workspace = true`, and internal dependencies inherit their root workspace declarations. `[workspace.metadata.dear-imgui-release]` is the shared policy for the core package and private package paths/versions; Rust and Python release validators derive package counts from the actual workspace members. Update the release train with:

```bash
cargo run -p xtask -- release-version 0.16.0-alpha.2 --allow-prerelease-relabel
```

The command updates the root release version and inherited internal dependency requirements as one validated workspace operation. It never offers partial crate selection. Documentation remains an explicit review step.

### 3. `publish.py` - Publishing Script

`release.yml` is the normal publishing entry point. `publish.py` provides previews, exact registry verification, and a manual recovery path for the complete dependency train.

```bash
# Dry run (show what would be published)
python3 tools/publish.py --dry-run

# Cargo package dry-run for a selected crate
python3 tools/publish.py --cargo-dry-run --crates dear-imgui-sys

# Manual full-train recovery upload from an authorized environment
python3 tools/publish.py --yes

# Confirm every exact workspace version is available
python3 tools/publish.py --verify-published
```

Print-only `--dry-run` validates metadata without running the full pre-publish
check. `--cargo-dry-run` runs the strict local preflight. Real uploads reject
partial crate selection, target `crates-io` explicitly, recheck the clean Git
fingerprint before every upload, and automatically skip an exact version only
when the published Cargo archive records the same clean candidate commit.
Release authorization belongs to the protected workflow environment and its
short-lived OIDC token; a local recovery operator must verify the exact commit
and normal CI before supplying credentials. `--journal PATH` writes resumable
machine-readable state.

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
- Python workflow/release contracts and source/binding provenance checks pass
- Native source and explicit WASM-safe feature routes pass without using workspace `--all-features`
- Targeted Rust tests pass

### 5. `update_submodule_and_bindings.py` - Bindings Generation

Updates third-party submodules and regenerates pregenerated bindings for `-sys` crates (including optional WASM pregenerated bindings).

```bash
# Update all submodules and regenerate native bindings (all -sys crates)
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules update

# Regenerate bindings only (no submodule updates)
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules skip

# Update specific crate (e.g. dear-imgui-sys only)
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules auto

# Regenerate core binding profiles and compile-check the fixed WASM provider contract
python3 tools/update_submodule_and_bindings.py \
  --crates dear-imgui-sys \
  --submodules skip \
  --wasm
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

The no-argument form remains equivalent to `full`.

### 7. Release Automation

Normal CI is the release gate. The release workflow accepts only the workspace
version tag from `main` and requires a successful `ci.yml` run for the exact
candidate commit. It then invokes `prebuilt-binaries.yml`, where every target
builds all supported profiles and consumes them through an isolated crate
before publication can start.

The workflow retains each target's package archives as ordinary Actions
artifacts. There is no second evidence schema or aggregation layer: a failed CI
job, prebuilt build, or consumer directly fails the release.

## Typical Release Workflow

```bash
# 1. Generate versions, bindings, provenance, and lockfile changes.
python3 tools/tasks.py release-prepare 0.16.0-alpha.2

# 2. Review CHANGELOG.md, compatibility docs, generated files, and Cargo.lock.
git diff

# 3. Commit and validate the exact candidate.
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.2"
python3 tools/tasks.py release-check

# 4. Merge to main, require normal CI to pass, then run the complete release.
gh workflow run release.yml --ref main -f tag=v0.16.0-alpha.2
```

The release workflow acquires a short-lived crates.io token only after all
prebuilt targets pass, resumes exact already-published versions automatically,
and creates the tag and GitHub Release only after all 27 crates are available.

## Common Tasks

### Update Bindings After Upstream Changes

```bash
python3 tools/update_submodule_and_bindings.py \
  --crates all \
  --submodules update \
  --cimgui-branch docking_inter \
  --cimplot-branch master \
  --cimplot3d-branch main \
  --cimnodes-branch master \
  --cimnodes-editor-branch main \
  --cimguizmo-branch master \
  --cimguizmo-quat-branch master \
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

Re-run failed jobs in the same release workflow. The publisher queries every exact version and skips crates that already succeeded:

```bash
gh run rerun RUN_ID --failed
```

Starting a new `release.yml` run is safe but repeats the five-target prebuilt
matrix. Real uploads intentionally do not support `--start-from` or a partial
`--crates` list because the release train is one contract.

### Preview Specific Crates

```bash
python3 tools/publish.py --cargo-dry-run --crates dear-imgui-winit,dear-imgui-wgpu
```

## Requirements

All scripts require:
- **Python 3.11+**
- **cargo** in PATH
- **git** in PATH (for submodule management)

CI publishing additionally requires the protected `release` environment and crates.io Trusted Publisher entries described in [`docs/PUBLISHING.md`](../docs/PUBLISHING.md). A manual fallback requires `CARGO_REGISTRY_TOKEN`; normal CI does not store a registry token.

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

### Publishing Stops Partway Through

Published versions cannot be overwritten. Re-run the failed workflow jobs to complete the same version; exact versions already present are idempotent successes. If the release itself is defective, prepare a new version and optionally yank affected versions:
```bash
cargo yank --registry crates-io --vers <published-version> dear-imgui-sys
python3 tools/tasks.py release-prepare <next-version>
# Review, commit, and merge the candidate, then run the complete release workflow.
python3 tools/tasks.py release-check
gh workflow run release.yml --ref main -f tag=v<next-version>
```

### docs.rs Build Failures

Ensure pregenerated bindings are up-to-date:
```bash
python3 tools/update_submodule_and_bindings.py --crates all
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
