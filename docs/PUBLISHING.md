# Publishing Guide

This guide explains how to publish new versions of the dear-imgui-rs workspace crates to crates.io.

## Overview

The workspace uses a **unified release train** model where all 27 publishable crates share the same version number. Version 0.16.0 includes the build-support crate, core, backends, extension sys/high-level pairs, and `dear-app`; examples and `xtask` are not published.

## Prerequisites

### 1. Environment Setup

- **Rust toolchain**: Latest stable Rust installed
- **Git**: For version control and submodule management
- **Python 3.11+**: Automation uses the standard-library `tomllib` module
- **Cargo login**: Must be logged in to crates.io
  ```bash
  cargo login <your-api-token>
  ```

### 2. Pre-publish Checklist

Before publishing, ensure:

- [ ] All tests pass on all platforms (Linux, Windows, macOS)
  ```bash
  python3 tools/tasks.py test
  ```
  The task runner keeps mutually incompatible feature families separate; a workspace-wide feature-unified nextest invocation is not a valid substitute.

- [ ] `release-check` and the CI route matrix pass for feature-gated examples, WGPU versions, Ash render modes, platform adapters, node-editor blueprints, and WASM contracts. A default `cargo build --examples` does not cover those routes.

- [ ] The unified workspace release version is updated
  ```bash
  cargo run -p xtask -- release-version 0.16.0
  ```
  - The root `workspace.package.version` is the single source of truth.
  - `[workspace.metadata.dear-imgui-release]` is the single policy for the core package and private package paths/versions; package counts are derived from workspace members.
  - All 27 publishable manifests use `version.workspace = true`.
  - Internal package dependencies inherit the root workspace dependency declarations.
  - Examples and workspace-only tools retain their independent package versions.

- [ ] `CHANGELOG.md` is updated with release notes
  - Keep changelog prose soft-wrapped. Do not hard-wrap paragraphs or bullet text just to fit a fixed column width.
  - Verify the GitHub Release body that CI will use:
    ```bash
    python3 tools/changelog.py check-unreleased
    python3 tools/changelog.py extract --version 0.16.0
    python3 tools/changelog.py check-soft-wrap --version 0.16.0
    ```

- [ ] Documentation is up-to-date
  - [ ] Root `README.md` compatibility table
  - [ ] `docs/COMPATIBILITY.md` with new release train info
  - [ ] Individual crate READMEs if needed

- [ ] Pregenerated native and WASM bindings are up-to-date for `-sys` crates
  ```bash
  # Windows PowerShell
  python tools/update_submodule_and_bindings.py --crates all --profile release --submodules skip --wasm --wasm-ext implot,implot3d,imnodes,imguizmo,imguizmo-quat

  # Linux/macOS
  python3 tools/update_submodule_and_bindings.py --crates all --profile release --submodules skip --wasm --wasm-ext implot,implot3d,imnodes,imguizmo,imguizmo-quat
  ```

- [ ] Core bindings reproduce exactly from the canonical Windows, non-Windows, and WASM specifications
  ```bash
  cargo run -p xtask -- verify-bindings
  ```

- [ ] The packaged `dear-imgui-sys` crate contains all three binding files and exact source metadata, builds from an unpacked crate without `.git`, and passes the offline package-consumption gate in CI

- [ ] Verify `-sys` crates build in docs.rs offline mode
  ```bash
  # Windows PowerShell
  $env:DOCS_RS = '1'; cargo check -p dear-imgui-sys
  $env:DOCS_RS = '1'; cargo check -p dear-implot-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imnodes-sys
  $env:DOCS_RS = '1'; cargo check -p dear-node-editor-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imguizmo-sys
  $env:DOCS_RS = '1'; cargo check -p dear-implot3d-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imguizmo-quat-sys
  $env:DOCS_RS = '1'; cargo check -p dear-imgui-test-engine-sys
  
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

- [ ] The fixed 13-cell `.github/workflows/release-gate.yml` aggregate is `Go`
  for the exact release candidate SHA. Generic branch CI is not a substitute.

- [ ] Git working tree is clean (commit all changes)

## Release Preparation and Final Check

Release preparation and release validation are intentionally separate. Preparation invokes `xtask release-version`, refreshes bindings, metadata, and `Cargo.lock`, so it must not run a clean-tree release gate against its own output.

```bash
# Phase 1: create the 0.16.0 release diff.
python3 tools/tasks.py release-prepare 0.16.0

