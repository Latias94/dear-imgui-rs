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
python3 tools/tasks.py release-prepare 0.16.0-alpha.2

# Review generated bindings, source metadata, Cargo.lock, CHANGELOG.md, and docs.
git diff
git add -A
git commit -m "chore: prepare release v0.16.0-alpha.2"

python3 tools/tasks.py release-check
```

Before merging, require normal CI to pass on Linux, Windows, and macOS. The root `workspace.package.version` is the version source, and `CHANGELOG.md` must contain the matching release section.

## Publish

Merge the candidate to `main`, then dispatch the release workflow with the matching tag:

```bash
gh workflow run release.yml --ref main -f tag=v0.16.0-alpha.2
```

The workflow binds the tag to the workspace version and the exact `main` commit before doing any irreversible work. It then:

1. Requires a successful `main` push CI run for the exact candidate SHA.
2. Builds every prebuilt profile on Linux x86_64, macOS x86_64/aarch64,
   and Windows MSVC `/MD`/`/MT`, then consumes every generated archive from an
   isolated crate.
3. Enters the protected `release` environment and obtains a short-lived
   crates.io OIDC token.
4. Publishes the complete 27-crate dependency train, automatically skipping an
   exact version only when its published Cargo archive records the same clean
   candidate commit.
5. Confirms that all 27 exact versions are available through crates.io and
   Cargo.
6. Generates `SHA256SUMS` and creates the tag and GitHub Release for the same
   commit with all prebuilt archives.

Pushing a tag does not trigger publication. Do not create the tag manually before this workflow; an existing tag is accepted only when it already points to the candidate commit.

## Recovery

crates.io uploads cannot be rolled back. A failed run is resumed by re-running its failed jobs; the publication journal is retained as a workflow artifact, and the publisher queries every exact version and verifies its packaged Git provenance before uploading.

```bash
gh run rerun RUN_ID --failed
```

Starting a new release workflow run is also safe, but it reruns the five-target
prebuilt matrix. Never bump the version merely because one attempt stopped
after publishing only part of the train; first resume the same version and
complete it.

If a published crate is defective, finish or halt the current train deliberately, yank affected versions when appropriate, and prepare a new release version. A crates.io version can never be overwritten.

## Manual fallback

The normal path is `release.yml`. For registry incident recovery, `tools/publish.py` can resume the complete train with a manually supplied crates.io token:

```bash
python3 tools/publish.py --dry-run
python3 tools/publish.py --cargo-dry-run --crates dear-imgui-sys

python3 tools/publish.py --yes
```

Real uploads always operate on the complete release train; `--crates` is limited to previews and Cargo dry-runs. The script fails closed when crates.io state is unavailable, rejects published archives from a different or dirty Git candidate, reconciles Cargo upload errors against the exact registry version, rechecks the clean source commit before every upload, and can write a machine-readable journal with `--journal PATH`. Release authorization is deliberately external: the normal workflow uses the protected `release` environment and a short-lived OIDC token. Before a manual recovery upload, verify the exact commit and successful release workflow yourself.

Verify a completed train without uploading:

```bash
python3 tools/publish.py --verify-published
```

CI passes `--no-verify` only after exact-SHA normal CI and the complete prebuilt
producer/consumer matrix succeed. This keeps the short-lived publishing token
out of build scripts and inside its intended upload window.
