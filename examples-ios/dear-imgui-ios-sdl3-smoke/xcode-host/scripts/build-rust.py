#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
from pathlib import Path


CRATE_NAME = "dear_imgui_ios_sdl3_smoke"


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
    effective_platform = os.environ.get("EFFECTIVE_PLATFORM_NAME", f"-{platform}")

    parser = argparse.ArgumentParser(
        description="Build the Rust static library used by the Xcode iOS host."
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
        "--effective-platform-name",
        default=effective_platform,
        help="Xcode EFFECTIVE_PLATFORM_NAME value (default: %(default)s).",
    )
    parser.add_argument(
        "--output-dir",
        default=str(
            example_root
            / "xcode-host"
            / "build"
            / "rust"
            / f"{configuration}{effective_platform}"
        ),
        help="Directory where the linked static library should be copied.",
    )
    return parser.parse_args()


def run(argv: list[str]) -> None:
    print("+", " ".join(argv), flush=True)
    completed = subprocess.run(argv)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def resolve_rust_target(platform: str, arch: str) -> str:
    if platform == "iphoneos":
        return "aarch64-apple-ios"

    normalized_arch = arch.strip().lower()
    if normalized_arch.startswith("arm64"):
        return "aarch64-apple-ios-sim"
    if normalized_arch == "x86_64":
        return "x86_64-apple-ios"

    raise SystemExit(
        f"Unsupported simulator architecture '{arch}'. "
        "Expected arm64/arm64e or x86_64."
    )


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
        # Xcode can leave CURRENT_ARCH undefined when it decides to build all
        # simulator architectures. Build both slices so the static library links
        # regardless of which simulator arch the target resolves to.
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


def main() -> int:
    args = parse_args()

    script_path = Path(__file__).resolve()
    example_root = script_path.parent.parent.parent
    manifest_path = example_root / "Cargo.toml"
    cargo_target_dir = example_root / "target" / "xcode-host-build"
    output_dir = Path(args.output_dir).expanduser().resolve()

    if shutil.which("cargo") is None:
        raise SystemExit("cargo was not found on PATH.")
    rustup = shutil.which("rustup")
    if rustup is None:
        raise SystemExit("rustup was not found on PATH.")

    profile = "release" if args.configuration.lower() == "release" else "debug"
    requested_arches = resolve_requested_arches(args.platform, args.arch, args.archs)

    installed_targets_output = subprocess.check_output(
        [rustup, "target", "list", "--installed"],
        text=True,
    )
    installed_targets = {
        line.strip() for line in installed_targets_output.splitlines() if line.strip()
    }
    rust_targets = [
        resolve_rust_target(args.platform, requested_arch)
        for requested_arch in requested_arches
    ]
    for rust_target in rust_targets:
        if rust_target not in installed_targets:
            run([rustup, "target", "add", rust_target])

    staticlib_paths: list[Path] = []
    for requested_arch, rust_target in zip(requested_arches, rust_targets):
        cargo_args = [
            "cargo",
            "build",
            "--manifest-path",
            str(manifest_path),
            "--target",
            rust_target,
            "--target-dir",
            str(cargo_target_dir),
            "--locked",
        ]
        if profile == "release":
            cargo_args.append("--release")

        print(
            f"Building Rust static library for {args.platform} "
            f"({requested_arch}) as {rust_target}...",
            flush=True,
        )
        run(cargo_args)

        staticlib_path = cargo_target_dir / rust_target / profile / f"lib{CRATE_NAME}.a"
        if not staticlib_path.exists():
            raise SystemExit(
                f"Expected static library was not produced: {staticlib_path}"
            )
        staticlib_paths.append(staticlib_path)

    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / f"lib{CRATE_NAME}.a"
    if output_path.exists():
        output_path.unlink()

    if len(staticlib_paths) == 1:
        shutil.copy2(staticlib_paths[0], output_path)
    else:
        xcrun = shutil.which("xcrun")
        if xcrun is None:
            raise SystemExit("xcrun was not found on PATH.")
        run(
            [
                xcrun,
                "lipo",
                "-create",
                *(str(path) for path in staticlib_paths),
                "-output",
                str(output_path),
            ]
        )

    print(f"Rust static library copied to {output_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
