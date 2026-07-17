"""Build and consume exact Dear ImGui native prebuilt profiles."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

from _archive import read_unique_root_file, safe_extract_tar
from _process import environment, github_group, run
from _verification import VerificationError, temporary_workspace


WORKSPACE_ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class PrebuiltProfile:
    """One exact artifact feature set and its consumer-facing configuration."""

    name: str
    artifact_features: frozenset[str]
    consumer_features: tuple[str, ...]
    included_in_base: bool


PREBUILT_PROFILES = (
    PrebuiltProfile(
        "normal",
        frozenset(("platform-io-aggregate-hooks", "wchar32")),
        (),
        True,
    ),
    PrebuiltProfile(
        "stack-layout",
        frozenset(("platform-io-aggregate-hooks", "stack-layout", "wchar32")),
        ("stack-layout",),
        True,
    ),
    PrebuiltProfile(
        "freetype",
        frozenset(("platform-io-aggregate-hooks", "freetype", "wchar32")),
        ("freetype",),
        False,
    ),
    PrebuiltProfile(
        "stack-layout-freetype",
        frozenset(
            (
                "platform-io-aggregate-hooks",
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
PREBUILT_ENV_TO_UNSET = (
    "IMGUI_SYS_FORCE_BUILD",
    "IMGUI_SYS_PREBUILT_URL",
    "IMGUI_SYS_USE_PREBUILT",
)
NATIVE_BUILD_ENV_TO_UNSET = (
    *PREBUILT_ENV_TO_UNSET,
    "IMGUI_SYS_LIB_DIR",
    "IMGUI_SYS_SKIP_CC",
)


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
    return dict(line.split("=", 1) for line in lines[1:] if "=" in line)


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
) -> dict[str, Path]:
    """Select exactly one compatible archive for every profile in a scope."""
    required_profiles = _required_profiles(profile_scope)
    matches = {profile: [] for profile in required_profiles}
    for archive in sorted(package_dir.resolve().glob("dear-imgui-*.tar.gz")):
        fields = _read_prebuilt_manifest(archive)
        if fields.get("target") != target:
            continue
        if crt and fields.get("crt") != crt:
            continue
        features = frozenset(filter(None, fields.get("features", "").split(",")))
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
        f"""fn main() {{
    let mut context = dear_imgui_rs::Context::create();
    context.io_mut().set_display_size([320.0, 240.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let _ = context.font_atlas().build();
    {{
        let ui = context.frame();
{frame_body}    }}
    assert!(context.render().valid());
    assert!(!dear_imgui_rs::dear_imgui_version().is_empty());
}}
""",
        encoding="utf-8",
    )
    try:
        shutil.copy2(source_root / "Cargo.lock", destination / "Cargo.lock")
    except OSError as error:
        raise VerificationError(f"could not copy Cargo.lock: {error}") from error


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


def verify_core_prebuilt_packages(
    package_dir: Path,
    target: str,
    *,
    crt: str = "",
    source_root: Path = WORKSPACE_ROOT,
    profile_scope: str = "base",
) -> None:
    """Consume required prebuilt profiles and prove strict mismatch rejection."""
    profiles = _required_profiles(profile_scope)
    with temporary_workspace("dear-imgui-prebuilt-consumer.") as work_dir:
        selected = select_core_prebuilt_archives(
            package_dir, target, crt, profile_scope=profile_scope
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
    print(f"Verified {' '.join(profiles)} prebuilt consumer round-trips for {target}.")


def build_host_prebuilt_packages(
    package_workspace: Path, target_dir: Path, package_dir: Path
) -> None:
    """Build the two base host-native profiles consumed by the source gate."""
    for profile_name in _required_profiles("base"):
        profile = PREBUILT_PROFILE_BY_NAME[profile_name]
        package_features = ",".join(profile.consumer_features)
        values: dict[str, str | Path] = {
            "IMGUI_SYS_FORCE_BUILD": "1",
            "IMGUI_SYS_PACKAGE_DIR": package_dir,
            "CARGO_TARGET_DIR": target_dir / f"native-{profile.name}",
        }
        unset = NATIVE_BUILD_ENV_TO_UNSET
        if package_features:
            values["IMGUI_SYS_PKG_FEATURES"] = package_features
        else:
            unset = (*unset, "IMGUI_SYS_PKG_FEATURES")

        command = ["cargo", "run", "-p", "dear-imgui-sys", "--release"]
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
