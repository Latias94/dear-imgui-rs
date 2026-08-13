"""Build and consume exact Dear ImGui native prebuilt profiles."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

from _archive import read_unique_root_file, safe_extract_tar
from _process import environment, github_group, run
from _verification import VerificationError, temporary_workspace


WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
PLATFORM_IO_AGGREGATE_HOOKS_FEATURE = "platform-io-aggregate-hooks-v3"
SAFE_DEMO_FONT_BOUNDARY_FEATURE = "safe-demo-font-boundary-v1"


@dataclass(frozen=True)
class PrebuiltProfile:
    """One exact artifact feature set and its consumer-facing configuration."""

    name: str
    artifact_features: frozenset[str]
    consumer_features: tuple[str, ...]
    included_in_base: bool


@dataclass(frozen=True)
class ExtensionSpec:
    """One safe extension and its exact matching native sys artifact."""

    extension_id: str
    safe_crate: str
    sys_crate: str
    archive_stem: str
    library_name: str
    env_stem: str
    profiles: tuple[str, ...]
    symbol: str


PREBUILT_PROFILES = (
    PrebuiltProfile(
        "normal",
        frozenset(
            (
                PLATFORM_IO_AGGREGATE_HOOKS_FEATURE,
                SAFE_DEMO_FONT_BOUNDARY_FEATURE,
                "wchar32",
            )
        ),
        (),
        True,
    ),
    PrebuiltProfile(
        "stack-layout",
        frozenset(
            (
                PLATFORM_IO_AGGREGATE_HOOKS_FEATURE,
                SAFE_DEMO_FONT_BOUNDARY_FEATURE,
                "stack-layout",
                "wchar32",
            )
        ),
        ("stack-layout",),
        True,
    ),
    PrebuiltProfile(
        "freetype",
        frozenset(
            (
                PLATFORM_IO_AGGREGATE_HOOKS_FEATURE,
                SAFE_DEMO_FONT_BOUNDARY_FEATURE,
                "freetype",
                "wchar32",
            )
        ),
        ("freetype",),
        False,
    ),
    PrebuiltProfile(
        "stack-layout-freetype",
        frozenset(
            (
                PLATFORM_IO_AGGREGATE_HOOKS_FEATURE,
                SAFE_DEMO_FONT_BOUNDARY_FEATURE,
                "stack-layout",
                "freetype",
                "wchar32",
            )
        ),
        ("stack-layout", "freetype"),
        False,
    ),
)
PREBUILT_PROFILE_BY_NAME = {profile.name: profile for profile in PREBUILT_PROFILES}
ARTIFACT_PROFILE_BY_FEATURES = {
    profile.artifact_features: profile for profile in PREBUILT_PROFILES
}
PROFILE_SCOPES = {
    "base": tuple(
        profile.name for profile in PREBUILT_PROFILES if profile.included_in_base
    ),
    "all": tuple(profile.name for profile in PREBUILT_PROFILES),
}
EXTENSION_SPECS = (
    ExtensionSpec(
        "implot",
        "dear-implot",
        "dear-implot-sys",
        "dear-implot",
        "dear_implot",
        "IMPLOT_SYS",
        ("normal", "freetype"),
        "ImPlot_GetPlotPos",
    ),
    ExtensionSpec(
        "implot3d",
        "dear-implot3d",
        "dear-implot3d-sys",
        "dear-implot3d",
        "dear_implot3d",
        "IMPLOT3D_SYS",
        ("normal",),
        "ImPlot3D_GetPlotRectPos",
    ),
    ExtensionSpec(
        "imnodes",
        "dear-imnodes",
        "dear-imnodes-sys",
        "dear-imnodes",
        "dear_imnodes",
        "IMNODES_SYS",
        ("normal", "freetype"),
        "imnodes_EditorContextGetPanning",
    ),
    ExtensionSpec(
        "node-editor",
        "dear-node-editor",
        "dear-node-editor-sys",
        "dear-node-editor",
        "dear_node_editor",
        "NODE_EDITOR_SYS",
        ("normal", "stack-layout", "freetype", "stack-layout-freetype"),
        "dne_create_editor",
    ),
    ExtensionSpec(
        "imguizmo",
        "dear-imguizmo",
        "dear-imguizmo-sys",
        "dear-imguizmo",
        "dear_imguizmo",
        "IMGUIZMO_SYS",
        ("normal", "freetype"),
        "ImGuizmo_BeginFrame",
    ),
    ExtensionSpec(
        "imguizmo-quat",
        "dear-imguizmo-quat",
        "dear-imguizmo-quat-sys",
        "dear-imguizmo-quat",
        "dear_imguizmo_quat",
        "IMGUIZMO_QUAT_SYS",
        ("normal", "freetype"),
        "imguiGizmo_buildPlane",
    ),
)
EXTENSION_BY_ID = {spec.extension_id: spec for spec in EXTENSION_SPECS}
SUPPORTED_WASM_EXTENSION_SPECS = tuple(
    spec for spec in EXTENSION_SPECS if spec.extension_id != "node-editor"
)
PREBUILT_ENV_TO_UNSET = (
    "DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH",
    "DEAR_IMGUI_RS_CANDIDATE_SHA",
    "IMGUI_SYS_FORCE_BUILD",
    "IMGUI_SYS_PREBUILT_URL",
    "IMGUI_SYS_USE_PREBUILT",
)
EXTENSION_PREBUILT_ENV_TO_UNSET = tuple(
    f"{spec.env_stem}_{suffix}"
    for spec in EXTENSION_SPECS
    for suffix in ("FORCE_BUILD", "LIB_DIR", "PREBUILT_URL", "SKIP_CC", "USE_PREBUILT")
)
NATIVE_BUILD_ENV_TO_UNSET = (
    *PREBUILT_ENV_TO_UNSET,
    "IMGUI_SYS_LIB_DIR",
    "IMGUI_SYS_PACKAGE_DIR",
    "IMGUI_SYS_PKG_CRT",
    "IMGUI_SYS_PKG_FEATURES",
    "IMGUI_SYS_SKIP_CC",
    *EXTENSION_PREBUILT_ENV_TO_UNSET,
    *(
        f"{spec.env_stem}_{suffix}"
        for spec in EXTENSION_SPECS
        for suffix in ("PACKAGE_DIR", "PKG_CRT", "PKG_FEATURES")
    ),
)

GIT_SHA_PATTERN = re.compile(r"[0-9a-fA-F]{40}\Z")
STABLE_HASH_PATTERN = re.compile(r"fnv1a64:[0-9a-fA-F]{16}\Z")


class _StableHash:
    """Byte-for-byte implementation of build-support's canonical FNV encoder."""

    def __init__(self) -> None:
        self.value = 0xCBF29CE484222325

    def _bytes(self, value: bytes) -> None:
        for byte in value:
            self.value ^= byte
            self.value = (self.value * 0x00000100000001B3) & 0xFFFFFFFFFFFFFFFF

    def _u64(self, value: int) -> None:
        self._bytes(value.to_bytes(8, "little"))

    def _string(self, value: str) -> None:
        encoded = value.encode("utf-8")
        self._u64(len(encoded))
        self._bytes(encoded)

    def field(self, label: str, value: str) -> None:
        self._bytes(b"field")
        self._string(label)
        self._string(value)

    def fields(self, label: str, values: tuple[str, ...]) -> None:
        self._bytes(b"list")
        self._string(label)
        self._u64(len(values))
        for index, value in enumerate(values):
            self._bytes(b"item")
            self._u64(index)
            self._string(value)

    def finish(self) -> str:
        return f"fnv1a64:{self.value:016x}"


