#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

usage() {
    cat <<'EOF'
Usage:
  tools/ci/verify_packaged_core.sh
  tools/ci/verify_packaged_core.sh --verify-prebuilt-packages PACKAGE_DIR TARGET [CRT]

The default mode validates every publishable .crate from a clean clone and performs
host round-trips for the normal and stack-layout Dear ImGui native artifacts. The
prebuilt-only mode consumes artifacts already produced by the binary workflow.
EOF
}

host_target() {
    rustc -vV | sed -n 's/^host: //p'
}

write_prebuilt_consumer() {
    local destination="$1"
    local source_root="$2"
    local profile="$3"

    mkdir -p "$destination/src"
    python3 - "$destination" "$source_root/dear-imgui" "$profile" <<'PY'
import json
import sys
from pathlib import Path

destination = Path(sys.argv[1])
dependency_path = Path(sys.argv[2]).resolve()
profile = sys.argv[3]
features = ', features = ["stack-layout"]' if profile == "stack-layout" else ""
destination.joinpath("Cargo.toml").write_text(
    "\n".join(
        (
            "[package]",
            f'name = "dear-imgui-prebuilt-{profile}"',
            'version = "0.0.0"',
            'edition = "2024"',
            "publish = false",
            "",
            "[dependencies]",
            (
                "dear-imgui-rs = { path = "
                f"{json.dumps(str(dependency_path))}, default-features = false{features} }}"
            ),
            "",
            "[workspace]",
            "",
        )
    ),
    encoding="utf-8",
)

if profile == "stack-layout":
    frame_body = """
        let layout = ui.begin_horizontal("artifact-row", [0.0, 0.0], -1.0);
        ui.text("stack-layout artifact");
        layout.end();
"""
else:
    frame_body = '        ui.text("normal artifact");\n'

destination.joinpath("src/main.rs").write_text(
    f"""fn main() {{
    let mut context = dear_imgui_rs::Context::create();
    context.io_mut().set_display_size([320.0, 240.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let _ = context.font_atlas_mut().build();
    {{
        let ui = context.frame();
{frame_body}    }}
    assert!(context.render().valid());
    assert!(!dear_imgui_rs::dear_imgui_version().is_empty());
}}
""",
    encoding="utf-8",
)
PY
    cp "$source_root/Cargo.lock" "$destination/Cargo.lock"
}

select_core_prebuilt_archives() {
    local package_dir="$1"
    local target="$2"
    local crt="$3"
    local output_dir="$4"

    python3 - "$package_dir" "$target" "$crt" "$output_dir" <<'PY'
import sys
import tarfile
from pathlib import Path

package_dir = Path(sys.argv[1]).resolve()
expected_target = sys.argv[2]
expected_crt = sys.argv[3]
output_dir = Path(sys.argv[4])
profiles = {
    frozenset(("platform-io-aggregate-hooks", "wchar32")): "normal",
    frozenset(("platform-io-aggregate-hooks", "stack-layout", "wchar32")): "stack-layout",
}
matches = {name: [] for name in profiles.values()}

for archive in sorted(package_dir.glob("dear-imgui-*.tar.gz")):
    with tarfile.open(archive, "r:gz") as package:
        manifest_member = next(
            (member for member in package.getmembers() if member.name.lstrip("./") == "manifest.txt"),
            None,
        )
        if manifest_member is None:
            raise SystemExit(f"{archive} does not contain manifest.txt")
        extracted = package.extractfile(manifest_member)
        if extracted is None:
            raise SystemExit(f"could not read manifest.txt from {archive}")
        lines = extracted.read().decode("utf-8").splitlines()
    fields = dict(line.split("=", 1) for line in lines[1:] if "=" in line)
    if fields.get("target") != expected_target:
        continue
    if expected_crt and fields.get("crt") != expected_crt:
        continue
    features = frozenset(filter(None, fields.get("features", "").split(",")))
    profile = profiles.get(features)
    if profile is not None:
        matches[profile].append(archive)

for profile, archives in matches.items():
    if len(archives) != 1:
        rendered = ", ".join(str(path) for path in archives) or "none"
        raise SystemExit(
            f"expected exactly one {profile} archive for target={expected_target!r} "
            f"crt={expected_crt!r}, found {rendered}"
        )
    output_dir.joinpath(f"{profile}.path").write_text(
        str(archives[0]), encoding="utf-8"
    )
PY
}

