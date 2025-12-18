#!/usr/bin/env bash
set -euo pipefail

# CI helper: initialize only the submodules we actually build against.
#
# Why this exists:
# - GitHub Actions `actions/checkout` with `submodules: recursive` will also fetch
#   nested submodules inside third-party repos (e.g. `vcpkg`), which is large and
#   can cause "No space left on device" failures.
# - Some nested submodules are required for building (e.g. cimgui -> imgui),
#   so we explicitly initialize the ones we need and skip the rest.
#
# NOTE: This script assumes top-level submodules are already initialized (we use
# `actions/checkout` with `submodules: true`).

retry() {
  local attempts=5
  local delay=5

  for ((i = 1; i <= attempts; i++)); do
    if "$@"; then
      return 0
    fi
    if ((i < attempts)); then
      echo "Command failed (attempt $i/$attempts): $*" >&2
      echo "Retrying in ${delay}s..." >&2
      sleep "$delay"
      delay=$((delay * 2))
    fi
  done

  echo "Command failed after $attempts attempts: $*" >&2
  return 1
}

echo "::group::Init nested submodules (selective)"

# cimgui -> imgui (required for dear-imgui-sys)
retry git -C dear-imgui-sys/third-party/cimgui submodule update --init --depth=1 imgui

# cimplot -> implot (required for dear-implot-sys). Avoid shallow to ensure the pinned commit exists.
retry git -C extensions/dear-implot-sys/third-party/cimplot submodule update --init implot

# cimplot3d -> implot3d (required for dear-implot3d-sys). Avoid shallow to ensure the pinned commit exists.
retry git -C extensions/dear-implot3d-sys/third-party/cimplot3d submodule update --init implot3d

# cimguizmo -> ImGuizmo (required for dear-imguizmo-sys)
retry git -C extensions/dear-imguizmo-sys/third-party/cimguizmo submodule update --init --depth=1 ImGuizmo

# cimguizmo_quat -> imGuIZMO.quat, then its libs/imgui (required for dear-imguizmo-quat-sys)
retry git -C extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat submodule update --init --depth=1 imGuIZMO.quat
retry git -C extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat/imGuIZMO.quat submodule update --init --depth=1 libs/imgui

# cimnodes -> imnodes (required for dear-imnodes-sys). Do NOT recurse further (imnodes includes a large vcpkg submodule).
retry git -C extensions/dear-imnodes-sys/third-party/cimnodes submodule update --init --depth=1 imnodes

echo "::endgroup::"

