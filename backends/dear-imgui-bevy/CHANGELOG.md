# Changelog

## 0.16.0-alpha.2 - Unreleased

### Changed

- Added `ImguiAppExt::try_install_imgui` for typed, App-aware validation before plugin mutation;
  direct `ImguiPlugin` installation remains the panic convenience path.
- Changed `ImguiContexts::remove` into a coalescing managed-retirement request. Completion is
  reported once through generation-qualified `ImguiContextRetired` messages while other Contexts
  continue running.
- Renamed the synchronous owner-recovery path to `try_remove_immediately` so its explicit
  acknowledgement-and-retry contract is visible at the call site.