run_prebuilt_consumer() {
    local label="$1"
    local consumer_dir="$2"
    local artifact_root="$3"
    local target="$4"
    local target_dir="$5"

    echo "::group::Run $label prebuilt consumer"
    CARGO_TARGET_DIR="$target_dir" cargo metadata \
        --quiet \
        --manifest-path "$consumer_dir/Cargo.toml" \
        --format-version 1 > /dev/null
    CARGO_TARGET_DIR="$target_dir" cargo fetch \
        --manifest-path "$consumer_dir/Cargo.toml" \
        --target "$target" \
        --locked
    (
        unset IMGUI_SYS_FORCE_BUILD IMGUI_SYS_PREBUILT_URL IMGUI_SYS_USE_PREBUILT
        IMGUI_SYS_SKIP_CC=1 \
        IMGUI_SYS_LIB_DIR="$artifact_root/lib" \
        CARGO_TARGET_DIR="$target_dir" \
            cargo run \
                --manifest-path "$consumer_dir/Cargo.toml" \
                --target "$target" \
                --locked \
                --offline
    )
    echo "::endgroup::"
}

reject_prebuilt_profile_mismatch() {
    local label="$1"
    local consumer_dir="$2"
    local artifact_root="$3"
    local target="$4"
    local target_dir="$5"
    local log="$6"

    echo "::group::Reject $label prebuilt profile mismatch"
    if (
        unset IMGUI_SYS_FORCE_BUILD IMGUI_SYS_PREBUILT_URL IMGUI_SYS_USE_PREBUILT
        IMGUI_SYS_SKIP_CC=1 \
        IMGUI_SYS_LIB_DIR="$artifact_root/lib" \
        CARGO_TARGET_DIR="$target_dir" \
            cargo check \
                --manifest-path "$consumer_dir/Cargo.toml" \
                --target "$target" \
                --locked \
                --offline
    ) >"$log" 2>&1; then
        cat "$log"
        echo "::error::$label profile mismatch unexpectedly succeeded"
        exit 1
    fi
    if ! grep -Fq "selected an incompatible dear_imgui artifact" "$log"; then
        cat "$log"
        echo "::error::$label failed without the strict artifact profile diagnostic"
        exit 1
    fi
    echo "Verified expected profile rejection: $label"
    echo "::endgroup::"
}

