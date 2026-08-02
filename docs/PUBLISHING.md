# Publishing Guide

The workspace uses one release train: all 27 publishable crates inherit `workspace.package.version` and must be published together. Examples, the web demo, and `xtask` are not published.

## One-time setup

The release workflow uses crates.io Trusted Publishing. It does not store a crates.io token and does not require `cargo login`.

1. Create a protected GitHub Environment named `release`.
2. Require maintainer approval for that environment and restrict deployments to `main`.
3. For each of the 27 crates on crates.io, add the same GitHub Trusted Publisher configuration: repository owner `Latias94`, repository `dear-imgui-rs`, workflow `release.yml`, environment `release`.

The repository workflow token defaults to read-only. Only the crates.io job receives `id-token: write`, and only the final GitHub Release job receives `contents: write`.

## Prepare a candidate

Preparation intentionally changes the worktree; validation requires the resulting commit to be clean.

```bash
python3 tools/tasks.py release-prepare 0.16.0-alpha.1

# Review generated bindings, source metadata, Cargo.lock, CHANGELOG.md, and docs.
git diff
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.1"

python3 tools/tasks.py release-check
```

Before merging, require normal CI to pass on Linux, Windows, and macOS. The root `workspace.package.version` is the version source, and `CHANGELOG.md` must contain the matching release section.

## Publish

Merge the candidate to `main`, then dispatch the release workflow with the matching tag:

```bash
gh workflow run release.yml --ref main -f tag=v0.16.0-alpha.1
```

The workflow binds the tag to the workspace version and the exact `main` commit before doing any irreversible work. It then:

1. Runs the fixed 16-cell Release Gate for that commit, including native runtime tests, Winit/WGPU and SDL3/Glow viewport smokes, WASM, Windows ABI/native dependency routes, macOS, all source packages, and five prebuilt producer/consumer targets.
2. Recomputes the authoritative decision from retained cell payloads and stages only the prebuilt archives recorded by successful cells.
3. Generates `SHA256SUMS` and a release manifest for the exact staged assets.
4. Enters the protected `release` environment and obtains a short-lived crates.io OIDC token.
5. Publishes the complete 27-crate dependency train, automatically skipping an exact version only when its published Cargo archive records the same clean candidate commit.
6. Confirms that all 27 exact versions are available through crates.io and Cargo and carry that candidate provenance.
7. Creates the tag and GitHub Release for the same commit, rejecting pre-existing unexpected assets and verifying that the final download inventory contains exactly the staged archives and checksums.

Pushing a tag does not trigger publication. Do not create the tag manually before this workflow; an existing tag is accepted only when it already points to the candidate commit.

## Recovery

crates.io uploads cannot be rolled back. A failed run is resumed by re-running its failed jobs; the publication journal is retained as a workflow artifact, and the publisher queries every exact version and verifies its packaged Git provenance before uploading.

```bash
gh run rerun RUN_ID --failed
```

Starting a new release workflow run is also safe, but it reruns the full Release Gate. Never bump the version merely because one attempt stopped after publishing only part of the train; first resume the same version and complete it.

If a published crate is defective, finish or halt the current train deliberately, yank affected versions when appropriate, and prepare a new release version. A crates.io version can never be overwritten.

## Diagnostic Gate

`release-gate.yml` remains independently dispatchable for diagnostics without publishing:

```bash
git rev-parse HEAD
gh workflow run release-gate.yml -f candidate_sha=FULL_40_HEX_SHA
```

A cell that is missing, skipped, cancelled, timed out, malformed, duplicated, or bound to another SHA makes the aggregate `No-Go`. Headless Test Engine success does not replace real viewport runtime cells.

## Manual fallback

The normal path is `release.yml`. For registry incident recovery, `tools/publish.py` can use a downloaded same-SHA `gate-result.json` and a manually supplied crates.io token:

```bash
python3 tools/publish.py --dry-run
python3 tools/publish.py --cargo-dry-run --crates dear-imgui-sys

python3 tools/publish.py \
  --release-gate-result artifacts/release-gate/gate-result.json \
  --yes
```

Real uploads always operate on the complete release train; `--crates` is limited to previews and Cargo dry-runs. The script fails closed when crates.io state is unavailable, rejects published archives from a different or dirty Git candidate, reconciles Cargo upload errors against the exact registry version, rechecks the clean source commit before every upload, and can write a machine-readable journal with `--journal PATH`.

Verify a completed train without uploading:

```bash
python3 tools/publish.py --verify-published
```

CI passes `--no-verify` only after the source-package cell has already packaged and consumed every release crate without credentials. This keeps the short-lived publishing token out of build scripts and inside its intended upload window.
