# Third-Party Notices

`dear-imgui-sys` vendors and builds Dear ImGui through cimgui. It also contains
small repository-owned C/C++ shim sources used to expose selected behavior
through a stable Rust-facing C ABI.

## Stack Layout Compatibility Shim

`src/stack_layout_shim.cpp` implements a compatibility layer for
`BeginHorizontal`, `BeginVertical`, `Spring`, `SuspendLayout`, and
`ResumeLayout` style APIs.

These APIs are not official Dear ImGui public APIs. The shim exists so Rust
examples can follow the blueprint-style node editor examples from
`imgui-node-editor` without patching the vendored Dear ImGui submodule.

Reference projects:

- `imgui-node-editor`: <https://github.com/thedmd/imgui-node-editor>
- Dear ImGui discussion about this stack layout extension:
  <https://github.com/ocornut/imgui/discussions/6458>

The compatibility algorithm is derived from the MIT-licensed stack layout
extension vendored by `imgui-node-editor`.

MIT License

Copyright (c) 2019 Michał Cichoń

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
