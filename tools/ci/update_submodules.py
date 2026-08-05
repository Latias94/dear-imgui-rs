#!/usr/bin/env python3
"""Initialize only the nested submodules required by repository CI."""

from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
import time
from collections.abc import Callable, Sequence
from pathlib import Path


CI_DIR = Path(__file__).resolve().parent
WORKSPACE_ROOT = CI_DIR.parents[1]
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _submodules import SUBMODULE_PROFILES  # noqa: E402
from _process import github_group  # noqa: E402


Runner = Callable[..., subprocess.CompletedProcess]
Sleeper = Callable[[float], None]


def retry(
    command: Sequence[str],
    *,
    runner: Runner = subprocess.run,
    sleeper: Sleeper = time.sleep,
    attempts: int = 5,
    initial_delay: int = 5,
    timeout_seconds: int = 180,
) -> None:
    """Run a command with bounded exponential backoff and per-attempt timeout."""
    delay = initial_delay
    failure: subprocess.CalledProcessError | subprocess.TimeoutExpired | None = None
    for attempt in range(1, attempts + 1):
        try:
            result = runner(
                list(command),
                cwd=WORKSPACE_ROOT,
                check=False,
                timeout=timeout_seconds,
            )
        except subprocess.TimeoutExpired as error:
            failure = error
        else:
            if result.returncode == 0:
                return
            failure = subprocess.CalledProcessError(result.returncode, list(command))

        if attempt < attempts:
            rendered = shlex.join(command)
            print(
                f"Command failed (attempt {attempt}/{attempts}): {rendered}",
                file=sys.stderr,
            )
            print(f"Retrying in {delay}s...", file=sys.stderr)
            sleeper(delay)
            delay *= 2

    assert failure is not None
    rendered = shlex.join(command)
    print(
        f"Command failed after {attempts} attempts: {rendered}",
        file=sys.stderr,
    )
    raise failure


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Initialize the nested submodules required by repository CI",
        epilog=(
            "The default profile expects top-level repository submodules to be "
            "initialized by checkout. Runtime profiles initialize only their own "
            "top-level sources."
        ),
    )
    parser.add_argument(
        "--profile",
        choices=tuple(SUBMODULE_PROFILES),
        default="all",
        help="Submodule source profile to initialize (default: %(default)s)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    with github_group("Init nested submodules (selective)"):
        try:
            for command in SUBMODULE_PROFILES[args.profile]:
                retry(command)
        except (OSError, subprocess.SubprocessError) as error:
            print(f"::error::{error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
