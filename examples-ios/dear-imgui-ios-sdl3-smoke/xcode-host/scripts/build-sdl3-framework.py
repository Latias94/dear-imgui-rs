#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import plistlib
import re
import shutil
import subprocess
from pathlib import Path


FRAMEWORK_NAME = "SDL3.framework"


def parse_args() -> argparse.Namespace:
    script_path = Path(__file__).resolve()
    example_root = script_path.parent.parent.parent

    configuration = os.environ.get("CONFIGURATION", "Debug")
    platform = os.environ.get("PLATFORM_NAME", "iphonesimulator")
    arch = (
        os.environ.get("NATIVE_ARCH_ACTUAL")
        or os.environ.get("CURRENT_ARCH")
        or os.environ.get("ARCHS", "arm64").split()[0]
    )
    archs = os.environ.get("ARCHS", arch)

    parser = argparse.ArgumentParser(
        description="Resolve or build the SDL3 framework used by the Xcode host."
    )
    parser.add_argument(
        "--configuration",
        default=configuration,
        help="Xcode build configuration name (default: %(default)s).",
    )
    parser.add_argument(
        "--platform",
        default=platform,
        choices=("iphoneos", "iphonesimulator"),
        help="Xcode platform name (default: %(default)s).",
    )
    parser.add_argument(
        "--arch",
        default=arch,
        help="Target architecture reported by Xcode (default: %(default)s).",
    )
    parser.add_argument(
        "--archs",
        default=archs,
        help="Space- or comma-separated architectures requested by Xcode "
        "(default: %(default)s).",
    )
    parser.add_argument(
        "--output-dir",
        default=str(example_root / "xcode-host" / "build" / "frameworks"),
        help="Directory where the selected SDL3.framework should be copied.",
    )
    return parser.parse_args()


def run(argv: list[str]) -> None:
    print("+", " ".join(argv), flush=True)
    completed = subprocess.run(argv)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def normalize_arch(arch: str) -> str:
    normalized_arch = arch.strip().lower()
    if normalized_arch in {"", "undefined_arch"}:
        return ""
    if normalized_arch.startswith("arm64"):
        return "arm64"
    if normalized_arch == "x86_64":
        return "x86_64"
    return normalized_arch


def split_arches(value: str) -> list[str]:
    return [part for part in value.replace(",", " ").split() if part]


def resolve_requested_arches(platform: str, arch: str, archs: str) -> list[str]:
    if platform == "iphoneos":
        normalized_arch = normalize_arch(arch) or "arm64"
        if normalized_arch != "arm64":
            raise SystemExit(
                f"Unsupported device architecture '{arch}'. Expected arm64/arm64e."
            )
        return ["arm64"]

    requested_arches = [normalize_arch(item) for item in split_arches(archs)]
    requested_arches = [item for item in requested_arches if item]

    fallback_arch = normalize_arch(arch)
    if fallback_arch and fallback_arch not in requested_arches:
        requested_arches.append(fallback_arch)

    if not requested_arches:
        requested_arches = ["arm64", "x86_64"]

    supported_arches = {"arm64", "x86_64"}
    unsupported_arches = [item for item in requested_arches if item not in supported_arches]
    if unsupported_arches:
        unsupported_text = ", ".join(sorted(set(unsupported_arches)))
        raise SystemExit(
            f"Unsupported simulator architecture(s): {unsupported_text}. "
            "Expected arm64/arm64e or x86_64."
        )

    deduped_arches: list[str] = []
    for item in requested_arches:
        if item not in deduped_arches:
            deduped_arches.append(item)
    return deduped_arches


def cargo_home() -> Path:
    value = os.environ.get("CARGO_HOME")
    if value:
        return Path(value).expanduser().resolve()
    return (Path.home() / ".cargo").resolve()


def copy_framework(source_framework: Path, destination_root: Path) -> Path:
    if not source_framework.exists():
        raise SystemExit(f"Missing SDL3 framework: {source_framework}")

    destination_root.mkdir(parents=True, exist_ok=True)
    destination_framework = destination_root / FRAMEWORK_NAME
    if destination_framework.exists():
        shutil.rmtree(destination_framework)
    shutil.copytree(source_framework, destination_framework, symlinks=True)
    return destination_framework