def _read_prebuilt_manifest(archive: Path) -> dict[str, str]:
    try:
        lines = (
            read_unique_root_file(archive, "manifest.txt", mode="r:gz")
            .decode("utf-8")
            .splitlines()
        )
    except UnicodeDecodeError as error:
        raise VerificationError(
            f"could not inspect prebuilt manifest in {archive}: {error}"
        ) from error
    if not lines:
        raise VerificationError(f"prebuilt manifest in {archive} is empty")
    heading = lines[0]
    if not heading.endswith(" prebuilt") or heading == " prebuilt":
        raise VerificationError(
            f"prebuilt manifest in {archive} has invalid heading {heading!r}"
        )
    fields = {"crate_name": heading.removesuffix(" prebuilt")}
    for line in lines[1:]:
        if not line:
            continue
        if "=" not in line:
            raise VerificationError(
                f"prebuilt manifest in {archive} has invalid line {line!r}"
            )
        key, value = line.split("=", 1)
        if not key:
            raise VerificationError(
                f"prebuilt manifest in {archive} has an empty field name"
            )
        if key in fields:
            raise VerificationError(
                f"prebuilt manifest in {archive} repeats field {key}"
            )
        fields[key] = value
    return fields


def _require_exact_fields(
    fields: dict[str, str], expected: frozenset[str], archive: Path
) -> None:
    actual = frozenset(fields)
    if actual != expected:
        missing = ",".join(sorted(expected - actual)) or "none"
        unknown = ",".join(sorted(actual - expected)) or "none"
        raise VerificationError(
            f"prebuilt manifest in {archive} field mismatch: "
            f"missing={missing}; unknown={unknown}"
        )


def _canonical_features(fields: dict[str, str], archive: Path) -> tuple[str, ...]:
    raw = fields.get("features", "")
    features = tuple(filter(None, raw.split(",")))
    canonical = tuple(sorted(set(features)))
    if features != canonical:
        raise VerificationError(
            f"prebuilt manifest in {archive} has non-canonical features={raw!r}"
        )
    return features


CORE_MANIFEST_FIELDS = frozenset(
    (
        "crate_name",
        "version",
        "candidate_sha",
        "target",
        "link",
        "crt",
        "features",
        "cimgui_revision",
        "imgui_revision",
        "binding_spec_hash",
        "source_contract_hash",
    )
)


def core_artifact_profile_hash(fields: dict[str, str], archive: Path) -> str:
    """Validate and hash one complete canonical core ArtifactProfile manifest."""
    _require_exact_fields(fields, CORE_MANIFEST_FIELDS, archive)
    if fields["crate_name"] != "dear-imgui":
        raise VerificationError(
            f"foreign core artifact in {archive}: crate={fields['crate_name']!r}"
        )
    features = _canonical_features(fields, archive)
    if "platform-io-aggregate-hooks-v2" in features:
        raise VerificationError(
            f"prebuilt {archive} declares obsolete "
            "platform-io-aggregate-hooks-v2; regenerate the archive with "
            f"{PLATFORM_IO_AGGREGATE_HOOKS_FEATURE} or use a source build"
        )
    _validate_candidate_sha(fields["candidate_sha"])
    for revision in ("cimgui_revision", "imgui_revision"):
        if not GIT_SHA_PATTERN.fullmatch(fields[revision]):
            raise VerificationError(
                f"prebuilt manifest in {archive} has invalid {revision}"
            )
    if not STABLE_HASH_PATTERN.fullmatch(fields["binding_spec_hash"]):
        raise VerificationError(
            f"prebuilt manifest in {archive} has invalid binding_spec_hash"
        )
    if not STABLE_HASH_PATTERN.fullmatch(fields["source_contract_hash"]):
        raise VerificationError(
            f"prebuilt manifest in {archive} has invalid source_contract_hash"
        )

    identity = _StableHash()
    identity.field("schema", "core-artifact-profile-v3")
    for label, field in (
        ("crate_name", "crate_name"),
        ("version", "version"),
        ("target", "target"),
        ("link_type", "link"),
        ("crt", "crt"),
    ):
        identity.field(label, fields[field])
    identity.fields("features", features)
    identity.field("cimgui_revision", fields["cimgui_revision"])
    identity.field("imgui_revision", fields["imgui_revision"])
    identity.field("binding_spec_hash", fields["binding_spec_hash"])
    identity.field("source_contract_hash", fields["source_contract_hash"])
    return identity.finish()


def _validate_candidate_sha(candidate_sha: str) -> str:
    if not GIT_SHA_PATTERN.fullmatch(candidate_sha):
        raise VerificationError(
            "release candidate SHA must contain exactly 40 hexadecimal characters"
        )
    if candidate_sha != candidate_sha.lower():
        raise VerificationError("release candidate SHA must use lowercase hexadecimal")
    return candidate_sha


def core_artifact_identity(
    fields: dict[str, str],
    archive: Path,
    expected_candidate_sha: str | None = None,
) -> str:
    """Validate and hash one release-specific core artifact identity."""
    profile_hash = core_artifact_profile_hash(fields, archive)
    candidate_sha = _validate_candidate_sha(fields["candidate_sha"])
    if expected_candidate_sha is not None:
        expected_candidate_sha = _validate_candidate_sha(expected_candidate_sha)
        if candidate_sha != expected_candidate_sha:
            raise VerificationError(
                f"core artifact candidate_sha mismatch in {archive}: "
                f"expected {expected_candidate_sha!r}, found {candidate_sha!r}"
            )
    identity = _StableHash()
    identity.field("schema", "core-artifact-identity-v1")
    identity.field("profile", profile_hash)
    identity.field("candidate_sha", candidate_sha)
    return identity.finish()


