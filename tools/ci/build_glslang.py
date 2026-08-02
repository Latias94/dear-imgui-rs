#!/usr/bin/env python3
"""Build the pinned glslangValidator used to verify checked-in Ash shaders."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from collections.abc import Callable, Sequence
from pathlib import Path


GLSLANG_REPOSITORY = "https://github.com/KhronosGroup/glslang.git"
GLSLANG_COMMIT = "1062752a891c95b2bfeed9e356562d88f9df84ac"
Runner = Callable[..., subprocess.CompletedProcess[str]]


class GlslangBuildError(RuntimeError):
    """Raised when the pinned compiler cannot be built unambiguously."""


def build_glslang(
    work_root: Path,
    github_env: Path,
    *,
    runner: Runner = subprocess.run,
) -> Path:
    work_root = work_root.resolve()
    github_env = github_env.resolve()
    source = work_root / f"glslang-{GLSLANG_COMMIT}"
    build = work_root / f"glslang-build-{GLSLANG_COMMIT}"
    for path in (source, build):
        if path.exists():
            raise GlslangBuildError(
                f"refusing to reuse an existing pinned glslang path: {path}"
            )

    commands = (
        ("git", "init", source),
        (
            "git",
            "-C",
            source,
            "fetch",
            "--depth",
            "1",
            GLSLANG_REPOSITORY,
            GLSLANG_COMMIT,
        ),
        ("git", "-C", source, "checkout", "--detach", "FETCH_HEAD"),
        (
            "cmake",
            "-S",
            source,
            "-B",
            build,
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_EXTERNAL=OFF",
            "-DENABLE_OPT=OFF",
            "-DENABLE_SPVREMAPPER=OFF",
            "-DGLSLANG_ENABLE_INSTALL=OFF",
            "-DGLSLANG_TESTS=OFF",
        ),
        (
            "cmake",
            "--build",
            build,
            "--target",
            "glslang-standalone",
            "--parallel",
            "2",
        ),
    )
    for command in commands:
        runner(command, check=True, text=True)

    executable = "glslangValidator.exe" if os.name == "nt" else "glslangValidator"
    compiler = build / "StandAlone" / executable
    if not compiler.is_file() or compiler.stat().st_size == 0:
        raise GlslangBuildError(
            f"pinned glslang build did not produce a compiler: {compiler}"
        )
    compiler_value = str(compiler)
    if "\r" in compiler_value or "\n" in compiler_value:
        raise GlslangBuildError("compiler path cannot be written to GITHUB_ENV")
    with github_env.open("a", encoding="utf-8", newline="\n") as environment_file:
        environment_file.write(f"GLSLANG_VALIDATOR={compiler_value}\n")
    print(f"Built pinned glslangValidator: {compiler}")
    return compiler


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-root", type=Path, required=True)
    parser.add_argument("--github-env", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        build_glslang(args.work_root, args.github_env)
    except (OSError, subprocess.CalledProcessError, GlslangBuildError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
