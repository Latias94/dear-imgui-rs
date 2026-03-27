#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


APK_NAME = "dear-imgui-android-smoke.apk"


def parse_args() -> argparse.Namespace:
    script_path = Path(__file__).resolve()
    example_root = script_path.parent.parent

    parser = argparse.ArgumentParser(
        description=(
            "Build per-target Android APKs for dear-imgui-android-smoke with "
            "isolated target directories."
        )
    )
    parser.add_argument(
        "--profile",
        choices=("debug", "release"),
        default="debug",
        help="Cargo profile to build (default: debug).",
    )
    parser.add_argument(
        "--targets",
        default="aarch64-linux-android",
        help=(
            "Comma-separated Rust Android targets to build. "
            "Example: aarch64-linux-android,x86_64-linux-android"
        ),
    )
    parser.add_argument(
        "--target-dir-root",
        default=str(example_root / "target" / "packaged-apks"),
        help="Root directory for per-target cargo --target-dir outputs.",
    )
    parser.add_argument(
        "--keystore-path",
        help="Release keystore path. Optional for debug builds.",
    )
    parser.add_argument(
        "--keystore-password",
        help="Release keystore password. Optional for debug builds.",
    )
    return parser.parse_args()


def run(argv: list[str], env: dict[str, str] | None = None) -> None:
    print("+", " ".join(argv), flush=True)
    completed = subprocess.run(argv, env=env)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def main() -> int:
    args = parse_args()
    script_path = Path(__file__).resolve()
    example_root = script_path.parent.parent
    manifest_path = example_root / "Cargo.toml"
    target_dir_root = Path(args.target_dir_root).resolve()
    targets = [target.strip() for target in args.targets.split(",") if target.strip()]

    if not targets:
        raise SystemExit("No Rust Android targets were provided.")

    if shutil.which("cargo") is None:
        raise SystemExit("cargo was not found on PATH.")

    rustup = shutil.which("rustup")
    if rustup is None:
        raise SystemExit("rustup was not found on PATH.")

    installed_targets_output = subprocess.check_output(
        [rustup, "target", "list", "--installed"],
        text=True,
    )
    installed_targets = {
        line.strip() for line in installed_targets_output.splitlines() if line.strip()
    }
    missing_targets = [target for target in targets if target not in installed_targets]
    if missing_targets:
        joined = " ".join(missing_targets)
        raise SystemExit(
            f"Missing Rust Android targets: {', '.join(missing_targets)}. "
            f"Install them with: rustup target add {joined}"
        )

    env = os.environ.copy()
    original_keystore = env.get("CARGO_APK_RELEASE_KEYSTORE")
    original_keystore_password = env.get("CARGO_APK_RELEASE_KEYSTORE_PASSWORD")

    try:
        if args.profile == "release":
            if args.keystore_path:
                env["CARGO_APK_RELEASE_KEYSTORE"] = str(
                    Path(args.keystore_path).expanduser().resolve()
                )
            if args.keystore_password:
                env["CARGO_APK_RELEASE_KEYSTORE_PASSWORD"] = args.keystore_password

            if (
                not env.get("CARGO_APK_RELEASE_KEYSTORE")
                or not env.get("CARGO_APK_RELEASE_KEYSTORE_PASSWORD")
            ):
                raise SystemExit(
                    "Release builds require CARGO_APK_RELEASE_KEYSTORE and "
                    "CARGO_APK_RELEASE_KEYSTORE_PASSWORD, either as arguments "
                    "or pre-set environment variables."
                )

        for target in targets:
            target_dir = target_dir_root / target
            target_dir.mkdir(parents=True, exist_ok=True)

            cargo_args = [
                "cargo",
                "apk2",
                "build",
                "--manifest-path",
                str(manifest_path),
                "--target",
                target,
                "--target-dir",
                str(target_dir),
            ]

            if args.profile == "release":
                cargo_args.append("--release")

            print(f"Building {args.profile} APK for {target} ...", flush=True)
            run(cargo_args, env=env)

            apk_path = target_dir / args.profile / "apk" / APK_NAME
            if not apk_path.exists():
                raise SystemExit(
                    f"Expected APK was not produced for {target}: {apk_path}"
                )
            print(f"APK ready: {apk_path}", flush=True)
    finally:
        if original_keystore is None:
            env.pop("CARGO_APK_RELEASE_KEYSTORE", None)
        else:
            env["CARGO_APK_RELEASE_KEYSTORE"] = original_keystore

        if original_keystore_password is None:
            env.pop("CARGO_APK_RELEASE_KEYSTORE_PASSWORD", None)
        else:
            env["CARGO_APK_RELEASE_KEYSTORE_PASSWORD"] = original_keystore_password

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
