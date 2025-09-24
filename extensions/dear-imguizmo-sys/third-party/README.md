# Third-Party: cimguizmo

This folder is populated by a Git submodule:

- Path: `extensions/dear-imguizmo-sys/third-party/cimguizmo`
- URL: `https://github.com/cimgui/cimguizmo`

Initialize it with:

```
git submodule update --init --recursive extensions/dear-imguizmo-sys/third-party/cimguizmo
```

Verify the following files exist after initialization:

- `cimguizmo/cimguizmo.h`
- `cimguizmo/cimguizmo.cpp`
- `cimguizmo/ImGuizmo/ImGuizmo.cpp`

If the submodule is not initialized, the build script will error with a helpful message.