# Review generated bindings, package metadata, Cargo.lock, CHANGELOG.md, and docs.
git diff
git add -A
git commit -m "chore: prepare release v0.16.0"

# Phase 2: validate the committed, clean release candidate.
python3 tools/tasks.py release-check

# Record this full SHA, dispatch release-gate.yml with candidate_sha, and
# download the successful run's release-gate-<SHA>/gate-result.json artifact.
git rev-parse HEAD
gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
gh run download RELEASE_GATE_RUN_ID \
  --name release-gate-FULL_40_HEX_SHA \
  --dir artifacts/release-gate
```

`release-prepare` may leave the working tree dirty by design. `release-check` is the strict gate: it requires a clean tree, one unified publishable version, a matching changelog section, a locked dependency graph, exact binding/source provenance, package/offline checks, documentation, and tests. Do not publish by skipping the second phase.

The remote gate is independently required for the same SHA. Its immutable
inventory covers Linux Test Engine and real viewport runtimes, Linux WASM,
Windows vcpkg/MSVC `/MD`/`/MT`/GNU, macOS, and five prebuilt producer/consumer
targets. Missing, skipped, cancelled, timed-out, failed, malformed, or wrong-
SHA evidence is `No-Go`.

## Publishing Process

### Automated Publishing (Recommended)

After the local `release-check` and same-SHA remote aggregate both succeed, use
`tools/publish.py` to publish all crates in dependency order:

#### 1. Dry Run (Preview)

First, do a dry run to see what would be published:

```bash
python3 tools/publish.py --dry-run
```

This will show you:
- The order in which crates will be published
- Version numbers for each crate
- Any potential issues

This print-only preview does not run the expensive release gate. `--cargo-dry-run`
reruns the strict clean-tree preflight. Every real upload additionally requires
the authoritative remote `gate-result.json`, verifies its fixed inventory and
exact Git `HEAD`, and only then invokes Cargo.

#### 2. Publish All Crates

Once you've verified the dry run output:

```bash
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json
```

The script will:
1. Verify the downloaded aggregate is a complete 13-cell `Go` for the clean `HEAD`
2. Rerun the strict local `release-check` preflight
3. Show a summary and ask for confirmation
4. Publish each crate explicitly to the `crates-io` registry in dependency order
5. Recheck the source fingerprint before every Cargo upload
6. Wait between publishes for crates.io to index
7. Check if crates are already published and skip if needed

#### 3. Advanced Options

**Publish specific crates:**
```bash
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --crates dear-imgui-sys,dear-imgui-rs
```

**Skip Cargo's per-crate package verification (the strict release preflight still runs):**
```bash
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --no-verify
```

**Adjust wait time between publishes:**
```bash
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --wait 60
```

**Resume from a specific crate:**
```bash
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --start-from dear-implot-sys
```

This is useful if publishing was interrupted and you want to continue from where it stopped.

Do not replace this command with a hand-written `cargo publish` sequence. That
would bypass the authoritative remote aggregate, clean-tree preflight, release
order, and per-upload source fingerprint checks. For interrupted publishing,
resume with `--start-from` and the same verified gate result.

## Post-Publishing

After successful publishing:

### 1. Create Git Tag

Tag the release in git:

```bash
git tag -a v0.16.0 -m "Release v0.16.0"
git push origin v0.16.0
```

### 2. GitHub Release

Pushing a tag does not publish a GitHub Release. Dispatch
`.github/workflows/release.yml` with all three required inputs:

```bash
gh workflow run release.yml \
  -f tag=v0.16.0 \
  -f candidate_sha=FULL_40_HEX_SHA \
  -f gate_run_id=RELEASE_GATE_RUN_ID