def expected_extension_binding_identity(source_root: Path, spec: ExtensionSpec) -> str:
    binding = source_root / "extensions" / spec.sys_crate / "src" / "bindings_pregenerated.rs"
    try:
        marker = binding.read_text(encoding="utf-8").splitlines()[0]
    except (OSError, IndexError) as error:
        raise VerificationError(
            f"could not read native binding provenance for {spec.sys_crate}: {error}"
        ) from error
    prefix = "// dear-imgui-rs-binding-provenance-v1 "
    if not marker.startswith(prefix):
        raise VerificationError(
            f"native binding for {spec.sys_crate} has invalid provenance prefix"
        )
    values: dict[str, str] = {}
    for item in marker.removeprefix(prefix).split():
        if "=" not in item:
            raise VerificationError(
                f"native binding for {spec.sys_crate} has malformed provenance"
            )
        key, value = item.split("=", 1)
        if key in values:
            raise VerificationError(
                f"native binding for {spec.sys_crate} repeats provenance field {key}"
            )
        values[key] = value
    expected_fields = ("crate", "target", "source", "spec", "inputs", "output")
    if tuple(values) != expected_fields:
        raise VerificationError(
            f"native binding for {spec.sys_crate} has non-canonical provenance fields"
        )
    if values["crate"] != spec.sys_crate or values["target"] != "native":
        raise VerificationError(
            f"native binding provenance does not belong to {spec.sys_crate}"
        )
    if not GIT_SHA_PATTERN.fullmatch(values["source"]):
        raise VerificationError(
            f"native binding for {spec.sys_crate} has invalid source revision"
        )
    for field in ("spec", "inputs", "output"):
        if not STABLE_HASH_PATTERN.fullmatch(values[field]):
            raise VerificationError(
                f"native binding for {spec.sys_crate} has invalid {field} hash"
            )

    provenance = _StableHash()
    provenance.field("schema", "crate-binding-identity-v1")
    for label, field in (
        ("crate_name", "crate"),
        ("target", "target"),
        ("source_revision", "source"),
        ("spec_hash", "spec"),
        ("input_hash", "inputs"),
        ("output_hash", "output"),
    ):
        provenance.field(label, values[field])
    identity = _StableHash()
    identity.field("schema", "extension-binding-identity-v1")
    identity.field("extension", spec.extension_id)
    identity.field("provenance", provenance.finish())
    return identity.finish()


def _required_profiles(profile_scope: str) -> tuple[str, ...]:
    try:
        return PROFILE_SCOPES[profile_scope]
    except KeyError as error:
        raise VerificationError(
            f"unsupported prebuilt profile scope: {profile_scope}"
        ) from error


def select_core_prebuilt_archives(
    package_dir: Path,
    target: str,
    crt: str,
    *,
    profile_scope: str,
    candidate_sha: str | None = None,
) -> dict[str, Path]:
    """Select exactly one compatible archive for every profile in a scope."""
    required_profiles = _required_profiles(profile_scope)
    if candidate_sha is not None:
        candidate_sha = _validate_candidate_sha(candidate_sha)
    matches = {profile: [] for profile in required_profiles}
    for archive in sorted(package_dir.resolve().glob("dear-imgui-*.tar.gz")):
        fields = _read_prebuilt_manifest(archive)
        core_artifact_identity(fields, archive)
        if fields.get("target") != target:
            continue
        if fields.get("crt") != crt:
            continue
        if candidate_sha is not None and fields["candidate_sha"] != candidate_sha:
            continue
        if fields["link"] != "static":
            raise VerificationError(
                f"unsupported dear_imgui link type in {archive}: {fields['link']!r}"
            )
        features = frozenset(_canonical_features(fields, archive))
        profile = ARTIFACT_PROFILE_BY_FEATURES.get(features)
        if profile is None:
            rendered_features = ",".join(sorted(features)) or "<none>"
            raise VerificationError(
                f"unsupported dear_imgui artifact profile in {archive}: "
                f"features={rendered_features}"
            )
        if profile.name in matches:
            matches[profile.name].append(archive)

    selected = {}
    for profile, archives in matches.items():
        if len(archives) != 1:
            rendered = ", ".join(str(path) for path in archives) or "none"
            raise VerificationError(
                f"expected exactly one {profile} archive for target={target!r} "
                f"crt={crt!r}, found {rendered}"
            )
        selected[profile] = archives[0]
    return selected


EXTENSION_MANIFEST_FIELDS = frozenset(
    (
        "crate_name",
        "version",
        "candidate_sha",
        "target",
        "link",
        "crt",
        "features",
        "extension",
        "safe_crate",
        "library",
        "archive",
        "core_artifact_identity",
        "extension_binding_identity",
    )
)


def extension_artifact_features(
    spec: ExtensionSpec, profile_name: str
) -> tuple[str, ...]:
    if profile_name not in spec.profiles:
        raise VerificationError(
            f"{spec.safe_crate} does not support prebuilt profile {profile_name}"
        )
    features = ["wchar32"]
    if "freetype" in profile_name:
        features.append("freetype")
    if "stack-layout" in profile_name:
        features.append("stack-layout")
    return tuple(sorted(features))


