# dear-implot3d-sys

Low-level FFI bindings to ImPlot3D via `cimplot3d` (C API).

## Build

- Requires `dear-imgui-sys` for include paths and defines
- Compiles `cimplot3d.cpp` and `implot3d/*.cpp`
- Windows MSVC CRT matches cargo target features (`crt-static` -> MT)

## Prebuilt

Environment variables:

- `IMPLOT3D_SYS_USE_PREBUILT` / feature `prebuilt`
- `IMPLOT3D_SYS_PREBUILT_URL` or release autodownload
- `IMPLOT3D_SYS_LIB_DIR` to point at an unpacked lib dir
- `IMPLOT3D_SYS_SKIP_CC` to skip compilation

Packaging helper binary (feature `package-bin`) produces
`dear-implot3d-prebuilt-<ver>-<target>-static[-md|mt].tar.gz` containing:

- `lib/<static lib>`
- `include/implot3d/**` + `include/cimplot3d/cimplot3d.h`
- `licenses/*` + `manifest.txt`

