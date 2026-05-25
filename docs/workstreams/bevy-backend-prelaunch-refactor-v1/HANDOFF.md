# Bevy Backend Prelaunch Refactor - Handoff

Status: Closed
Last updated: 2026-05-25

## Current State

The workstream is closed. The backend prelaunch refactor landed in the working tree and all closeout
gates recorded in `EVIDENCE_AND_GATES.md` pass with
`CARGO_TARGET_DIR=target/bevy-backend-prelaunch`.

## Closeout Task

- Task ID: BPR-070
- Owner: codex
- Files: `backends/dear-imgui-bevy/src/render.rs`, `src/input.rs`, `src/viewport.rs`, tests,
  examples, README, workstream docs
- Validation: full Bevy backend gate set in `EVIDENCE_AND_GATES.md`
- Status: DONE
- Review: no blocking findings at closeout
- Evidence: DONE 2026-05-25

## Decisions Since Last Update

- Open a new workstream instead of reopening the closed docking multi-viewport lane.
- Treat breaking Bevy backend API changes as acceptable before public release.
- Permit `dear-imgui-rs` core refactoring when required by Bevy-native correctness.
- Add `render::ImguiOverlayCamera` as the explicit overlay target marker while keeping the
  highest-order fallback for simple apps.
- Preserve Bevy image sampler semantics by moving samplers into texture bind groups.
- Treat the nextest discovery hang as shared-target artifact pollution and require a dedicated
  target directory for local release checks.

## Blockers

- None.

## Next Recommended Action

- The prelaunch refactor and closeout docs landed in commit
  `654ac26 feat(bevy): finalize prelaunch backend refactor`.
- Use the residual risks below as seeds for any new issue-audit or follow-up workstream.

## Residual Risks

- GPU output correctness for split-screen viewports is covered by extraction/preparation unit tests,
  not by a pixel-level render harness.
- wasm check passes, but browser runtime IME/clipboard behavior remains host-dependent and documented
  as unsupported/limited.