def extension_consumer_features(
    spec: ExtensionSpec, profile_name: str
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    artifact_features = extension_artifact_features(spec, profile_name)
    sys_features = tuple(
        feature for feature in ("freetype", "stack-layout") if feature in artifact_features
    )
    safe_features = []
    if "freetype" in artifact_features:
        safe_features.append("freetype")
    if "stack-layout" in artifact_features:
        if spec.extension_id != "node-editor":
            raise VerificationError(
                f"{spec.safe_crate} has no safe stack-layout feature route"
            )
        safe_features.append("blueprints")
    return tuple(safe_features), sys_features


def _expected_extension_archive_name(
    spec: ExtensionSpec,
    version: str,
    target: str,
    crt: str,
    features: tuple[str, ...],
) -> str:
    suffix_features = tuple(
        feature
        for feature in ("stack-layout", "freetype")
        if feature in features
    )
    suffix = f"-{'-'.join(suffix_features)}" if suffix_features else ""
    crt_suffix = f"-{crt}" if crt else ""
    return (
        f"{spec.archive_stem}-prebuilt-{version}-{target}-static"
        f"{suffix}{crt_suffix}.tar.gz"
    )


def select_extension_prebuilt_archives(
    package_dir: Path,
    target: str,
    crt: str,
    candidate_sha: str,
    source_root: Path,
    core_archives: dict[str, Path],
    *,
    profile_scope: str,
) -> dict[tuple[str, str], Path]:
    """Select one extension archive matching each exact selected core profile."""
    candidate_sha = _validate_candidate_sha(candidate_sha)
    required_profiles = _required_profiles(profile_scope)
    core_manifests = {
        profile: _read_prebuilt_manifest(archive)
        for profile, archive in core_archives.items()
    }
    core_identities = {
        profile: core_artifact_identity(
            fields, core_archives[profile], candidate_sha
        )
        for profile, fields in core_manifests.items()
    }
    matches: dict[tuple[str, str], list[Path]] = {
        (spec.extension_id, profile): []
        for spec in EXTENSION_SPECS
        for profile in required_profiles
        if profile in spec.profiles
    }

    for spec in EXTENSION_SPECS:
        binding_identity = expected_extension_binding_identity(source_root, spec)
        profiles_by_features = {
            extension_artifact_features(spec, profile): profile
            for profile in spec.profiles
        }
        pattern = f"{spec.archive_stem}-prebuilt-*.tar.gz"
        for archive in sorted(package_dir.resolve().glob(pattern)):
            fields = _read_prebuilt_manifest(archive)
            _require_exact_fields(fields, EXTENSION_MANIFEST_FIELDS, archive)
            if fields["target"] != target or fields["crt"] != crt:
                continue
            if _validate_candidate_sha(fields["candidate_sha"]) != candidate_sha:
                continue
            features = _canonical_features(fields, archive)
            profile = profiles_by_features.get(features)
            if profile is None:
                rendered = ",".join(features) or "<none>"
                raise VerificationError(
                    f"unsupported {spec.sys_crate} artifact profile in {archive}: "
                    f"features={rendered}"
                )
            key = (spec.extension_id, profile)
            if key not in matches:
                continue
            expected_name = _expected_extension_archive_name(
                spec, fields["version"], fields["target"], fields["crt"], features
            )
            expected_identity = {
                "crate_name": spec.sys_crate,
                "extension": spec.extension_id,
                "safe_crate": spec.safe_crate,
                "library": spec.library_name,
                "candidate_sha": candidate_sha,
                "extension_binding_identity": binding_identity,
                "link": "static",
            }
            for field, expected in expected_identity.items():
                if fields[field] != expected:
                    raise VerificationError(
                        f"extension artifact {field} mismatch in {archive}: "
                        f"expected {expected!r}, found {fields[field]!r}"
                    )
            if archive.name != expected_name or fields["archive"] != expected_name:
                raise VerificationError(
                    f"extension archive identity mismatch: expected {expected_name}, "
                    f"found file={archive.name} manifest={fields['archive']}"
                )
            core_manifest = core_manifests[profile]
            if fields["version"] != core_manifest["version"]:
                raise VerificationError(
                    f"extension artifact version mismatch in {archive}: expected "
                    f"{core_manifest['version']!r}, found {fields['version']!r}"
                )
            if fields["core_artifact_identity"] != core_identities[profile]:
                raise VerificationError(
                    f"extension artifact core_artifact_identity mismatch in {archive}"
                )
            matches[key].append(archive)

    selected: dict[tuple[str, str], Path] = {}
    for key, archives in matches.items():
        if len(archives) != 1:
            extension, profile = key
            rendered = ", ".join(str(path) for path in archives) or "none"
            raise VerificationError(
                f"expected exactly one {extension} {profile} archive for "
                f"target={target!r} crt={crt!r}, found {rendered}"
            )
        selected[key] = archives[0]
    return selected


def write_prebuilt_consumer(destination: Path, source_root: Path, profile: str) -> None:
    """Create a locked Rust consumer for one native artifact profile."""
    try:
        consumer_features = PREBUILT_PROFILE_BY_NAME[profile].consumer_features
    except KeyError as error:
        raise VerificationError(
            f"unsupported prebuilt consumer profile: {profile}"
        ) from error

    source_dir = destination / "src"
    source_dir.mkdir(parents=True, exist_ok=True)
    selected_features = ["prebuilt", *consumer_features]
    dependency_path = (source_root / "dear-imgui").resolve()
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
                    f"{json.dumps(os.fspath(dependency_path))}, default-features = false, "
                    f"features = {json.dumps(selected_features)} }}"
                ),
                "",
                "[workspace]",
                "",
            )
        ),
        encoding="utf-8",
    )
    if "stack-layout" in consumer_features:
        frame_body = """        let layout = ui.begin_horizontal("artifact-row", [0.0, 0.0], -1.0);
        ui.text("stack-layout artifact");
        layout.end();
"""
    else:
        frame_body = '        ui.text("normal artifact");\n'
    source_dir.joinpath("main.rs").write_text(
        f"""fn verify_platform_io_aggregate_v3_symbols() {{
    use dear_imgui_rs::sys;

    let input = sys::ImVec2::new(3.0, 4.0);
    let mut output = sys::ImVec2::new(-1.0, -1.0);
    unsafe {{
        assert!(!sys::ImGuiPlatformIO_InvokePlatformSetWindowPos(
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::from_ref(&input),
        ));
        assert!(!sys::ImGuiPlatformIO_InvokePlatformSetWindowSize(
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::from_ref(&input),
        ));
        assert!(!sys::ImGuiPlatformIO_InvokePlatformGetWindowPos(
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::from_mut(&mut output),
        ));
        assert_eq!((output.x, output.y), (0.0, 0.0));
        output = sys::ImVec2::new(-1.0, -1.0);
        assert!(!sys::ImGuiPlatformIO_InvokePlatformGetWindowSize(
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::from_mut(&mut output),
        ));
        assert_eq!((output.x, output.y), (0.0, 0.0));
        output = sys::ImVec2::new(-1.0, -1.0);
        assert!(!sys::ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::from_mut(&mut output),
        ));
        assert_eq!((output.x, output.y), (0.0, 0.0));
        assert!(!sys::ImGuiPlatformIO_InvokeRendererSetWindowSize(
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::from_ref(&input),
        ));
        assert!(sys::ImGuiPlatformIO_PlatformSetWindowPosPointerParam(
            std::ptr::null_mut(),
        )
        .is_none());
        assert!(sys::ImGuiPlatformIO_PlatformSetWindowSizePointerParam(
            std::ptr::null_mut(),
        )
        .is_none());
        assert!(sys::ImGuiPlatformIO_PlatformGetWindowPosOutParam(
            std::ptr::null_mut(),
        )
        .is_none());
        assert!(sys::ImGuiPlatformIO_PlatformGetWindowSizeOutParam(
            std::ptr::null_mut(),
        )
        .is_none());
        assert!(sys::ImGuiPlatformIO_PlatformGetWindowFramebufferScaleOutParam(
            std::ptr::null_mut(),
        )
        .is_none());
    }}
}}

fn main() {{
    verify_platform_io_aggregate_v3_symbols();
    let mut context = dear_imgui_rs::Context::create();
    context.io_mut().set_display_size([320.0, 240.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    {{
        let mut atlas = context
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("the standalone artifact consumer owns legacy atlas rendering");
        let _ = atlas.build();
    }}
    {{
        let ui = context.frame();
{frame_body}    }}
    assert!(context.render_legacy().valid());
    assert!(!dear_imgui_rs::dear_imgui_version().is_empty());
}}
""",
        encoding="utf-8",
    )
    _copy_consumer_lockfile(destination, source_root)


