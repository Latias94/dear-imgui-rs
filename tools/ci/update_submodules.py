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

from _submodules import SUBMODULE_COMMANDS  # noqa: E402
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
) -> None:
    """Run a command with bounded exponential backoff."""
    delay = initial_delay
    result: subprocess.CompletedProcess | None = None
    for attempt in range(1, attempts + 1):
        result = runner(list(command), cwd=WORKSPACE_ROOT, check=False)
        if result.returncode == 0:
            return
        if attempt < attempts:
            rendered = shlex.join(command)
            print(
                f"Command failed (attempt {attempt}/{attempts}): {rendered}",
                file=sys.stderr,
            )
            print(f"Retrying in {delay}s...", file=sys.stderr)
            sleeper(delay)
            delay *= 2

    assert result is not None
    rendered = shlex.join(command)
    print(
        f"Command failed after {attempts} attempts: {rendered}",
        file=sys.stderr,
    )
    raise subprocess.CalledProcessError(result.returncode, list(command))


def _build_parser() -> argparse.ArgumentParser:
    return argparse.ArgumentParser(
        description="Initialize the nested submodules required by repository CI",
        epilog=(
            "Top-level repository submodules must already be initialized, for "
            "example by the checkout action or git submodule update --init."
        ),
    )


def main(argv: Sequence[str] | None = None) -> int:
    _build_parser().parse_args(argv)
    with github_group("Init nested submodules (selective)"):
        try:
            for command in SUBMODULE_COMMANDS:
                retry(command)
        except (OSError, subprocess.CalledProcessError) as error:
            print(f"::error::{error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
