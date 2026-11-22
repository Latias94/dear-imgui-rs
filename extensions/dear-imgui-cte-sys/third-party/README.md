# Third-party sources

The crate packages the following pinned upstream sources:

- `cimCTE`: https://github.com/cimgui/cimCTE at
  `dbf2abe4841b9dd8a3f1f1864038ac9d60155c5e`. This revision does not contain a
  standalone license file; its README and source notices are retained verbatim.
- `ImGuiColorTextEdit`: https://github.com/goossens/ImGuiColorTextEdit at the
  `cimCTE` gitlink revision `7515d1c765f8f9d47cd704fd3fbd20eb03fcc304`.
  Its license is retained at `cimCTE/ImGuiColorTextEdit/LICENSE`.

The revisions are also recorded in `../Cargo.toml` so packaged crates retain
the source identity without Git metadata.
