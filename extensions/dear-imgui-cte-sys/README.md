# dear-imgui-cte-sys

Preview low-level binding crate for `cimCTE` and its pinned
`ImGuiColorTextEdit` source. Native implementation and generated bindings are
added by the binding-generation layer.

This crate shares the Dear ImGui and cimgui core supplied by `dear-imgui-sys`.
The upstream CMake files are not used because they also compile their own copy
of cimgui and Dear ImGui.

The native library name is `dear_imgui_cte`. Native configuration variables use
the `CTE_SYS_*` prefix.

See `third-party/README.md` for exact upstream revisions and license locations.
