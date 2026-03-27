#!/usr/bin/env python3

from __future__ import annotations

import argparse
import shutil
import subprocess
from pathlib import Path


CRATE_NAME = "dear_imgui_ios_smoke"
XCFRAMEWORK_NAME = "DearImguiIosSmoke.xcframework"
DEVICE_TARGET = "aarch64-apple-ios"
SIM_TARGET = "aarch64-apple-ios-sim"


def parse_args() -> argparse.Namespace:
    script_path = Path(__file__).resolve()
    crate_dir = script_path.parent.parent

    parser = argparse.ArgumentParser(
        description=(
            "Build dear-imgui-ios-smoke for iOS device + simulator and package "
            "the static libraries into an XCFramework."
        )
    )
    parser.add_argument(
        "--profile",
        choices=("debug", "release"),
        default="release",
        help="Cargo profile to build (default: release).",
    )
    parser.add_argument(
        "--output",
        default=str(crate_dir / "dist" / XCFRAMEWORK_NAME),
        help="XCFramework output path.",
    )
    return parser.parse_args()


def run(argv: list[str]) -> None:
    print("+", " ".join(argv), flush=True)
    completed = subprocess.run(argv)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def main() -> int:
    args = parse_args()
    script_path = Path(__file__).resolve()
    crate_dir = script_path.parent.parent
    manifest_path = crate_dir / "Cargo.toml"
    header_dir = crate_dir / "include"
    dist_dir = crate_dir / "dist"
    output_path = Path(args.output).expanduser().resolve()

    if shutil.which("cargo") is None:
        raise SystemExit("cargo was not found on PATH.")
    if shutil.which("rustup") is None:
        raise SystemExit("rustup was not found on PATH.")
    if shutil.which("xcodebuild") is None:
        raise SystemExit("xcodebuild was not found on PATH.")

    dist_dir.mkdir(parents=True, exist_ok=True)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    print("Ensuring Rust iOS targets are installed...", flush=True)
    run(["rustup", "target", "add", DEVICE_TARGET, SIM_TARGET])

    cargo_common = ["cargo", "build", "--manifest-path", str(manifest_path)]
    if args.profile == "release":
        cargo_common.append("--release")

    print(f"Building static library for {DEVICE_TARGET} ({args.profile})...", flush=True)
    run([*cargo_common, "--target", DEVICE_TARGET])

    print(f"Building static library for {SIM_TARGET} ({args.profile})...", flush=True)
    run([*cargo_common, "--target", SIM_TARGET])

    lib_name = f"lib{CRATE_NAME}.a"
    device_lib = crate_dir / "target" / DEVICE_TARGET / args.profile / lib_name
    sim_lib = crate_dir / "target" / SIM_TARGET / args.profile / lib_name

    if not device_lib.exists():
        raise SystemExit(f"Missing device library: {device_lib}")
    if not sim_lib.exists():
        raise SystemExit(f"Missing simulator library: {sim_lib}")

    if output_path.exists():
        shutil.rmtree(output_path)

    print("Creating XCFramework...", flush=True)
    run(
        [
            "xcodebuild",
            "-create-xcframework",
            "-library",
            str(device_lib),
            "-headers",
            str(header_dir),
            "-library",
            str(sim_lib),
            "-headers",
            str(header_dir),
            "-output",
            str(output_path),
        ]
    )

    print(f"XCFramework written to {output_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