def expected_supported_platform(platform: str) -> str:
    if platform == "iphoneos":
        return "iPhoneOS"
    if platform == "iphonesimulator":
        return "iPhoneSimulator"
    raise SystemExit(f"Unsupported Apple platform '{platform}'.")


def read_framework_info(framework_path: Path) -> dict[str, object]:
    info_path = framework_path / "Info.plist"
    if not info_path.exists():
        return {}
    with info_path.open("rb") as handle:
        return plistlib.load(handle)


def validate_framework_platform(
    framework_path: Path, platform: str, requested_arches: list[str]
) -> None:
    info = read_framework_info(framework_path)
    expected_platform = expected_supported_platform(platform)
    supported_platforms = info.get("CFBundleSupportedPlatforms", [])
    if supported_platforms and expected_platform not in supported_platforms:
        raise SystemExit(
            f"{framework_path} supports {supported_platforms}, not {expected_platform}."
        )

    binary_path = framework_path / "SDL3"
    if not binary_path.exists():
        raise SystemExit(f"Missing SDL3 binary inside framework: {binary_path}")

    xcrun = shutil.which("xcrun")
    if xcrun is None:
        raise SystemExit("xcrun was not found on PATH.")

    arch_text = subprocess.check_output(
        [xcrun, "lipo", "-archs", str(binary_path)],
        text=True,
    ).strip()
    available_arches = set(arch_text.split())
    missing_arches = [arch for arch in requested_arches if arch not in available_arches]
    if missing_arches:
        missing_text = ", ".join(missing_arches)
        raise SystemExit(
            f"{framework_path} does not provide the requested arch(es): {missing_text}. "
            f"Available arch(es): {', '.join(sorted(available_arches))}."
        )


def resolve_xcframework_slice(
    xcframework_path: Path, platform: str, requested_arches: list[str]
) -> Path:
    info_path = xcframework_path / "Info.plist"
    if not info_path.exists():
        raise SystemExit(f"Missing XCFramework Info.plist: {info_path}")

    with info_path.open("rb") as handle:
        info = plistlib.load(handle)

    expected_platform = "ios"
    expected_variant = "simulator" if platform == "iphonesimulator" else None

    candidates: list[tuple[Path, set[str]]] = []
    for library in info.get("AvailableLibraries", []):
        if library.get("SupportedPlatform") != expected_platform:
            continue
        if library.get("SupportedPlatformVariant") != expected_variant:
            continue

        relative_path = xcframework_path / library["LibraryIdentifier"] / library["LibraryPath"]
        architectures = set(library.get("SupportedArchitectures", []))
        candidates.append((relative_path, architectures))

    if not candidates:
        raise SystemExit(
            f"No matching library slice found in {xcframework_path} for platform '{platform}'."
        )

    requested_arch_set = set(requested_arches)
    for candidate_path, candidate_arches in candidates:
        if requested_arch_set.issubset(candidate_arches):
            return candidate_path

    first_candidate, first_arches = candidates[0]
    requested_text = ", ".join(requested_arches)
    available_text = ", ".join(sorted(first_arches))
    raise SystemExit(
        f"No XCFramework slice in {xcframework_path} satisfies requested arch(es) "
        f"{requested_text}. Candidate arch(es): {available_text}."
    )


def parse_version_key(name: str) -> tuple[int | str, ...]:
    version_text = name.removeprefix("sdl3-src-")
    parts: list[int | str] = []
    for token in re.split(r"[.-]", version_text):
        if token.isdigit():
            parts.append(int(token))
        else:
            parts.append(token)
    return tuple(parts)


def resolve_locked_sdl3_version(example_root: Path) -> str | None:
    metadata_output = subprocess.check_output(
        [
            "cargo",
            "metadata",
            "--manifest-path",
            str(example_root / "Cargo.toml"),
            "--format-version",
            "1",
            "--locked",
        ],
        text=True,
    )
    metadata = json.loads(metadata_output)
    for package in metadata.get("packages", []):
        if package.get("name") == "sdl3-src":
            return package.get("version")
    return None


