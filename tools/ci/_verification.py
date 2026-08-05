"""Shared lifecycle and error handling for release artifact verification."""

from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any


class VerificationError(RuntimeError):
    """A release artifact violated a repository verification contract."""


_CANDIDATE_SHA = re.compile(r"[0-9a-f]{40}")


def parse_candidate_sha(value: str) -> str:
    """Validate a full lowercase Git commit ID."""
    if _CANDIDATE_SHA.fullmatch(value) is None:
        raise VerificationError(
            "candidate SHA must be exactly 40 lowercase hexadecimal characters"
        )
    return value


def resolve_candidate_sha(repo_root: Path, expected: str) -> str:
    """Require the checked-out commit to match the expected candidate."""
    expected = parse_candidate_sha(expected)
    try:
        completed = subprocess.run(
            ("git", "rev-parse", "--verify", "HEAD"),
            cwd=repo_root,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise VerificationError(f"could not resolve candidate HEAD: {error}") from error
    if completed.returncode != 0:
        diagnostic = completed.stderr.strip() or "git rev-parse failed"
        raise VerificationError(diagnostic)
    actual = parse_candidate_sha(completed.stdout.strip())
    if actual != expected:
        raise VerificationError(
            f"candidate HEAD mismatch: expected {expected}, found {actual}"
        )
    return actual


def write_json(path: Path, value: Any) -> None:
    """Write deterministic JSON evidence for a CI artifact."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


@contextmanager
def temporary_workspace(prefix: str) -> Iterator[Path]:
    """Create a verification workspace under the configured runner temp root."""
    temporary_parent = os.environ.get("RUNNER_TEMP")
    if temporary_parent and not Path(temporary_parent).is_dir():
        raise VerificationError(f"RUNNER_TEMP is not a directory: {temporary_parent}")
    with tempfile.TemporaryDirectory(prefix=prefix, dir=temporary_parent) as temporary:
        yield Path(temporary)
