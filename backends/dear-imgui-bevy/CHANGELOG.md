# Changelog

## Unreleased

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
