"""Shared subprocess and environment helpers for repository CI tools."""

from __future__ import annotations

import os
import subprocess
from collections.abc import Iterable, Iterator, Mapping, Sequence
from contextlib import contextmanager
from pathlib import Path


class CommandError(RuntimeError):
    """A subprocess failed with a return code outside its accepted contract."""

    def __init__(
        self,
        command: Sequence[str],
        returncode: int,
        output: str = "",
    ) -> None:
        self.command = tuple(command)
        self.returncode = returncode
        self.output = output
        rendered = subprocess.list2cmdline(self.command)
        detail = f"command failed with exit code {returncode}: {rendered}"
        if output.strip():
            detail = f"{detail}\n{output.rstrip()}"
        super().__init__(detail)


def environment(
    values: Mapping[str, str | Path] | None = None,
    *,
    unset: Iterable[str] = (),
) -> dict[str, str]:
    """Return a process environment with explicit updates and removals."""
    result = os.environ.copy()
    for name in unset:
        result.pop(name, None)
    if values is not None:
        result.update({name: os.fspath(value) for name, value in values.items()})
    return result


def run(
    command: Sequence[str | Path],
    *,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    capture_output: bool = False,
    combine_output: bool = False,
    quiet_stdout: bool = False,
    accepted_returncodes: Iterable[int] | None = (0,),
) -> subprocess.CompletedProcess[str]:
    """Run without a shell; use accepted_returncodes=None to preserve any result."""
    rendered_command = [os.fspath(argument) for argument in command]
    stdout: int | None = None
    stderr: int | None = None
    if capture_output:
        stdout = subprocess.PIPE
        stderr = subprocess.STDOUT if combine_output else subprocess.PIPE
    elif quiet_stdout:
        stdout = subprocess.DEVNULL

    try:
        result = subprocess.run(
            rendered_command,
            cwd=cwd,
            env=env,
            check=False,
            stdout=stdout,
            stderr=stderr,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except OSError as error:
        rendered = subprocess.list2cmdline(rendered_command)
        raise CommandError(
            rendered_command, -1, f"could not run {rendered}: {error}"
        ) from error

    if accepted_returncodes is not None and result.returncode not in frozenset(
        accepted_returncodes
    ):
        output = result.stdout or ""
        if result.stderr:
            output = f"{output}{result.stderr}"
        raise CommandError(rendered_command, result.returncode, output)
    return result


@contextmanager
def github_group(label: str) -> Iterator[None]:
    """Keep GitHub Actions logs grouped while still closing failed groups."""
    print(f"::group::{label}", flush=True)
    try:
        yield
    finally:
        print("::endgroup::", flush=True)