def write_extension_prebuilt_consumer(
    destination: Path,
    source_root: Path,
    spec: ExtensionSpec,
    profile: str,
) -> None:
    """Create a consumer of the safe extension and its exact matching sys crate."""
    safe_features, _ = extension_consumer_features(spec, profile)
    source_dir = destination / "src"
    source_dir.mkdir(parents=True, exist_ok=True)
    safe_path = (source_root / "extensions" / spec.safe_crate).resolve()
    sys_path = (source_root / "extensions" / spec.sys_crate).resolve()
    safe_selected_features = ["prebuilt", *safe_features]
    destination.joinpath("Cargo.toml").write_text(
        "\n".join(
            (
                "[package]",
                f'name = "dear-imgui-prebuilt-{spec.extension_id}-{profile}"',
                'version = "0.0.0"',
                'edition = "2024"',
                "publish = false",
                "",
                "[dependencies]",
                (
                    f"{spec.safe_crate} = {{ path = {json.dumps(os.fspath(safe_path))}, "
                    "default-features = false, "
                    f"features = {json.dumps(safe_selected_features)} }}"
                ),
                (
                    f"{spec.sys_crate} = {{ path = {json.dumps(os.fspath(sys_path))}, "
                    "default-features = false }"
                ),
                "",
                "[workspace]",
                "",
            )
        ),
        encoding="utf-8",
    )
    safe_module = spec.safe_crate.replace("-", "_")
    sys_module = spec.sys_crate.replace("-", "_")
    source_dir.joinpath("main.rs").write_text(
        f"""use {safe_module} as _;

fn main() {{
    let _extension_symbol = {sys_module}::{spec.symbol};
    assert!(!"{spec.safe_crate}".is_empty());
}}
""",
        encoding="utf-8",
    )
    # The direct matching sys dependency is intentional; it proves that Cargo selected and
    # linked the exact artifact rather than only compiling the high-level crate transitively.
    _copy_consumer_lockfile(destination, source_root)


def _copy_consumer_lockfile(destination: Path, source_root: Path) -> None:
    try:
        shutil.copy2(source_root / "Cargo.lock", destination / "Cargo.lock")
    except OSError as error:
        raise VerificationError(f"could not copy Cargo.lock: {error}") from error


def write_extension_route_consumer(
    destination: Path,
    source_root: Path,
    route: str,
) -> tuple[ExtensionSpec, ...]:
    """Create one locked consumer that exercises every extension on a build route."""
    route_features = {
        "source": ("build-from-source",),
        "source-plus-prebuilt": ("build-from-source", "prebuilt"),
        "wasm": ("wasm",),
        "wasm-plus-prebuilt": ("wasm", "prebuilt"),
    }
    try:
        selected_features = route_features[route]
    except KeyError as error:
        raise VerificationError(f"unsupported extension consumer route: {route}") from error
    specs = (
        SUPPORTED_WASM_EXTENSION_SPECS
        if route.startswith("wasm")
        else EXTENSION_SPECS
    )
    source_dir = destination / "src"
    source_dir.mkdir(parents=True, exist_ok=True)
    dependencies = []
    statements = []
    for spec in specs:
        safe_path = (source_root / "extensions" / spec.safe_crate).resolve()
        sys_path = (source_root / "extensions" / spec.sys_crate).resolve()
        dependencies.extend(
            (
                (
                    f"{spec.safe_crate} = {{ path = {json.dumps(os.fspath(safe_path))}, "
                    "default-features = false, "
                    f"features = {json.dumps(selected_features)} }}"
                ),
                (
                    f"{spec.sys_crate} = {{ path = {json.dumps(os.fspath(sys_path))}, "
                    "default-features = false }"
                ),
            )
        )
        statements.extend(
            (
                f"    use {spec.safe_crate.replace('-', '_')} as _;",
                (
                    "    std::hint::black_box("
                    f"{spec.sys_crate.replace('-', '_')}::{spec.symbol} as *const ());"
                ),
            )
        )
    destination.joinpath("Cargo.toml").write_text(
        "\n".join(
            (
                "[package]",
                f'name = "dear-imgui-extension-route-{route}"',
                'version = "0.0.0"',
                'edition = "2024"',
                "publish = false",
                "",
                "[dependencies]",
                *dependencies,
                "",
                "[workspace]",
                "",
            )
        ),
        encoding="utf-8",
    )
    source_dir.joinpath("main.rs").write_text(
        "\n".join(("fn main() {", *statements, "}", "")),
        encoding="utf-8",
    )
    _copy_consumer_lockfile(destination, source_root)
    return specs


def write_stale_extension_prebuilts(destination: Path) -> None:
    """Create link-visible but intentionally invalid artifacts for precedence tests."""
    artifacts = (("core", "dear_imgui"),) + tuple(
        (spec.extension_id, spec.library_name) for spec in EXTENSION_SPECS
    )
    for artifact, library in artifacts:
        lib_dir = destination / artifact / "lib"
        lib_dir.mkdir(parents=True, exist_ok=True)
        lib_dir.joinpath(f"{library}.lib").touch()
        lib_dir.joinpath(f"lib{library}.a").touch()
        lib_dir.joinpath("manifest.txt").write_text(
            "stale artifact that must never be selected\n",
            encoding="utf-8",
        )


