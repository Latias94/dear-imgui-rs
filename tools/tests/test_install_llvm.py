import importlib
import io
import os
import subprocess
import sys
import unittest
import urllib.error
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

INSTALL_LLVM = importlib.import_module("install_llvm")


class FakeResponse(io.BytesIO):
    def __init__(self, payload: bytes):
        super().__init__(payload)
        self.headers = {"Content-Length": str(len(payload))}

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()


class InstallLlvmTests(unittest.TestCase):
    def test_binding_contract_pins_the_exact_official_release_asset(self):
        self.assertEqual(INSTALL_LLVM.LLVM_VERSION, "14.0.0")
        self.assertEqual(
            INSTALL_LLVM.LLVM_ARCHIVE_URL,
            "https://github.com/llvm/llvm-project/releases/download/"
            "llvmorg-14.0.0/"
            "clang%2Bllvm-14.0.0-x86_64-linux-gnu-ubuntu-18.04.tar.xz",
        )

    def test_download_retries_the_exact_pinned_asset(self):
        with TemporaryDirectory() as temporary:
            archive = Path(temporary) / INSTALL_LLVM.LLVM_ARCHIVE_NAME
            calls = []

            def opener(request, *, timeout):
                calls.append((request.full_url, timeout))
                if len(calls) == 1:
                    raise urllib.error.URLError("temporary failure")
                return FakeResponse(b"pinned archive")

            INSTALL_LLVM.download_archive(archive, opener=opener, sleep=lambda _: None)

            self.assertEqual(archive.read_bytes(), b"pinned archive")
            self.assertEqual(len(calls), 2)
            self.assertTrue(
                all(url == INSTALL_LLVM.LLVM_ARCHIVE_URL for url, _ in calls)
            )
            self.assertTrue(
                all(
                    timeout == INSTALL_LLVM.DOWNLOAD_TIMEOUT_SECONDS
                    for _, timeout in calls
                )
            )

    def test_install_rejects_a_nonempty_destination_before_downloading(self):
        with TemporaryDirectory() as temporary:
            destination = Path(temporary) / "llvm"
            destination.mkdir()
            (destination / "owned-by-caller").touch()
            with (
                patch.object(INSTALL_LLVM, "require_supported_host"),
                patch.object(INSTALL_LLVM, "download_archive") as download,
                self.assertRaisesRegex(
                    INSTALL_LLVM.LlvmInstallError, "destination is not empty"
                ),
            ):
                INSTALL_LLVM.install_llvm(destination, environment={})

            download.assert_not_called()

    def test_validation_requires_the_exact_llvm_version_and_libclang(self):
        with TemporaryDirectory() as temporary:
            destination = Path(temporary)
            (destination / "bin").mkdir()
            (destination / "lib").mkdir()
            (destination / "bin" / "clang").touch()
            (destination / "bin" / "llvm-config").touch()
            (destination / "lib" / "libclang.so.14").touch()
            result = subprocess.CompletedProcess(
                args=[], returncode=0, stdout="14.0.1\n"
            )
            with (
                patch.object(INSTALL_LLVM, "run", return_value=result),
                self.assertRaisesRegex(
                    INSTALL_LLVM.LlvmInstallError,
                    "expected LLVM 14.0.0",
                ),
            ):
                INSTALL_LLVM.validate_installation(destination)

    def test_exports_the_action_compatible_github_environment(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            destination = root / "llvm"
            destination.mkdir()
            github_path = root / "github-path"
            github_environment = root / "github-environment"
            github_path.write_bytes(b"existing-bin\n")
            github_environment.write_bytes(b"EXISTING=value\n")

            INSTALL_LLVM.export_github_environment(
                destination,
                {
                    "GITHUB_PATH": os.fspath(github_path),
                    "GITHUB_ENV": os.fspath(github_environment),
                    "LD_LIBRARY_PATH": "/existing/lib",
                },
            )

            self.assertEqual(
                github_path.read_bytes(),
                f"existing-bin\n{destination / 'bin'}\n".encode(),
            )
            self.assertEqual(
                github_environment.read_bytes(),
                (
                    "EXISTING=value\n"
                    f"LLVM_PATH={destination}\n"
                    f"LD_LIBRARY_PATH={destination / 'lib'}"
                    f"{os.pathsep}/existing/lib\n"
                ).encode(),
            )


if __name__ == "__main__":
    unittest.main()
