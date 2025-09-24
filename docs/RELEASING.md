# Releasing

This repository is a Cargo workspace containing multiple crates:

- Core: `dear-imgui`, `dear-imgui-sys`
- Backends: `dear-imgui-winit`, `dear-imgui-wgpu`, `dear-imgui-glow`
- Extensions: `dear-imguizmo[-sys]`, `dear-imnodes[-sys]`, `dear-implot[-sys]`

We use independent versioning per crate and automate releases with
[release-plz](https://github.com/MarcoIeni/release-plz).

## Versioning policy

- Crates are versioned independently (no shared workspace version).
- Follow semantic versioning. For 0.y.z series, treat `y` as the compatibility band.
- Breaking changes MUST be marked in commits (see below) and cause a version bump.

## Conventional commits

Use Conventional Commits to drive changelogs and suggested bumps:

- `feat(scope): ...` → minor bump (or major if `!`/breaking)
- `fix(scope): ...` → patch bump
- `perf:`, `refactor:`, `docs:`, `build:`, `ci:`, `chore:`, `test:` are grouped accordingly
- Add `!` to the type (e.g. `feat!:`) or include `BREAKING CHANGE: ...` in the footer for breaking changes
- Scope tip: include the crate name when possible, e.g. `fix(imnodes): ...`

## Changelog

- Workspace changelog configuration lives in `.github/changelog.toml` (git-cliff).
- Per-crate changelogs:
  - `dear-imgui`: `CHANGELOG.md` at repo root
  - `dear-imguizmo`: `extensions/dear-imguizmo/CHANGELOG.md`
  - `dear-imnodes`: `extensions/dear-imnodes/CHANGELOG.md`
  - `dear-implot`: (optional) can add a crate-specific changelog if/when desired
- `-sys` crates do not require a changelog entry by policy.

## Release flow

1) Push your changes (PR) → CI builds on Linux/macOS/Windows.
2) On merges to `main`, `release-plz` opens/updates a "Release" PR:
   - Bumps versions for crates with changes
   - Updates changelogs using conventional commits
   - Updates dependent versions when needed
3) Review the Release PR; ensure examples build with extensions.
4) Merge the Release PR:
   - `release-plz` publishes changed crates to crates.io in dependency order
   - Creates GitHub Releases for `dear-imgui`, `dear-imguizmo`, and `dear-imnodes` (safe crates)

## Tags and GitHub Releases

- Per-crate tags are created (e.g. `dear-imnodes-v0.1.3`).
- GitHub Releases are enabled for safe crates, disabled for `-sys` crates.

## Dry-run publish check

CI runs a `cargo publish --dry-run` matrix for major crates to validate packaging.
Run locally as needed:

```
for p in dear-imgui dear-imgui-winit dear-imgui-wgpu dear-imgui-glow \
         dear-imguizmo dear-imnodes dear-implot; do
  cargo publish --dry-run -p $p
done
```

## Notes for `-sys` crates

- Avoid mandatory network downloads in `build.rs` (provide env/feature switches).
- Ensure `include`/`exclude` in Cargo.toml are set to ship only required files.
- Keep `links` unique per crate.

