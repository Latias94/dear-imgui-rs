# Bevy Backend Follow-Ups Workstream - Milestones

Status: Closed
Last updated: 2026-05-23

## M0 - Scope And Evidence Freeze

Exit criteria:

- the follow-up problem and target state are explicit;
- the five target slices are listed;
- the relevant ADRs and closed workstreams are linked;
- the initial gate set is named.

Primary evidence:

- `docs/workstreams/bevy-backend-followups-v1/DESIGN.md`
- `docs/workstreams/bevy-backend-followups-v1/TODO.md`

## M1 - Multi-Window And Input Polish

Exit criteria:

- multi-window routing is explicit and tested;
- cursor and IME feedback is less approximate;
- the input policy still matches the backend contract.

Primary gates:

- `cargo +stable nextest run -p dear-imgui-bevy input`

## M2 - Platform And Runtime Proof

Exit criteria:

- wasm/mobile support is either working or split into a narrower follow-on;
- runtime smoke coverage keeps real Bevy render resources honest;
- any new platform gate is documented.

Primary gates:

- `cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features`
- `cargo +stable nextest run -p dear-imgui-bevy --features render`

## M3 - Editor Helper Surface

Exit criteria:

- example code is either shared through a helper surface or consciously kept local;
- docs explain the shipped editor-facing path.

Primary gates:

- `cargo +stable check -p dear-imgui-bevy --features render --example editor_shell`

## M4 - Closeout

Exit criteria:

- the gate set is fresh;
- remaining work is either done or split;
- `WORKSTREAM.json` is updated to match the shipped state.

Result:

- DONE 2026-05-23.
- The five follow-up slices are complete, the closeout gate is fresh, and any future Bevy backend
  follow-up should split to a new lane instead of reopening this one.
