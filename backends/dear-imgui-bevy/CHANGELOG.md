# Changelog

## Unreleased

## 0.21.1 - 2026-10-31

### Changed

- Update Bevy to 0.20.1. Applications must upgrade `bevy` and its subcrates to exactly `0.20.1` together with this release.
- Correct `BEVY_TARGET_VERSION` to report `0.20.1`.

## 0.21.0 - 2026-10-17

### Changed

- Reject snapshots containing Dear ImGui 1.93's unsupported `SetSamplerFromTexture` command before texture upload. The whole affected Context epoch is abandoned while other Contexts continue; inspect `ImguiDiagnosticKind::TextureSamplerUnsupported` through `ImguiDiagnostics` with origin `RenderPreparation`. Select linear or nearest sampling explicitly. [PR #94](https://github.com/Latias94/dear-imgui-rs/pull/94)

See the [workspace release notes](../../CHANGELOG.md#0210---2026-10-17) for the shared Dear ImGui 1.93 migration. The external Bevy dependency remains 0.20.0.

## 0.16.0 - 2026-08-14

### Breaking Changes

- `ImguiContexts::{primary_id,ids,contains}` now return `Result` so a retained registry reports terminal App shutdown instead of looking like an active empty registry.
- The public `ImguiInputSystems` set was removed. Register application-owned producers with `ImguiAppExt::add_imgui_input_producers`; backend lifecycle, producer, and commit ordering is private and cannot be suspended through an application run condition.
- Frame systems must be branded with an App-issued `ImguiPass` and registered as sealed `ImguiSystemConfigs`; pass declarations and registration now validate before mutating Bevy schedules.

### Changed

- Each driver run consumes one move-only input transaction containing its route epoch, Context metrics, and cursor/IME authority. Missing producer runs cannot replay prior-frame input, and custom producers commit into the same Context frame.
- Explicit shutdown now retires private pass registrations with the Context registry and leaves retained registry handles in an observable terminal state.

## 0.16.0-alpha.2 - 2026-08-09

### Changed

- Added `ImguiAppExt::try_install_imgui` for typed, App-aware validation before plugin mutation;
  direct `ImguiPlugin` installation remains the panic convenience path.
- Changed `ImguiContexts::remove` into a coalescing managed-retirement request. Completion is
  reported once through generation-qualified `ImguiContextRetired` messages while other Contexts
  continue running.
- Renamed the synchronous owner-recovery path to `try_remove_immediately` so its explicit
  acknowledgement-and-retry contract is visible at the call site.
