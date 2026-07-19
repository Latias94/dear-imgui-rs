#!/usr/bin/env python3
"""Install the pinned LLVM toolchain used to regenerate bindings in CI."""

from __future__ import annotations

import argparse
import os
import platform
import sys
import tempfile
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import BinaryIO, Protocol


CI_DIR = Path(__file__).resolve().parent
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _process import CommandError, run  # noqa: E402


LLVM_VERSION = "14.0.0"
LLVM_ARCHIVE_NAME = "clang+llvm-14.0.0-x86_64-linux-gnu-ubuntu-18.04.tar.xz"
LLVM_ARCHIVE_URL = (
    "https://github.com/llvm/llvm-project/releases/download/llvmorg-14.0.0/"
    "clang%2Bllvm-14.0.0-x86_64-linux-gnu-ubuntu-18.04.tar.xz"
)
DOWNLOAD_ATTEMPTS = 3
DOWNLOAD_TIMEOUT_SECONDS = 120
DOWNLOAD_CHUNK_BYTES = 1024 * 1024


class LlvmInstallError(RuntimeError):
    """The pinned LLVM toolchain could not be installed safely."""


class Response(Protocol):
    headers: Mapping[str, str]

    def __enter__(self) -> Response: ...

    def __exit__(self, *args: object) -> None: ...

    def read(self, size: int = -1) -> bytes: ...


Opener = Callable[..., Response]


def require_supported_host() -> None:
    """Keep the pinned release asset aligned with the CI host contract."""
    system = platform.system()
    machine = platform.machine().lower()
    if system != "Linux" or machine not in {"x86_64", "amd64"}:
        raise LlvmInstallError(
            "the pinned LLVM asset only supports Linux x86_64; "
            f"detected {system} {platform.machine()}"
        )


def prepare_destination(destination: Path) -> None:
    """Create an empty destination without deleting pre-existing files."""
    if destination.exists():
        if not destination.is_dir():
            raise LlvmInstallError(f"LLVM destination is not a directory: {destination}")
        if any(destination.iterdir()):
            raise LlvmInstallError(f"LLVM destination is not empty: {destination}")
        return
    destination.mkdir(parents=True)


def _copy_response(response: Response, output: BinaryIO) -> int:
    downloaded = 0
    while chunk := response.read(DOWNLOAD_CHUNK_BYTES):
        output.write(chunk)
        downloaded += len(chunk)
    return downloaded


def download_archive(
    archive: Path,
    *,
    opener: Opener | None = None,
    sleep: Callable[[float], None] = time.sleep,
) -> None:
    """Download the version-pinned LLVM release URL with bounded retries."""
    open_url = opener or urllib.request.urlopen
    request = urllib.request.Request(
        LLVM_ARCHIVE_URL,
        headers={"User-Agent": "dear-imgui-rs-binding-contract"},
    )
    failures: list[str] = []
    for attempt in range(1, DOWNLOAD_ATTEMPTS + 1):
        archive.unlink(missing_ok=True)
        try:
            with open_url(request, timeout=DOWNLOAD_TIMEOUT_SECONDS) as response:
                expected_header = response.headers.get("Content-Length")
                with archive.open("xb") as output:
                    downloaded = _copy_response(response, output)
            if expected_header is not None and downloaded != int(expected_header):
                raise LlvmInstallError(
                    "LLVM download size mismatch: "
                    f"expected {expected_header} bytes, received {downloaded}"
                )
            print(f"Downloaded LLVM {LLVM_VERSION} ({downloaded} bytes).")
            return
        except (LlvmInstallError, OSError, ValueError, urllib.error.URLError) as error:
            failures.append(f"attempt {attempt}: {error}")
            archive.unlink(missing_ok=True)
            if attempt < DOWNLOAD_ATTEMPTS:
                sleep(float(2 ** (attempt - 1)))
    raise LlvmInstallError(
        f"could not download LLVM {LLVM_VERSION} after {DOWNLOAD_ATTEMPTS} attempts: "
        + "; ".join(failures)
    )


def extract_archive(archive: Path, destination: Path) -> None:
    """Extract the official archive with the same layout used by the old action."""
    run(
        (
            "tar",
            "xf",
            archive,
            "--strip-components=1",
            "-C",
            destination,
        )
    )


def validate_installation(destination: Path) -> None:
    """Require the tools and shared library consumed by binding generation."""
    clang = destination / "bin" / "clang"
    llvm_config = destination / "bin" / "llvm-config"
    for executable in (clang, llvm_config):
        if not executable.is_file():
            raise LlvmInstallError(f"LLVM archive is missing {executable}")

    libclang_candidates = tuple((destination / "lib").glob("libclang.so*"))
    if not any(path.is_file() for path in libclang_candidates):
        raise LlvmInstallError(
            f"LLVM archive is missing a libclang shared library in {destination / 'lib'}"
        )

    result = run((llvm_config, "--version"), capture_output=True)
    actual_version = (result.stdout or "").strip()
    if actual_version != LLVM_VERSION:
        raise LlvmInstallError(
            f"expected LLVM {LLVM_VERSION}, but llvm-config reported {actual_version!r}"
        )


def _append_lines(path: Path, lines: Sequence[str]) -> None:
    encoded = bytearray()
    for line in lines:
        if not line or "\n" in line or "\r" in line:
            raise LlvmInstallError("GitHub environment entries must be non-empty lines")
        encoded.extend(f"{line}\n".encode())
    with path.open("ab") as output:
        output.write(encoded)


def export_github_environment(
    destination: Path, environment: Mapping[str, str]
) -> None:
    """Replicate the PATH and library exports provided by the removed action."""
    github_path_value = environment.get("GITHUB_PATH")
    github_env_value = environment.get("GITHUB_ENV")
    if github_path_value is None and github_env_value is None:
        return
    if not github_path_value or not github_env_value:
        raise LlvmInstallError("GITHUB_PATH and GITHUB_ENV must be provided together")

    binary_dir = destination / "bin"
    library_dir = destination / "lib"
    library_path = os.fspath(library_dir)
    inherited = environment.get("LD_LIBRARY_PATH", "")
    if inherited:
        library_path = f"{library_path}{os.pathsep}{inherited}"
    _append_lines(Path(github_path_value), (os.fspath(binary_dir),))
    _append_lines(
        Path(github_env_value),
        (
            f"LLVM_PATH={destination}",
            f"LD_LIBRARY_PATH={library_path}",
        ),
    )


def install_llvm(
    destination: Path, *, environment: Mapping[str, str] | None = None
) -> None:
    """Install and export the pinned toolchain into one empty directory."""
    require_supported_host()
    resolved_destination = destination.resolve()
    prepare_destination(resolved_destination)
    with tempfile.TemporaryDirectory(
        prefix="dear-imgui-llvm-", dir=resolved_destination.parent
    ) as temporary:
        archive = Path(temporary) / LLVM_ARCHIVE_NAME
        download_archive(archive)
        extract_archive(archive, resolved_destination)
    validate_installation(resolved_destination)
    export_github_environment(
        resolved_destination, os.environ if environment is None else environment
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Install the LLVM release pinned by the binding contract"
    )
    parser.add_argument("--destination", required=True, type=Path)
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    options = _build_parser().parse_args(arguments)
    try:
        install_llvm(options.destination)
    except (CommandError, LlvmInstallError, OSError, ValueError) as error:
        print(f"::error::{error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
