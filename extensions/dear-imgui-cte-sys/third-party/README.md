# Third-party sources

The crate packages the following pinned upstream sources:

- `cimCTE`: https://github.com/cimgui/cimCTE at
  `3cdee0b5e1d8f0a59a40fda80c4a6b894d752240`. This revision does not contain a
  standalone license file; its README and source notices are retained verbatim.
- `ImGuiColorTextEdit`: https://github.com/goossens/ImGuiColorTextEdit at the
  `cimCTE` gitlink revision `f28136480fa4091164e0b528dc9cca147c5a6ee9`.
  Its license is retained at `cimCTE/ImGuiColorTextEdit/LICENSE`.

The revisions are also recorded in `../Cargo.toml` so packaged crates retain
the source identity without Git metadata.