def extension_source_consumer_environment(
    target_dir: Path,
    stale_prebuilt_root: Path | None = None,
) -> dict[str, str]:
    """Create a clean source-route environment, optionally with stale prebuilts present."""
    values: dict[str, str | Path] = {"CARGO_TARGET_DIR": target_dir}
    if stale_prebuilt_root is not None:
        values["IMGUI_SYS_LIB_DIR"] = stale_prebuilt_root / "core" / "lib"
        for spec in EXTENSION_SPECS:
            values[f"{spec.env_stem}_LIB_DIR"] = (
                stale_prebuilt_root / spec.extension_id / "lib"
            )
    return environment(values, unset=NATIVE_BUILD_ENV_TO_UNSET)


def prebuilt_consumer_environment(
    artifact_root: Path, target_dir: Path
) -> dict[str, str]:
    """Build an isolated environment for consuming one prebuilt artifact."""
    return environment(
        {
            "IMGUI_SYS_SKIP_CC": "1",
            "IMGUI_SYS_LIB_DIR": artifact_root / "lib",
            "CARGO_TARGET_DIR": target_dir,
        },
        unset=PREBUILT_ENV_TO_UNSET,
    )


def extension_prebuilt_consumer_environment(
    core_artifact_root: Path,
    extension_artifact_root: Path,
    spec: ExtensionSpec,
    target_dir: Path,
) -> dict[str, str]:
    """Build an isolated exact core-plus-extension prebuilt environment."""
    return environment(
        {
            "IMGUI_SYS_SKIP_CC": "1",
            "IMGUI_SYS_LIB_DIR": core_artifact_root / "lib",
            f"{spec.env_stem}_SKIP_CC": "1",
            f"{spec.env_stem}_LIB_DIR": extension_artifact_root / "lib",
            "CARGO_TARGET_DIR": target_dir,
        },
        unset=(*PREBUILT_ENV_TO_UNSET, *EXTENSION_PREBUILT_ENV_TO_UNSET),
    )


def run_prebuilt_consumer(
    label: str,
    consumer_dir: Path,
    artifact_root: Path,
    target: str,
    target_dir: Path,
) -> None:
    """Resolve online, then build and execute a prebuilt consumer offline."""
    manifest = consumer_dir / "Cargo.toml"
    with github_group(f"Run {label} prebuilt consumer"):
        run(
            (
                "cargo",
                "metadata",
                "--quiet",
                "--manifest-path",
                manifest,
                "--format-version",
                "1",
            ),
            env=environment({"CARGO_TARGET_DIR": target_dir}),
            quiet_stdout=True,
        )
        run(
            (
                "cargo",
                "fetch",
                "--manifest-path",
                manifest,
                "--target",
                target,
                "--locked",
            ),
            env=environment({"CARGO_TARGET_DIR": target_dir}),
        )
        run(
            (
                "cargo",
                "run",
                "--manifest-path",
                manifest,
                "--target",
                target,
                "--locked",
                "--offline",
            ),
            env=prebuilt_consumer_environment(artifact_root, target_dir),
        )


def run_extension_prebuilt_consumer(
    label: str,
    consumer_dir: Path,
    core_artifact_root: Path,
    extension_artifact_root: Path,
    spec: ExtensionSpec,
    target: str,
    target_dir: Path,
) -> None:
    """Resolve online, then run a core-plus-extension consumer offline."""
    manifest = consumer_dir / "Cargo.toml"
    with github_group(f"Run {label} extension prebuilt consumer"):
        run(
            (
                "cargo",
                "metadata",
                "--quiet",
                "--manifest-path",
                manifest,
                "--format-version",
                "1",
            ),
            env=environment({"CARGO_TARGET_DIR": target_dir}),
            quiet_stdout=True,
        )
        run(
            (
                "cargo",
                "fetch",
                "--manifest-path",
                manifest,
                "--target",
                target,
                "--locked",
            ),
            env=environment({"CARGO_TARGET_DIR": target_dir}),
        )
        run(
            (
                "cargo",
                "run",
                "--manifest-path",
                manifest,
                "--target",
                target,
                "--locked",
                "--offline",
            ),
            env=extension_prebuilt_consumer_environment(
                core_artifact_root,
                extension_artifact_root,
                spec,
                target_dir,
            ),
        )


def _prepare_locked_consumer(
    manifest: Path,
    target: str,
    target_dir: Path,
) -> None:
    """Resolve a generated consumer once before enforcing locked offline checks."""
    run(
        (
            "cargo",
            "metadata",
            "--quiet",
            "--manifest-path",
            manifest,
            "--format-version",
            "1",
        ),
        env=environment({"CARGO_TARGET_DIR": target_dir}),
        quiet_stdout=True,
    )
    run(
        (
            "cargo",
            "fetch",
            "--manifest-path",
            manifest,
            "--target",
            target,
            "--locked",
        ),
        env=environment({"CARGO_TARGET_DIR": target_dir}),
    )


def run_extension_route_consumer(
    label: str,
    consumer_dir: Path,
    target: str,
    target_dir: Path,
    *,
    stale_prebuilt_root: Path | None = None,
    execute: bool,
) -> None:
    """Resolve and compile one locked all-extension route consumer."""
    manifest = consumer_dir / "Cargo.toml"
    with github_group(f"Run {label} extension route consumer"):
        _prepare_locked_consumer(manifest, target, target_dir)
        run(
            (
                "cargo",
                "run" if execute else "check",
                "--manifest-path",
                manifest,
                "--target",
                target,
                "--locked",
                "--offline",
            ),
            env=extension_source_consumer_environment(
                target_dir, stale_prebuilt_root
            ),
        )


def verify_extension_feature_routes(
    source_root: Path,
    native_target: str,
    *,
    wasm_target: str | None = None,
) -> None:
    """Execute native source precedence and optional supported/invalid WASM routes."""
    with temporary_workspace("dear-imgui-extension-routes.") as work_dir:
        stale_root = work_dir / "stale-prebuilts"
        write_stale_extension_prebuilts(stale_root)
        for route, stale in (
            ("source", None),
            ("source-plus-prebuilt", stale_root),
        ):
            consumer = work_dir / "consumers" / route
            write_extension_route_consumer(consumer, source_root, route)
            run_extension_route_consumer(
                route,
                consumer,
                native_target,
                work_dir / "targets" / route,
                stale_prebuilt_root=stale,
                execute=True,
            )

        if wasm_target is not None:
            consumer = work_dir / "consumers" / "wasm"
            write_extension_route_consumer(consumer, source_root, "wasm")
            run_extension_route_consumer(
                "wasm",
                consumer,
                wasm_target,
                work_dir / "targets" / "wasm",
                execute=False,
            )
            invalid = work_dir / "consumers" / "wasm-plus-prebuilt"
            write_extension_route_consumer(
                invalid, source_root, "wasm-plus-prebuilt"
            )
            manifest = invalid / "Cargo.toml"
            invalid_target_dir = work_dir / "targets" / "wasm-plus-prebuilt"
            _prepare_locked_consumer(manifest, wasm_target, invalid_target_dir)
            result = run(
                (
                    "cargo",
                    "check",
                    "--manifest-path",
                    manifest,
                    "--target",
                    wasm_target,
                    "--locked",
                    "--offline",
                ),
                env=extension_source_consumer_environment(
                    invalid_target_dir
                ),
                capture_output=True,
                combine_output=True,
                accepted_returncodes=None,
            )
            expected = (
                "feature `prebuilt` is native-only and cannot be combined with WASM"
            )
            if result.returncode == 0 or expected not in (result.stdout or ""):
                raise VerificationError(
                    "wasm-plus-prebuilt extension route did not fail with its stable diagnostic"
                )


