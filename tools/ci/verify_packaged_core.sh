#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$workspace_root"

target_dir="${CARGO_TARGET_DIR:-$workspace_root/target/ci-package}"
work_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/dear-imgui-package.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
package_workspace="$work_dir/repository"
publish_order_file="$work_dir/publish-order.txt"

if [[ -n "$(git status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ]]; then
    echo "::error::package verification requires a clean source checkout"
    git status --short --ignore-submodules=none
    exit 1
fi

PYTHONPATH="$workspace_root/tools${PYTHONPATH:+:$PYTHONPATH}" \
    python3 - "$workspace_root" "$publish_order_file" <<'PY'
import sys
from pathlib import Path

from release_metadata import (
    PUBLISH_ORDER,
    load_workspace_metadata,
    validate_publish_order,
    validate_release_workspace,
)

repo_root = Path(sys.argv[1]).resolve()
publish_order_file = Path(sys.argv[2])
metadata = load_workspace_metadata(repo_root)
errors = [
    *validate_release_workspace(metadata),
    *validate_publish_order(metadata, PUBLISH_ORDER, repo_root),
]
if errors:
    print("release workspace validation failed:", file=sys.stderr)
    for error in errors:
        print(f"  - {error}", file=sys.stderr)
    raise SystemExit(1)

publish_order_file.write_text(
    "".join(f"{name}\n" for name, _path in PUBLISH_ORDER),
    encoding="utf-8",
)
PY

# Cargo records an otherwise-unused registry patch in the workspace lock file before
# it verifies the stripped package manifest. Keep that lock adjustment inside a
# standalone clean clone so the real checkout stays immutable and `--locked` remains
# meaningful for the package and consumer builds.
git clone --quiet --local --no-hardlinks "$workspace_root" "$package_workspace"
git -C "$package_workspace" submodule update --init dear-imgui-sys/third-party/cimgui
git -C "$package_workspace/dear-imgui-sys/third-party/cimgui" submodule update --init imgui

package_version() {
    local package_name="$1"
    cargo metadata \
        --manifest-path "$package_workspace/Cargo.toml" \
        --locked \
        --no-deps \
        --format-version 1 | python3 -c '
import json
import sys

package_name = sys.argv[1]
metadata = json.load(sys.stdin)
for package in metadata["packages"]:
    if package["name"] == package_name:
        print(package["version"])
        break
else:
    raise SystemExit(f"workspace package not found: {package_name}")
' "$package_name"
}

extract_crate() {
    local archive="$1"
    local destination="$2"
    mkdir -p "$destination"
    tar -xzf "$archive" -C "$destination"
}

helper_version="$(package_version dear-imgui-build-support)"
sys_version="$(package_version dear-imgui-sys)"

echo "::group::Package dear-imgui-build-support"
(
    cd "$package_workspace"
    CARGO_TARGET_DIR="$target_dir" cargo package -p dear-imgui-build-support --locked
)
echo "::endgroup::"

helper_archive="$target_dir/package/dear-imgui-build-support-$helper_version.crate"
test -f "$helper_archive"
extract_crate "$helper_archive" "$work_dir/helper"
helper_path="$work_dir/helper/dear-imgui-build-support-$helper_version"
test -f "$helper_path/Cargo.toml"

helper_patch="patch.crates-io.dear-imgui-build-support.path=\"$helper_path\""

(
    cd "$package_workspace"
    cargo metadata --format-version 1 --config "$helper_patch" > /dev/null
    git add Cargo.lock
    if ! git diff --cached --quiet; then
        git \
            -c user.name='Dear ImGui CI' \
            -c user.email='ci@example.invalid' \
            -c commit.gpgsign=false \
            commit -m 'ci: lock packaged build helper patch' > /dev/null
    fi
    if [[ -n "$(git status --porcelain=v1 --ignore-submodules=none)" ]]; then
        echo "::error::temporary package workspace is not clean"
        git status --short --ignore-submodules=none
        exit 1
    fi
)

echo "::group::Package dear-imgui-sys with the packaged build helper"
(
    cd "$package_workspace"
    CARGO_TARGET_DIR="$target_dir" cargo package \
        -p dear-imgui-sys \
        --locked \
        --config "$helper_patch"
)
echo "::endgroup::"

sys_archive="$target_dir/package/dear-imgui-sys-$sys_version.crate"
test -f "$sys_archive"

archive_listing="$work_dir/dear-imgui-sys-files.txt"
tar -tzf "$sys_archive" > "$archive_listing"
archive_root="dear-imgui-sys-$sys_version"

for required_path in \
    "$archive_root/src/bindings_pregenerated.rs" \
    "$archive_root/src/bindings_pregenerated_windows.rs" \
    "$archive_root/src/wasm_bindings_pregenerated.rs"; do
    if ! grep -Fxq "$required_path" "$archive_listing"; then
        echo "::error::packaged dear-imgui-sys is missing $required_path"
        exit 1
    fi
done

if grep -Eq '(^|/)\.git(/|$)' "$archive_listing"; then
    echo "::error::packaged dear-imgui-sys contains a .git entry"
    grep -E '(^|/)\.git(/|$)' "$archive_listing"
    exit 1
fi

extract_crate "$sys_archive" "$work_dir/sys"
sys_path="$work_dir/sys/$archive_root"

python3 - "$package_workspace/dear-imgui-sys/Cargo.toml" "$sys_path/Cargo.toml" <<'PY'
import re
import sys
import tomllib
from pathlib import Path

source_manifest, packaged_manifest = map(Path, sys.argv[1:])
with source_manifest.open("rb") as source_file:
    source = tomllib.load(source_file)
with packaged_manifest.open("rb") as packaged_file:
    packaged = tomllib.load(packaged_file)

section = "dear-imgui-sources"
expected = source["package"]["metadata"][section]
actual = packaged["package"]["metadata"][section]
required_keys = {"cimgui-revision", "imgui-revision"}
if set(actual) != required_keys:
    raise SystemExit(
        f"packaged [{section}] keys differ: expected {sorted(required_keys)}, "
        f"found {sorted(actual)}"
    )
if actual != expected:
    raise SystemExit(
        f"packaged [{section}] differs from the source manifest: "
        f"expected {expected}, found {actual}"
    )
for key, revision in actual.items():
    if re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise SystemExit(f"packaged {key} is not an exact Git revision: {revision}")
PY

echo "::group::Consume unpacked dear-imgui-sys offline"
CARGO_TARGET_DIR="$target_dir/offline-consumer" cargo check \
    --manifest-path "$sys_path/Cargo.toml" \
    --offline \
    --locked \
    --config "$helper_patch"
echo "::endgroup::"

echo "::group::Inspect every publishable workspace package"
publishable_packages=()
while IFS= read -r package_name; do
    publishable_packages+=("$package_name")
done < "$publish_order_file"

for package_name in "${publishable_packages[@]}"; do
    echo "Checking package file list: $package_name"
    CARGO_TARGET_DIR="$target_dir" cargo package -p "$package_name" --list --locked > /dev/null
done
echo "::endgroup::"

echo "Verified packaged core crates and ${#publishable_packages[@]} publishable package file lists."