```

The workflow checks out the tag, downloads `release-gate-<SHA>` and all retained
release cells from that exact run, verifies the stored aggregate, recomputes it
from the 13 cell payloads, and requires both decisions to be same-SHA `Go`. Only
then does it extract the matching changelog section, create/update the GitHub
Release, and upload the five cells' prebuilt archives.

### 3. Standalone Prebuilt Diagnostic

The release gate always calls the complete five-target prebuilt matrix; it is
not optional and there is no selective crate input. Maintainers may run the
same reusable workflow by itself for diagnosis:

```bash
gh workflow run prebuilt-binaries.yml \
  -f candidate_sha=FULL_40_HEX_SHA
```

This standalone run produces retained cell artifacts but is not a release
aggregate and does not upload GitHub Release assets.

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

Published crate versions are immutable. The script can skip a package that is
already present, but yanking does not make the same version publishable again.
If 0.16.0 is defective, yank the affected package if necessary, prepare a new
patch release, and publish the complete release train at that new version:

```bash
cargo yank --registry crates-io --vers 0.16.0 dear-imgui-sys
python3 tools/tasks.py release-prepare 0.16.1
# Review and commit the release candidate, then:
python3 tools/tasks.py release-check
# Run the new candidate through release-gate.yml, download its Go result, then:
python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json
```

### Publishing Failed

If publishing fails for a crate:

1. **Check the error message**: Often it's a missing dependency or version mismatch
2. **Fix the issue**: Update Cargo.toml or fix the code
3. **Resume publishing**: Use `--start-from` to continue from the failed crate
   ```bash
   python3 tools/publish.py \
     --release-gate-result artifacts/release-gate/gate-result.json \
     --start-from dear-implot-sys
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

- [ ] Run `cargo run -p xtask -- release-version 0.16.0`
- [ ] Verify every publishable manifest inherits `workspace.package.version` and internal dependencies inherit the root workspace declarations
- [ ] Update `CHANGELOG.md`
- [ ] Update `README.md` compatibility table
- [ ] Update `docs/COMPATIBILITY.md`
- [ ] Verify `Cargo.lock` with `cargo metadata --locked --format-version 1 --no-deps`
- [ ] Commit all changes
- [ ] Run full test suite
- [ ] Generate pregenerated bindings
- [ ] Verify docs.rs offline builds
- [ ] Commit the release candidate and run `python3 tools/tasks.py release-check` from a clean tree
- [ ] Dispatch `release-gate.yml` with the exact `candidate_sha`; require all 13
      cells to be same-SHA `Go` and retain the gate run ID/result
- [ ] Publish using `tools/publish.py --release-gate-result ...`
- [ ] Create git tag
- [ ] Dispatch `release.yml` with `tag`, `candidate_sha`, and `gate_run_id`
- [ ] Verify the GitHub Release contains the five already-gated prebuilt cells

## Release Cadence

The project follows a **release train** model:

- **1.0 and later major versions** (1.x → 2.0): Breaking changes and major upstream transitions
- **Pre-1.0 minor versions** (0.15 → 0.16): May intentionally contain source-breaking architecture changes
- **Patch versions** (0.16.0 → 0.16.1): Bug fixes and documentation updates

All 27 publishable crates are versioned together, even if some crates have not
changed. The private examples, web demo, and `xtask` remain independently
versioned workspace members.

## Related Documentation

- [RELEASING.md](./RELEASING.md) - Technical details about sys crate bindings
- [COMPATIBILITY.md](./COMPATIBILITY.md) - Version compatibility matrix
- [README.md](../README.md) - Main project documentation