verify_core_prebuilt_packages() {
    local package_dir="$1"
    local target="$2"
    local crt="${3:-}"
    local source_root="${4:-$workspace_root}"
    local roundtrip_work_dir
    roundtrip_work_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/dear-imgui-prebuilt-consumer.XXXXXX")"

    select_core_prebuilt_archives "$package_dir" "$target" "$crt" "$roundtrip_work_dir"
    local normal_archive stack_archive
    normal_archive="$(<"$roundtrip_work_dir/normal.path")"
    stack_archive="$(<"$roundtrip_work_dir/stack-layout.path")"

    mkdir -p "$roundtrip_work_dir/artifacts/normal" "$roundtrip_work_dir/artifacts/stack-layout"
    tar -xzf "$normal_archive" -C "$roundtrip_work_dir/artifacts/normal"
    tar -xzf "$stack_archive" -C "$roundtrip_work_dir/artifacts/stack-layout"
    test -f "$roundtrip_work_dir/artifacts/normal/manifest.txt"
    test -f "$roundtrip_work_dir/artifacts/stack-layout/manifest.txt"

    write_prebuilt_consumer "$roundtrip_work_dir/consumers/normal" "$source_root" normal
    write_prebuilt_consumer \
        "$roundtrip_work_dir/consumers/stack-layout" \
        "$source_root" \
        stack-layout

    run_prebuilt_consumer \
        normal \
        "$roundtrip_work_dir/consumers/normal" \
        "$roundtrip_work_dir/artifacts/normal" \
        "$target" \
        "$roundtrip_work_dir/targets/normal"
    run_prebuilt_consumer \
        stack-layout \
        "$roundtrip_work_dir/consumers/stack-layout" \
        "$roundtrip_work_dir/artifacts/stack-layout" \
        "$target" \
        "$roundtrip_work_dir/targets/stack-layout"

    reject_prebuilt_profile_mismatch \
        normal-consumer-with-stack-layout-artifact \
        "$roundtrip_work_dir/consumers/normal" \
        "$roundtrip_work_dir/artifacts/stack-layout" \
        "$target" \
        "$roundtrip_work_dir/targets/mismatch-normal" \
        "$roundtrip_work_dir/mismatch-normal.log"
    reject_prebuilt_profile_mismatch \
        stack-layout-consumer-with-normal-artifact \
        "$roundtrip_work_dir/consumers/stack-layout" \
        "$roundtrip_work_dir/artifacts/normal" \
        "$target" \
        "$roundtrip_work_dir/targets/mismatch-stack-layout" \
        "$roundtrip_work_dir/mismatch-stack-layout.log"

    rm -rf "$roundtrip_work_dir"
    echo "Verified normal and stack-layout prebuilt consumer round-trips for $target."
}