def verify_profile_mismatch_result(
    label: str, result: subprocess.CompletedProcess[str]
) -> None:
    """Require a mismatch check to fail for the strict artifact diagnostic."""
    if result.returncode == 0:
        raise VerificationError(f"{label} profile mismatch unexpectedly succeeded")
    if "selected an incompatible dear_imgui artifact" not in (result.stdout or ""):
        raise VerificationError(
            f"{label} failed without the strict artifact profile diagnostic"
        )


def reject_prebuilt_profile_mismatch(
    label: str,
    consumer_dir: Path,
    artifact_root: Path,
    target: str,
    target_dir: Path,
) -> None:
    """Prove that an artifact cannot satisfy an incompatible consumer profile."""
    with github_group(f"Reject {label} prebuilt profile mismatch"):
        result = run(
            (
                "cargo",
                "check",
                "--manifest-path",
                consumer_dir / "Cargo.toml",
                "--target",
                target,
                "--locked",
                "--offline",
            ),
            env=prebuilt_consumer_environment(artifact_root, target_dir),
            capture_output=True,
            combine_output=True,
            accepted_returncodes=None,
        )
        try:
            verify_profile_mismatch_result(label, result)
        except VerificationError:
            if result.stdout:
                print(result.stdout, end="")
            raise
        print(f"Verified expected profile rejection: {label}")


def verify_candidate_mismatch_result(
    label: str, result: subprocess.CompletedProcess[str]
) -> None:
    """Require candidate rejection before any validated core metadata is emitted."""
    output = result.stdout or ""
    if result.returncode == 0:
        raise VerificationError(f"{label} candidate mismatch unexpectedly succeeded")
    if "artifact candidate mismatch" not in output:
        raise VerificationError(
            f"{label} failed without the strict artifact candidate diagnostic"
        )
    forbidden_metadata = (
        "cargo:ARTIFACT_PROFILE_HASH=",
        "cargo:ARTIFACT_IDENTITY_HASH=",
        "cargo:CANDIDATE_SHA=",
    )
    leaked = tuple(name for name in forbidden_metadata if name in output)
    if leaked:
        raise VerificationError(
            f"{label} emitted validated metadata before candidate rejection: "
            + ", ".join(leaked)
        )


def reject_prebuilt_candidate_mismatch(
    consumer_dir: Path,
    artifact_root: Path,
    target: str,
    target_dir: Path,
    candidate_sha: str,
) -> None:
    """Prove explicit candidate assertions fail closed before metadata export."""
    candidate_sha = _validate_candidate_sha(candidate_sha)
    wrong_candidate = ("0" if candidate_sha[0] != "0" else "1") + candidate_sha[1:]
    with github_group("Reject mismatched prebuilt candidate identity"):
        env = prebuilt_consumer_environment(artifact_root, target_dir)
        env["DEAR_IMGUI_RS_CANDIDATE_SHA"] = wrong_candidate
        result = run(
            (
                "cargo",
                "check",
                "--verbose",
                "--manifest-path",
                consumer_dir / "Cargo.toml",
                "--target",
                target,
                "--locked",
                "--offline",
            ),
            env=env,
            capture_output=True,
            combine_output=True,
            accepted_returncodes=None,
        )
        try:
            verify_candidate_mismatch_result("core-prebuilt", result)
        except VerificationError:
            if result.stdout:
                print(result.stdout, end="")
            raise
        print("Verified candidate rejection before Cargo metadata export")


def verify_core_prebuilt_packages(
    package_dir: Path,
    target: str,
    *,
    crt: str = "",
    source_root: Path = WORKSPACE_ROOT,
    profile_scope: str = "base",
    candidate_sha: str | None = None,
) -> None:
    """Consume core profiles and, when identified, every matching safe extension."""
    profiles = _required_profiles(profile_scope)
    with temporary_workspace("dear-imgui-prebuilt-consumer.") as work_dir:
        selected = select_core_prebuilt_archives(
            package_dir,
            target,
            crt,
            profile_scope=profile_scope,
            candidate_sha=candidate_sha,
        )
        extension_selected = (
            select_extension_prebuilt_archives(
                package_dir,
                target,
                crt,
                candidate_sha,
                source_root,
                selected,
                profile_scope=profile_scope,
            )
            if candidate_sha is not None
            else {}
        )
        for profile in profiles:
            artifact_root = work_dir / "artifacts" / profile
            safe_extract_tar(selected[profile], artifact_root)
            if not (artifact_root / "manifest.txt").is_file():
                raise VerificationError(
                    f"extracted {profile} artifact does not contain manifest.txt"
                )
            consumer_dir = work_dir / "consumers" / profile
            write_prebuilt_consumer(consumer_dir, source_root, profile)
            run_prebuilt_consumer(
                profile,
                consumer_dir,
                artifact_root,
                target,
                work_dir / "targets" / profile,
            )

        for spec in EXTENSION_SPECS:
            for profile in profiles:
                key = (spec.extension_id, profile)
                if key not in extension_selected:
                    continue
                artifact_root = work_dir / "artifacts" / spec.extension_id / profile
                safe_extract_tar(extension_selected[key], artifact_root)
                if not (artifact_root / "manifest.txt").is_file():
                    raise VerificationError(
                        f"extracted {spec.extension_id} {profile} artifact lacks manifest.txt"
                    )
                consumer_dir = work_dir / "consumers" / spec.extension_id / profile
                write_extension_prebuilt_consumer(
                    consumer_dir, source_root, spec, profile
                )
                run_extension_prebuilt_consumer(
                    f"{spec.extension_id}-{profile}",
                    consumer_dir,
                    work_dir / "artifacts" / profile,
                    artifact_root,
                    spec,
                    target,
                    work_dir / "targets" / spec.extension_id / profile,
                )

        reject_prebuilt_profile_mismatch(
            "normal-consumer-with-stack-layout-artifact",
            work_dir / "consumers" / "normal",
            work_dir / "artifacts" / "stack-layout",
            target,
            work_dir / "targets" / "mismatch-normal",
        )
        reject_prebuilt_profile_mismatch(
            "stack-layout-consumer-with-normal-artifact",
            work_dir / "consumers" / "stack-layout",
            work_dir / "artifacts" / "normal",
            target,
            work_dir / "targets" / "mismatch-stack-layout",
        )
        if candidate_sha is not None:
            reject_prebuilt_candidate_mismatch(
                work_dir / "consumers" / "normal",
                work_dir / "artifacts" / "normal",
                target,
                work_dir / "targets" / "mismatch-candidate",
                candidate_sha,
            )
    extension_note = " plus all safe extensions" if candidate_sha is not None else ""
    print(
        f"Verified {' '.join(profiles)} prebuilt consumer round-trips"
        f"{extension_note} for {target}."
    )