def resolve_sdl3_source_root(example_root: Path) -> Path:
    env_root = os.environ.get("SDL3_FRAMEWORK_SOURCE_ROOT")
    if env_root:
        path = Path(env_root).expanduser().resolve()
        if not path.exists():
            raise SystemExit(f"SDL3_FRAMEWORK_SOURCE_ROOT does not exist: {path}")
        return path

    print("Resolving SDL3 source from Cargo metadata...", flush=True)
    run(
        [
            "cargo",
            "fetch",
            "--manifest-path",
            str(example_root / "Cargo.toml"),
            "--locked",
        ]
    )

    locked_version = resolve_locked_sdl3_version(example_root)
    registry_src_root = cargo_home() / "registry" / "src"
    if not registry_src_root.exists():
        raise SystemExit(f"Cargo registry source root does not exist: {registry_src_root}")

    candidates: list[Path] = []
    if locked_version is not None:
        candidates.extend(
            registry_src_root.glob(f"*/sdl3-src-{locked_version}/SDL")
        )

    if not candidates:
        all_candidates = list(registry_src_root.glob("*/sdl3-src-*/SDL"))
        if not all_candidates:
            raise SystemExit(
                "Could not find any sdl3-src source tree in Cargo's registry. "
                "Set SDL3_FRAMEWORK_SOURCE_ROOT, SDL3_FRAMEWORK_PATH, or "
                "SDL3_XCFRAMEWORK_PATH explicitly."
            )
        candidates = sorted(
            all_candidates,
            key=lambda path: parse_version_key(path.parent.name),
        )

    return candidates[-1].resolve()


def build_framework_from_source(
    source_root: Path, platform: str, configuration: str
) -> Path:
    project_path = source_root / "Xcode" / "SDL" / "SDL.xcodeproj"
    if not project_path.exists():
        raise SystemExit(f"Missing SDL Xcode project: {project_path}")

    print(f"Building SDL3.framework from {project_path} for {platform}...", flush=True)
    run(
        [
            "xcodebuild",
            "-project",
            str(project_path),
            "-target",
            "SDL3",
            "-configuration",
            configuration,
            "-sdk",
            platform,
            "CODE_SIGNING_ALLOWED=NO",
            "build",
        ]
    )

    built_framework = source_root / "Xcode" / "SDL" / "build" / f"{configuration}-{platform}" / FRAMEWORK_NAME
    if not built_framework.exists():
        raise SystemExit(f"Expected SDL3 framework was not produced: {built_framework}")
    return built_framework


def main() -> int:
    args = parse_args()

    script_path = Path(__file__).resolve()
    example_root = script_path.parent.parent.parent
    output_dir = Path(args.output_dir).expanduser().resolve()
    requested_arches = resolve_requested_arches(args.platform, args.arch, args.archs)

    if shutil.which("cargo") is None:
        raise SystemExit("cargo was not found on PATH.")
    if shutil.which("xcodebuild") is None:
        raise SystemExit("xcodebuild was not found on PATH.")

    framework_path_env = os.environ.get("SDL3_FRAMEWORK_PATH")
    xcframework_path_env = os.environ.get("SDL3_XCFRAMEWORK_PATH")

    if framework_path_env:
        source_framework = Path(framework_path_env).expanduser().resolve()
        print(f"Using SDL3.framework from SDL3_FRAMEWORK_PATH: {source_framework}", flush=True)
    elif xcframework_path_env:
        xcframework_path = Path(xcframework_path_env).expanduser().resolve()
        if not xcframework_path.exists():
            raise SystemExit(f"SDL3_XCFRAMEWORK_PATH does not exist: {xcframework_path}")
        source_framework = resolve_xcframework_slice(
            xcframework_path, args.platform, requested_arches
        )
        print(
            f"Using SDL3.framework slice from SDL3_XCFRAMEWORK_PATH: {source_framework}",
            flush=True,
        )
    else:
        source_root = resolve_sdl3_source_root(example_root)
        print(f"Using SDL3 source root: {source_root}", flush=True)
        source_framework = build_framework_from_source(
            source_root, args.platform, args.configuration
        )

    validate_framework_platform(source_framework, args.platform, requested_arches)
    destination_framework = copy_framework(source_framework, output_dir)
    print(f"SDL3.framework copied to {destination_framework}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