if [[ "${1:-}" == "--verify-prebuilt-packages" ]]; then
    if [[ $# -lt 3 || $# -gt 4 ]]; then
        usage >&2
        exit 2
    fi
    verify_core_prebuilt_packages "$2" "$3" "${4:-}" "$workspace_root"
    exit 0
elif [[ $# -ne 0 ]]; then
    usage >&2
    exit 2
fi

cd "$workspace_root"

work_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/dear-imgui-package.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
target_dir="$work_dir/target"
package_workspace="$work_dir/repository"
publish_order_file="$work_dir/publish-order.tsv"

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
    "".join(
        f"{name}\t{path}\t{metadata.package(name).version}\n"
        for name, path in PUBLISH_ORDER
    ),
    encoding="utf-8",
)
PY

# Keep Cargo's registry patch lock adjustments and all package preparation inside a
# standalone clone. The source checkout remains immutable and the generated archives
# therefore describe exactly HEAD, including every required top-level and nested source
# submodule.
git clone --quiet --local --no-hardlinks "$workspace_root" "$package_workspace"
while read -r key submodule_path; do
    submodule_name="${key#submodule.}"
    submodule_name="${submodule_name%.path}"
    if git -C "$workspace_root/$submodule_path" rev-parse --git-dir > /dev/null 2>&1; then
        git -C "$package_workspace" config \
            "submodule.$submodule_name.url" \
            "$workspace_root/$submodule_path"
    fi
done < <(git -C "$package_workspace" config -f .gitmodules --get-regexp '^submodule\..*\.path$')
git -C "$package_workspace" -c protocol.file.allow=always submodule update --init
while IFS=$'\t' read -r parent nested; do
    nested_key="$(
        git -C "$package_workspace/$parent" \
            config -f .gitmodules --get-regexp '^submodule\..*\.path$' |
            awk -v nested="$nested" '$2 == nested { print $1 }'
    )"
    if [[ -z "$nested_key" ]]; then
        echo "::error::nested submodule $parent/$nested is not declared"
        exit 1
    fi
    nested_name="${nested_key#submodule.}"
    nested_name="${nested_name%.path}"
    if git -C "$workspace_root/$parent/$nested" rev-parse --git-dir > /dev/null 2>&1; then
        git -C "$package_workspace/$parent" config \
            "submodule.$nested_name.url" \
            "$workspace_root/$parent/$nested"
    fi
    git -C "$package_workspace/$parent" \
        -c protocol.file.allow=always \
        submodule update --init "$nested"
done <<'EOF'
dear-imgui-sys/third-party/cimgui	imgui
extensions/dear-implot-sys/third-party/cimplot	implot
extensions/dear-imnodes-sys/third-party/cimnodes	imnodes
extensions/dear-node-editor-sys/third-party/cimnodes_editor	imgui-node-editor
extensions/dear-imguizmo-sys/third-party/cimguizmo	ImGuizmo
extensions/dear-implot3d-sys/third-party/cimplot3d	implot3d
extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat	imGuIZMO.quat
EOF

package_version() {
    local package_name="$1"
    awk -F '\t' -v package_name="$package_name" \
        '$1 == package_name { print $3; found = 1 } END { if (!found) exit 1 }' \
        "$publish_order_file"
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

PYTHONPATH="$workspace_root/tools${PYTHONPATH:+:$PYTHONPATH}" \
    python3 - "$package_workspace/dear-imgui-sys/Cargo.toml" "$sys_path/Cargo.toml" <<'PY'
import sys
from pathlib import Path

from source_metadata import read_core_source_metadata

source_manifest, packaged_manifest = map(Path, sys.argv[1:])
expected = read_core_source_metadata(source_manifest)
actual = read_core_source_metadata(packaged_manifest)
if actual != expected:
    raise SystemExit(
        "packaged [package.metadata.dear-imgui-sources] differs from the source manifest: "
        f"expected {expected}, found {actual}"
    )
PY

echo "::group::Consume unpacked dear-imgui-sys offline"
CARGO_TARGET_DIR="$target_dir/offline-consumer" cargo check \
    --manifest-path "$sys_path/Cargo.toml" \
    --offline \
    --locked \
    --config "$helper_patch"
echo "::endgroup::"

# Patch every unpublished workspace dependency back to this clean clone while Cargo
# prepares and verifies source archives. Cargo unpacks each normalized archive and
# builds it against this complete local release graph before the gate accepts it.
cargo_patch_args=()
while IFS=$'\t' read -r package_name package_path _package_version; do
    cargo_patch_args+=(
        --config
        "patch.crates-io.$package_name.path=\"$package_workspace/$package_path\""
    )
done < "$publish_order_file"

(
    cd "$package_workspace"
    cargo metadata --quiet --format-version 1 "${cargo_patch_args[@]}" > /dev/null
    git add Cargo.lock
    if ! git diff --cached --quiet; then
        git \
            -c user.name='Dear ImGui CI' \
            -c user.email='ci@example.invalid' \
            -c commit.gpgsign=false \
            commit -m 'ci: lock unpublished workspace package patches' > /dev/null
    fi
    if [[ -n "$(git status --porcelain=v1 --ignore-submodules=none)" ]]; then
        echo "::error::temporary package workspace is not clean after locking patches"
        git status --short --ignore-submodules=none
        exit 1
    fi
    cargo fetch --quiet --locked "${cargo_patch_args[@]}"
)

echo "::group::Create every publishable workspace source archive"
publishable_count=0
while IFS=$'\t' read -r package_name _package_path package_release_version; do
    publishable_count=$((publishable_count + 1))
    if [[ "$package_name" != "dear-imgui-build-support" && "$package_name" != "dear-imgui-sys" ]]; then
        echo "Packaging source archive: $package_name"
        (
            cd "$package_workspace"
            CARGO_TARGET_DIR="$target_dir" cargo package \
                -p "$package_name" \
                --quiet \
                --offline \
                --locked \
                "${cargo_patch_args[@]}"
        )
    fi
    test -f "$target_dir/package/$package_name-$package_release_version.crate"
done < "$publish_order_file"
echo "::endgroup::"

python3 - "$target_dir/package" "$publish_order_file" <<'PY'
import sys
import tarfile
from pathlib import Path

archive_dir = Path(sys.argv[1])
publish_order_file = Path(sys.argv[2])
packages = [line.rstrip("\n").split("\t") for line in publish_order_file.read_text().splitlines()]

sys_sentinels = {
    "dear-imgui-sys": (
        "src/platform_io_hooks.cpp",
        "third-party/cimgui/cimgui.cpp",
        "third-party/cimgui/imgui/imgui.cpp",
    ),
    "dear-implot-sys": (
        "third-party/cimplot/cimplot.cpp",
        "third-party/cimplot/implot/implot.cpp",
    ),
    "dear-imnodes-sys": (
        "shim/imnodes_extra.cpp",
        "third-party/cimnodes/cimnodes.cpp",
        "third-party/cimnodes/imnodes/imnodes.cpp",
    ),
    "dear-node-editor-sys": (
        "shim/node_editor_extra.cpp",
        "third-party/cimnodes_editor/cimnodes_editor.cpp",
        "third-party/cimnodes_editor/imgui-node-editor/imgui_node_editor.cpp",
    ),
    "dear-imguizmo-sys": (
        "third-party/cimguizmo/cimguizmo.cpp",
        "third-party/cimguizmo/ImGuizmo/src/ImGuizmo.cpp",
    ),
    "dear-implot3d-sys": (
        "third-party/cimplot3d/cimplot3d.cpp",
        "third-party/cimplot3d/implot3d/implot3d.cpp",
    ),
    "dear-imguizmo-quat-sys": (
        "third-party/cimguizmo_quat/cimguizmo_quat.cpp",
        "third-party/cimguizmo_quat/imGuIZMO.quat/imguizmo_quat/imguizmo_quat.cpp",
    ),
    "dear-imgui-test-engine-sys": (
        "shim/cimgui_test_engine.cpp",
        "third-party/imgui_test_engine/imgui_test_engine/imgui_te_engine.cpp",
    ),
}
expected_sys_packages = {name for name, _path, _version in packages if name.endswith("-sys")}
if set(sys_sentinels) != expected_sys_packages:
    raise SystemExit(
        "native source sentinel map differs from publishable sys crates: "
        f"expected {sorted(expected_sys_packages)}, found {sorted(sys_sentinels)}"
    )

for name, _path, version in packages:
    archive = archive_dir / f"{name}-{version}.crate"
    if not archive.is_file():
        raise SystemExit(f"missing source archive for {name}: {archive}")
    root = f"{name}-{version}"
    with tarfile.open(archive, "r:gz") as package:
        members = {member.name.lstrip("./") for member in package.getmembers()}
    if any(".git" in Path(member).parts for member in members):
        raise SystemExit(f"{archive} contains a .git entry")
    for sentinel in sys_sentinels.get(name, ()):
        required = f"{root}/{sentinel}"
        if required not in members:
            raise SystemExit(f"{archive} is missing native source sentinel {required}")

print(
    f"Verified {len(packages)} source archives and native source sentinels for "
    f"{len(sys_sentinels)} sys crates."
)
PY

native_package_dir="$work_dir/native-packages"
mkdir -p "$native_package_dir"

echo "::group::Build normal Dear ImGui host prebuilt"
(
    cd "$package_workspace"
    IMGUI_SYS_FORCE_BUILD=1 \
    IMGUI_SYS_PACKAGE_DIR="$native_package_dir" \
    CARGO_TARGET_DIR="$target_dir/native-normal" \
        cargo run -p dear-imgui-sys --release --features package-bin --bin package
)
echo "::endgroup::"

echo "::group::Build stack-layout Dear ImGui host prebuilt"
(
    cd "$package_workspace"
    IMGUI_SYS_FORCE_BUILD=1 \
    IMGUI_SYS_PACKAGE_DIR="$native_package_dir" \
    IMGUI_SYS_PKG_FEATURES=stack-layout \
    CARGO_TARGET_DIR="$target_dir/native-stack-layout" \
        cargo run \
            -p dear-imgui-sys \
            --release \
            --no-default-features \
            --features package-bin,stack-layout \
            --bin package
)
echo "::endgroup::"

verify_core_prebuilt_packages \
    "$native_package_dir" \
    "$(host_target)" \
    "" \
    "$package_workspace"

echo "Verified packaged core consumers and $publishable_count publishable source archives."
