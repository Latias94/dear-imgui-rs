"""Integration tests for the bounded cross-platform subprocess helper."""

from __future__ import annotations

import argparse
import io
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


def _run_grandchild(sentinel: Path | None, ignore_term: bool) -> int:
    if ignore_term and os.name != "nt":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
    if sentinel is None:
        while True:
            time.sleep(1.0)
    with sentinel.open("a", encoding="utf-8", newline="") as heartbeat:
        while True:
            heartbeat.write("tick\n")
            heartbeat.flush()
            time.sleep(0.05)


def _run_tree_child(sentinel: Path | None, ignore_term: bool) -> int:
    if ignore_term and os.name != "nt":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
    print("TREE_START", flush=True)
    grandchild_command = [sys.executable, __file__, "--child-mode", "grandchild"]
    if sentinel is not None:
        grandchild_command.extend(("--sentinel", os.fspath(sentinel)))
    if ignore_term:
        grandchild_command.append("--ignore-term")
    grandchild = subprocess.Popen(grandchild_command, shell=False)
    print(f"SPAWNED_GRANDCHILD_PID={grandchild.pid}", flush=True)
    if ignore_term and sentinel is not None and os.name != "nt":
        ready_deadline = time.monotonic() + 10.0
        while not sentinel.exists():
            if time.monotonic() >= ready_deadline:
                print(
                    "grandchild did not create its heartbeat",
                    file=sys.stderr,
                    flush=True,
                )
                return 2
            time.sleep(0.01)
    print(f"CHILD_PID={os.getpid()}", flush=True)
    print(f"GRANDCHILD_PID={grandchild.pid}", flush=True)
    print("tree stdout tail", end="", flush=True)
    print("tree stderr tail", end="", file=sys.stderr, flush=True)
    while True:
        time.sleep(1.0)


def _child_main(arguments: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--child-mode",
        choices=("normal", "hang", "tree", "grandchild", "utf8-chunks"),
        required=True,
    )
    parser.add_argument("--sentinel", type=Path)
    parser.add_argument("--ignore-term", action="store_true")
    parser.add_argument("--exit-code", type=int, default=0)
    options = parser.parse_args(arguments)
    if options.child_mode == "normal":
        print("normal stdout", flush=True)
        print("normal stderr", file=sys.stderr, flush=True)
        return options.exit_code
    if options.child_mode == "utf8-chunks":
        os.write(sys.stdout.fileno(), b"\xe2")
        time.sleep(0.05)
        os.write(sys.stdout.fileno(), b"\x82\xac\n")
        return 0
    if options.child_mode == "hang":
        print("stdout without newline", end="", flush=True)
        print("stderr without newline", end="", file=sys.stderr, flush=True)
        while True:
            time.sleep(1.0)
    if options.child_mode == "grandchild":
        return _run_grandchild(options.sentinel, options.ignore_term)
    return _run_tree_child(options.sentinel, options.ignore_term)


if "--child-mode" in sys.argv:
    raise SystemExit(_child_main(sys.argv[1:]))


ROOT = Path(__file__).resolve().parents[2]
CI_DIR = ROOT / "tools" / "ci"
if os.fspath(CI_DIR) not in sys.path:
    sys.path.insert(0, os.fspath(CI_DIR))

import _process as PROCESS  # noqa: E402


class BoundedProcessTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_child(
        self,
        mode: str,
        *,
        timeout: float = 5.0,
        grace: float = 0.2,
        script: Path | None = None,
        extra: tuple[str, ...] = (),
        label: str = "child",
    ) -> tuple[PROCESS.BoundedProcessResult, io.StringIO, io.StringIO]:
        stdout_console = io.StringIO()
        stderr_console = io.StringIO()
        result = PROCESS.run_bounded(
            (
                sys.executable,
                script or Path(__file__),
                "--child-mode",
                mode,
                *extra,
            ),
            timeout=timeout,
            termination_grace=grace,
            stdout_log=self.root / f"{label}.stdout.log",
            stderr_log=self.root / f"{label}.stderr.log",
            console_stdout=stdout_console,
            console_stderr=stderr_console,
        )
        return result, stdout_console, stderr_console

    def run_tree(
        self,
        *,
        ignore_term: bool = False,
        label: str = "tree",
    ) -> tuple[PROCESS.BoundedProcessResult, tuple[int, int]]:
        sentinel = self.root / f"{label}.heartbeat"
        extra = ("--sentinel", os.fspath(sentinel))
        if ignore_term:
            extra += ("--ignore-term",)
        result, _, _ = self.run_child(
            "tree", timeout=5.0, grace=0.2, extra=extra, label=label
        )
        self.assertTrue(result.timed_out)
        stdout = result.stdout_log.read_text(encoding="utf-8")
        child_match = re.search(r"CHILD_PID=(\d+)", stdout)
        grandchild_match = re.search(r"GRANDCHILD_PID=(\d+)", stdout)
        self.assertIsNotNone(child_match)
        self.assertIsNotNone(grandchild_match)
        assert child_match is not None
        assert grandchild_match is not None
        pids = (int(child_match.group(1)), int(grandchild_match.group(1)))
        self.assertIn("tree stdout tail", stdout)
        self.assertIn(
            "tree stderr tail", result.stderr_log.read_text(encoding="utf-8")
        )
        for pid in pids:
            self.assertTrue(
                self.wait_for_process_exit(pid),
                f"process {pid} remained alive after run_bounded returned",
            )
        if sentinel.exists():
            heartbeat = sentinel.read_bytes()
            time.sleep(0.2)
            self.assertEqual(
                sentinel.read_bytes(),
                heartbeat,
                "grandchild continued writing after run_bounded returned",
            )
        return result, pids

    @staticmethod
    def wait_for_process_exit(pid: int) -> bool:
        deadline = time.monotonic() + 5.0
        while time.monotonic() < deadline:
            if not BoundedProcessTests.process_is_alive(pid):
                return True
            time.sleep(0.05)
        return not BoundedProcessTests.process_is_alive(pid)

    @staticmethod
    def wait_for_tree_pids(
        background: PROCESS.ManagedBackgroundProcess,
        output: io.StringIO,
    ) -> tuple[int, int]:
        deadline = time.monotonic() + 20.0
        while time.monotonic() < deadline:
            text = output.getvalue()
            child_match = re.search(r"CHILD_PID=(\d+)", text)
            grandchild_match = re.search(r"GRANDCHILD_PID=(\d+)", text)
            if child_match is not None and grandchild_match is not None:
                return (
                    int(child_match.group(1)),
                    int(grandchild_match.group(1)),
                )
            if background.poll() is not None:
                break
            time.sleep(0.02)
        raise AssertionError(
            "background process did not publish child and grandchild PIDs"
        )

    @staticmethod
    def process_is_alive(pid: int) -> bool:
        if os.name != "nt":
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                return False
            return True

        import ctypes
        from ctypes import wintypes

        query_limited_information = 0x1000
        still_active = 259
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
        kernel32.OpenProcess.restype = wintypes.HANDLE
        kernel32.GetExitCodeProcess.argtypes = (
            wintypes.HANDLE,
            ctypes.POINTER(wintypes.DWORD),
        )
        kernel32.GetExitCodeProcess.restype = wintypes.BOOL
        kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
        kernel32.CloseHandle.restype = wintypes.BOOL
        handle = kernel32.OpenProcess(query_limited_information, False, pid)
        if not handle:
            return False
        try:
            exit_code = wintypes.DWORD()
            if not kernel32.GetExitCodeProcess(handle, ctypes.byref(exit_code)):
                return False
            return exit_code.value == still_active
        finally:
            kernel32.CloseHandle(handle)

    def test_normal_exit_streams_to_separate_logs_and_consoles(self) -> None:
        result, stdout_console, stderr_console = self.run_child("normal")

        self.assertEqual(result.returncode, 0)
        self.assertFalse(result.timed_out)
        self.assertGreaterEqual(result.duration, 0.0)
        self.assertEqual(result.log_paths, (result.stdout_log, result.stderr_log))
        self.assertEqual(
            result.stdout_log.read_text(encoding="utf-8"), "normal stdout\n"
        )
        self.assertEqual(
            result.stderr_log.read_text(encoding="utf-8"), "normal stderr\n"
        )
        self.assertEqual(
            stdout_console.getvalue().replace("\r\n", "\n"), "normal stdout\n"
        )
        self.assertEqual(
            stderr_console.getvalue().replace("\r\n", "\n"), "normal stderr\n"
        )
        self.assertFalse(result.termination.attempted)
        self.assertEqual(result.stream_errors, ())

    def test_start_failure_retains_logs_and_raises_typed_error(self) -> None:
        stdout_log = self.root / "missing.stdout.log"
        stderr_log = self.root / "missing.stderr.log"
        stderr_console = io.StringIO()

        with self.assertRaises(PROCESS.ProcessStartError) as raised:
            PROCESS.run_bounded(
                (self.root / "missing executable",),
                timeout=1.0,
                stdout_log=stdout_log,
                stderr_log=stderr_log,
                console_stdout=io.StringIO(),
                console_stderr=stderr_console,
            )

        self.assertEqual(raised.exception.stdout_log, stdout_log)
        self.assertEqual(raised.exception.stderr_log, stderr_log)
        self.assertEqual(stdout_log.read_text(encoding="utf-8"), "")
        self.assertIn("could not run", stderr_log.read_text(encoding="utf-8"))
        self.assertIn("could not run", stderr_console.getvalue())

    def test_nonzero_exit_is_returned_for_caller_classification(self) -> None:
        result, _, _ = self.run_child(
            "normal", extra=("--exit-code", "17"), label="nonzero"
        )

        self.assertEqual(result.returncode, 17)
        self.assertFalse(result.timed_out)

    def test_script_and_log_paths_with_spaces(self) -> None:
        spaced = self.root / "directory with spaces"
        spaced.mkdir()
        script = spaced / "child fixture.py"
        shutil.copyfile(__file__, script)

        result, _, _ = self.run_child("normal", script=script, label="logs with spaces")

        self.assertEqual(result.returncode, 0)
        self.assertEqual(
            result.stdout_log.read_text(encoding="utf-8"), "normal stdout\n"
        )

    def test_utf8_split_across_pipe_chunks_is_not_corrupted(self) -> None:
        result, stdout_console, _ = self.run_child("utf8-chunks", label="utf8")

        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout_log.read_text(encoding="utf-8"), "\u20ac\n")
        self.assertEqual(stdout_console.getvalue().replace("\r\n", "\n"), "\u20ac\n")

    def test_timeout_retains_unterminated_tail_output(self) -> None:
        result, stdout_console, stderr_console = self.run_child(
            "hang", timeout=15.0, grace=0.15, label="tail"
        )

        self.assertTrue(result.timed_out)
        self.assertTrue(result.termination.attempted)
        self.assertIn(
            "stdout without newline", result.stdout_log.read_text(encoding="utf-8")
        )
        self.assertIn(
            "stderr without newline", result.stderr_log.read_text(encoding="utf-8")
        )
        self.assertIn("stdout without newline", stdout_console.getvalue())
        self.assertIn("stderr without newline", stderr_console.getvalue())

    def test_timeout_terminates_grandchild(self) -> None:
        self.run_tree()

    def test_managed_background_context_tees_and_reclaims_tree(self) -> None:
        class ExpectedBodyError(RuntimeError):
            pass

        sentinel = self.root / "background.heartbeat"
        stdout_console = io.StringIO()
        stderr_console = io.StringIO()
        background = PROCESS.managed_background(
            (
                sys.executable,
                Path(__file__),
                "--child-mode",
                "tree",
                "--sentinel",
                sentinel,
            ),
            stdout_log=self.root / "background.stdout.log",
            stderr_log=self.root / "background.stderr.log",
            termination_grace=0.2,
            console_stdout=stdout_console,
            console_stderr=stderr_console,
        )

        with self.assertRaises(ExpectedBodyError):
            with background:
                pids = self.wait_for_tree_pids(background, stdout_console)
                raise ExpectedBodyError("exercise exceptional context exit")

        self.assertIsNotNone(background.termination)
        self.assertTrue(background.termination.attempted)
        self.assertEqual(background.stream_errors, ())
        self.assertIn(
            "tree stdout tail",
            background.stdout_log.read_text(encoding="utf-8"),
        )
        self.assertIn(
            "tree stderr tail",
            background.stderr_log.read_text(encoding="utf-8"),
        )
        for pid in pids:
            self.assertTrue(self.wait_for_process_exit(pid))
        self.assertIs(background.close(), background.termination)

    @unittest.skipIf(os.name == "nt", "POSIX-only process-group behavior")
    def test_posix_ignored_sigterm_escalates_to_sigkill(self) -> None:
        result, _ = self.run_tree(ignore_term=True, label="ignore-term")

        self.assertEqual(result.termination.strategy, "posix-process-group")
        self.assertFalse(result.termination.graceful)
        self.assertTrue(result.termination.force_kill)
        self.assertTrue(any("SIGKILL" in note for note in result.termination.notes))

    @unittest.skipUnless(os.name == "nt", "Windows-only Job Object behavior")
    def test_windows_uses_job_object_when_assignment_is_available(self) -> None:
        result, _ = self.run_tree(label="job-object")
        if result.termination.strategy != "windows-job-object":
            self.skipTest(
                result.termination.fallback_reason or "Job Object unavailable"
            )

        self.assertIsNone(result.termination.fallback_reason)
        self.assertTrue(
            any("TerminateJobObject" in note for note in result.termination.notes)
        )

    @unittest.skipUnless(os.name == "nt", "Windows-only taskkill fallback")
    def test_windows_records_and_uses_taskkill_fallback(self) -> None:
        assignment_error = PROCESS._WindowsJobError("forced parent-job rejection")
        with patch.object(
            PROCESS._WindowsJob,
            "create_and_assign",
            side_effect=assignment_error,
        ):
            result, _ = self.run_tree(label="taskkill-fallback")

        self.assertEqual(result.termination.strategy, "windows-taskkill")
        self.assertEqual(
            result.termination.fallback_reason, "forced parent-job rejection"
        )
        self.assertTrue(
            any("taskkill.exe /PID" in note for note in result.termination.notes)
        )


if __name__ == "__main__":
    unittest.main()