def verify_prebuilt_packages(
    package_dir: Path,
    target: str,
    candidate_sha: str,
    *,
    crt: str = "",
    source_root: Path = WORKSPACE_ROOT,
    profile_scope: str = "base",
) -> None:
    """Consume exact core and all six safe extension artifact routes."""
    verify_core_prebuilt_packages(
        package_dir,
        target,
        crt=crt,
        source_root=source_root,
        profile_scope=profile_scope,
        candidate_sha=_validate_candidate_sha(candidate_sha),
    )


def _built_core_archive(package_dir: Path, profile_name: str) -> Path:
    expected = PREBUILT_PROFILE_BY_NAME[profile_name].artifact_features
    matches = []
    for archive in sorted(package_dir.glob("dear-imgui-*.tar.gz")):
        fields = _read_prebuilt_manifest(archive)
        core_artifact_identity(fields, archive)
        if frozenset(_canonical_features(fields, archive)) == expected:
            matches.append(archive)
    if len(matches) != 1:
        rendered = ", ".join(str(path) for path in matches) or "none"
        raise VerificationError(
            f"expected exactly one freshly built {profile_name} core archive, found {rendered}"
        )
    return matches[0]


def _build_prebuilt_packages(
    package_workspace: Path,
    target_dir: Path,
    package_dir: Path,
    candidate_sha: str,
    *,
    target: str | None,
    crt: str | None,
    profile_scope: str,
) -> None:
    """Build exact core and matching extension profiles for one native target."""
    candidate_sha = _validate_candidate_sha(candidate_sha)
    target_arguments = ("--target", target) if target is not None else ()
    for profile_name in _required_profiles(profile_scope):
        profile = PREBUILT_PROFILE_BY_NAME[profile_name]
        package_features = ",".join(profile.consumer_features)
        values: dict[str, str | Path] = {
            "DEAR_IMGUI_RS_CANDIDATE_SHA": candidate_sha,
            "IMGUI_SYS_FORCE_BUILD": "1",
            "IMGUI_SYS_PACKAGE_DIR": package_dir,
            "CARGO_TARGET_DIR": target_dir / f"native-{profile.name}",
        }
        if crt is not None:
            values["IMGUI_SYS_PKG_CRT"] = crt
        unset = NATIVE_BUILD_ENV_TO_UNSET
        if package_features:
            values["IMGUI_SYS_PKG_FEATURES"] = package_features
        else:
            unset = (*unset, "IMGUI_SYS_PKG_FEATURES")

        command = [
            "cargo",
            "run",
            "-p",
            "dear-imgui-sys",
            "--release",
            *target_arguments,
        ]
        if profile.consumer_features:
            command.append("--no-default-features")
        command.extend(
            (
                "--features",
                ",".join(("package-bin", *profile.consumer_features)),
                "--bin",
                "package",
            )
        )
        with github_group(f"Build {profile.name} Dear ImGui host prebuilt"):
            run(
                command,
                cwd=package_workspace,
                env=environment(values, unset=unset),
            )

        core_archive = _built_core_archive(package_dir, profile_name)
        core_manifest = _read_prebuilt_manifest(core_archive)
        core_identity = core_artifact_identity(
            core_manifest, core_archive, candidate_sha
        )
        for spec in EXTENSION_SPECS:
            if profile_name not in spec.profiles:
                continue
            _, sys_features = extension_consumer_features(spec, profile_name)
            extension_features = ("package-bin", "build-from-source", *sys_features)
            extension_values: dict[str, str | Path] = {
                "DEAR_IMGUI_RS_CANDIDATE_SHA": candidate_sha,
                "DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH": core_identity,
                f"{spec.env_stem}_PACKAGE_DIR": package_dir,
                f"{spec.env_stem}_FORCE_BUILD": "1",
                "CARGO_TARGET_DIR": (
                    target_dir / f"native-{profile.name}-{spec.extension_id}"
                ),
            }
            if crt is not None:
                extension_values[f"{spec.env_stem}_PKG_CRT"] = crt
            extension_command = (
                "cargo",
                "run",
                "-p",
                spec.sys_crate,
                "--release",
                *target_arguments,
                "--no-default-features",
                "--features",
                ",".join(extension_features),
                "--bin",
                "package",
            )
            with github_group(
                f"Build {profile.name} {spec.safe_crate} host prebuilt"
            ):
                run(
                    extension_command,
                    cwd=package_workspace,
                    env=environment(
                        extension_values,
                        unset=NATIVE_BUILD_ENV_TO_UNSET,
                    ),
                )


def build_host_prebuilt_packages(
    package_workspace: Path,
    target_dir: Path,
    package_dir: Path,
    candidate_sha: str,
) -> str:
    """Build base host profiles and return the CRT recorded by the producer."""
    _build_prebuilt_packages(
        package_workspace,
        target_dir,
        package_dir,
        candidate_sha,
        target=None,
        crt=None,
        profile_scope="base",
    )
    normal_archive = _built_core_archive(package_dir, "normal")
    return _read_prebuilt_manifest(normal_archive)["crt"]


def build_release_prebuilt_packages(
    package_workspace: Path,
    target_dir: Path,
    package_dir: Path,
    target: str,
    candidate_sha: str,
    *,
    crt: str = "",
) -> None:
    """Build the complete release profile matrix for one explicit target and CRT."""
    _build_prebuilt_packages(
        package_workspace,
        target_dir,
        package_dir,
        candidate_sha,
        target=target,
        crt=crt,
        profile_scope="all",
    )
