# Third-party sources

The crate packages the following pinned upstream sources:

- `cimCTE`: https://github.com/cimgui/cimCTE at
  `b340b99748f9b13307a8e88b938c4c9f8d77df48`. This revision does not contain a
  standalone license file; its README and source notices are retained verbatim.
- `ImGuiColorTextEdit`: https://github.com/goossens/ImGuiColorTextEdit at the
  `cimCTE` gitlink revision `3b46d759975dfd628ef20fd51b7e1c81ef635be5`.
  Its license is retained at `cimCTE/ImGuiColorTextEdit/LICENSE`.

The revisions are also recorded in `../Cargo.toml` so packaged crates retain
the source identity without Git metadata.
