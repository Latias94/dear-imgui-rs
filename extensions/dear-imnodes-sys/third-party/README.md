# Third-Party: cimnodes

This folder is populated by a Git submodule:

- Path: `extensions/dear-imnodes-sys/third-party/cimnodes`
- URL: `https://github.com/cimgui/cimnodes`

Initialize it with:

```
git submodule update --init --recursive extensions/dear-imnodes-sys/third-party/cimnodes
```

Verify the following files exist after initialization:

- `cimnodes/cimnodes.h`
- `cimnodes/cimnodes.cpp`
- `cimnodes/imnodes/imnodes.cpp`

If the submodule is not initialized, the build script will error with a helpful message.

