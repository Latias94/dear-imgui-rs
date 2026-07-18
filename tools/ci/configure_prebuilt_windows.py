#!/usr/bin/env python3
"""Configure the Windows native dependencies for prebuilt release jobs."""

from __future__ import annotations

import argparse
import os
import shutil
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path


CI_DIR = Path(__file__).resolve().parent
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _process import run  # noqa: E402
from _verification import VerificationError  # noqa: E402


def vcpkg_triplet(crt: str) -> str:
    if crt == "mt":
        return "x64-windows-static"
    if crt == "md":
        return "x64-windows-static-md"
    raise VerificationError(f"unsupported Windows CRT profile: {crt}")


def find_vcpkg_root(environment: Mapping[str, str], executable: Path) -> Path:
    candidates = (
        environment.get("VCPKG_ROOT"),
        environment.get("VCPKG_INSTALLATION_ROOT"),
        os.fspath(executable.parent),
    )
    seen = set()
    for candidate in candidates:
        if not candidate:
            continue
        path = Path(candidate).resolve()
        if path in seen:
            continue
        seen.add(path)
        if path.joinpath(".vcpkg-root").is_file():
            return path
    raise VerificationError("vcpkg root not found in the configured environment")


def configure_windows_prebuilt(
    crt: str,
    environment: Mapping[str, str],
    github_env: Path,
) -> None:
    triplet = vcpkg_triplet(crt)
    executable_name = "vcpkg.exe" if os.name == "nt" else "vcpkg"
    executable = shutil.which(executable_name, path=environment.get("PATH"))
    if executable is None:
        raise VerificationError("vcpkg executable was not found on PATH")
    executable_path = Path(executable).resolve()
    run((executable_path, "install", f"freetype:{triplet}"))

    root = find_vcpkg_root(environment, executable_path)
    status_dir = root / "installed" / "vcpkg"
    updates_dir = status_dir / "updates"
    updates_dir.mkdir(parents=True, exist_ok=True)
    has_updates = any(path.is_file() for path in updates_dir.iterdir())
    if not (status_dir / "status").is_file() and not has_updates:
        raise VerificationError(f"vcpkg status data not found under {status_dir}")

    runner_temp = environment.get("RUNNER_TEMP")
    if not runner_temp:
        raise VerificationError("RUNNER_TEMP is required for Windows prebuilt jobs")
    values = {
        "VCPKG_ROOT": root,
        "VCPKGRS_TRIPLET": triplet,
        "PKG_CONFIG": Path(runner_temp) / "missing-pkg-config.exe",
        "PKG_CONFIG_PATH": "",
    }
    if crt == "mt":
        values["RUSTFLAGS"] = "-C target-feature=+crt-static"
    with github_env.open("a", encoding="utf-8", newline="\n") as destination:
        for name, value in values.items():
            destination.write(f"{name}={value}\n")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--crt", choices=("md", "mt"), required=True)
    parser.add_argument(
        "--github-env",
        type=Path,
        default=Path(os.environ["GITHUB_ENV"]) if "GITHUB_ENV" in os.environ else None,
    )
    args = parser.parse_args(argv)
    try:
        if args.github_env is None:
            raise VerificationError("GITHUB_ENV is required")
        configure_windows_prebuilt(args.crt, os.environ, args.github_env)
    except (OSError, VerificationError) as error:
        print(f"::error::{error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
