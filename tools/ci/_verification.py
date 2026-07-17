"""Shared lifecycle and error handling for release artifact verification."""

from __future__ import annotations

import os
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path


class VerificationError(RuntimeError):
    """A release artifact violated a repository verification contract."""


@contextmanager
def temporary_workspace(prefix: str) -> Iterator[Path]:
    """Create a verification workspace under the configured runner temp root."""
    temporary_parent = os.environ.get("RUNNER_TEMP")
    if temporary_parent and not Path(temporary_parent).is_dir():
        raise VerificationError(f"RUNNER_TEMP is not a directory: {temporary_parent}")
    with tempfile.TemporaryDirectory(prefix=prefix, dir=temporary_parent) as temporary:
        yield Path(temporary)
