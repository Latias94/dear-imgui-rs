"""Shared subprocess and environment helpers for repository CI tools."""

from __future__ import annotations

import codecs
import locale
import math
import os
import queue
import signal
import subprocess
import sys
import threading
import time
from collections.abc import Iterable, Iterator, Mapping, Sequence
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import IO, TextIO


_WINDOWS_CREATE_SUSPENDED = 0x00000004


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


class ProcessStartError(CommandError):
    """A bounded subprocess could not start after its logs were opened."""

    def __init__(
        self,
        command: Sequence[str],
        output: str,
        stdout_log: Path,
        stderr_log: Path,
    ) -> None:
        self.stdout_log = stdout_log
        self.stderr_log = stderr_log
        super().__init__(command, -1, output)


@dataclass(frozen=True)
class TerminationDiagnostics:
    """Describe how a bounded process tree was reclaimed."""

    strategy: str
    attempted: bool
    graceful: bool | None
    force_kill: bool
    fallback_reason: str | None
    notes: tuple[str, ...]
    errors: tuple[str, ...]


@dataclass(frozen=True)
class BoundedProcessResult:
    """Result and retained evidence from a bounded streaming subprocess."""

    args: tuple[str, ...]
    returncode: int
    timed_out: bool
    duration: float
    stdout_log: Path
    stderr_log: Path
    termination: TerminationDiagnostics
    stream_errors: tuple[str, ...]

    @property
    def log_paths(self) -> tuple[Path, Path]:
        """Return stdout and stderr log paths in stream order."""
        return (self.stdout_log, self.stderr_log)


@dataclass
class _MutableTerminationDiagnostics:
    strategy: str
    attempted: bool = False
    graceful: bool | None = None
    force_kill: bool = False
    fallback_reason: str | None = None
    notes: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    def freeze(self) -> TerminationDiagnostics:
        return TerminationDiagnostics(
            strategy=self.strategy,
            attempted=self.attempted,
            graceful=self.graceful,
            force_kill=self.force_kill,
            fallback_reason=self.fallback_reason,
            notes=tuple(self.notes),
            errors=tuple(self.errors),
        )


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
    timeout: float | None = None,
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
            timeout=timeout,
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


def _process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


class _PosixProcessTree:
    def __init__(self, process: subprocess.Popen[bytes]) -> None:
        self._process = process
        self._process_group = process.pid
        self._grace_deadline: float | None = None
        self._finished = False
        self.diagnostics = _MutableTerminationDiagnostics("posix-process-group")

    @property
    def pending(self) -> bool:
        return self._grace_deadline is not None and not self._finished

    def begin_termination(self, now: float, grace: float) -> None:
        diagnostics = self.diagnostics
        diagnostics.attempted = True
        diagnostics.notes.append(
            f"sent SIGTERM to process group {self._process_group}"
        )
        try:
            os.killpg(self._process_group, signal.SIGTERM)
        except ProcessLookupError:
            diagnostics.graceful = True
            self._finished = True
            return
        except OSError as error:
            diagnostics.errors.append(f"SIGTERM failed: {error}")
            try:
                self._process.terminate()
                diagnostics.notes.append("fell back to terminating the root process")
            except OSError as fallback_error:
                diagnostics.errors.append(
                    f"root-process terminate failed: {fallback_error}"
                )
        self._grace_deadline = now + grace
        self.tick(now)

    def tick(self, now: float) -> None:
        if not self.pending:
            return
        if not _process_group_exists(self._process_group):
            self.diagnostics.graceful = True
            self._finished = True
            return
        if self._grace_deadline is not None and now >= self._grace_deadline:
            self._kill_group("SIGTERM grace expired")

    def root_exited(self, timed_out: bool) -> None:
        if timed_out:
            return
        if _process_group_exists(self._process_group):
            self.diagnostics.attempted = True
            self._kill_group("root exited while descendants remained")
        else:
            self._finished = True

    def close(self) -> None:
        if _process_group_exists(self._process_group):
            self.diagnostics.attempted = True
            self._kill_group("final process-group cleanup")

    def _kill_group(self, reason: str) -> None:
        self.diagnostics.force_kill = True
        self.diagnostics.graceful = False
        self.diagnostics.notes.append(
            f"sent SIGKILL to process group {self._process_group}: {reason}"
        )
        try:
            os.killpg(self._process_group, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError as error:
            self.diagnostics.errors.append(f"SIGKILL failed: {error}")
        self._finished = True


class _WindowsJobError(RuntimeError):
    pass


def _resume_windows_process(process: subprocess.Popen[bytes]) -> None:
    import ctypes
    from ctypes import wintypes

    class ThreadEntry32(ctypes.Structure):
        _fields_ = (
            ("dwSize", wintypes.DWORD),
            ("cntUsage", wintypes.DWORD),
            ("th32ThreadID", wintypes.DWORD),
            ("th32OwnerProcessID", wintypes.DWORD),
            ("tpBasePri", wintypes.LONG),
            ("tpDeltaPri", wintypes.LONG),
            ("dwFlags", wintypes.DWORD),
        )

    snapshot_threads = 0x00000004
    thread_suspend_resume = 0x0002
    no_more_files = 18
    invalid_handle = ctypes.c_void_p(-1).value
    resume_failed = 0xFFFFFFFF

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CreateToolhelp32Snapshot.argtypes = (wintypes.DWORD, wintypes.DWORD)
    kernel32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel32.Thread32First.argtypes = (
        wintypes.HANDLE,
        ctypes.POINTER(ThreadEntry32),
    )
    kernel32.Thread32First.restype = wintypes.BOOL
    kernel32.Thread32Next.argtypes = (
        wintypes.HANDLE,
        ctypes.POINTER(ThreadEntry32),
    )
    kernel32.Thread32Next.restype = wintypes.BOOL
    kernel32.OpenThread.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel32.OpenThread.restype = wintypes.HANDLE
    kernel32.ResumeThread.argtypes = (wintypes.HANDLE,)
    kernel32.ResumeThread.restype = wintypes.DWORD
    kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
    kernel32.CloseHandle.restype = wintypes.BOOL

    snapshot = kernel32.CreateToolhelp32Snapshot(snapshot_threads, 0)
    if snapshot == invalid_handle:
        raise _WindowsJob._last_error("CreateToolhelp32Snapshot")
    resumed_threads = 0
    try:
        entry = ThreadEntry32()
        entry.dwSize = ctypes.sizeof(entry)
        if not kernel32.Thread32First(snapshot, ctypes.byref(entry)):
            raise _WindowsJob._last_error("Thread32First")
        while True:
            if entry.th32OwnerProcessID == process.pid:
                thread = kernel32.OpenThread(
                    thread_suspend_resume,
                    False,
                    entry.th32ThreadID,
                )
                if not thread:
                    raise _WindowsJob._last_error("OpenThread")
                try:
                    if kernel32.ResumeThread(thread) == resume_failed:
                        raise _WindowsJob._last_error("ResumeThread")
                    resumed_threads += 1
                finally:
                    kernel32.CloseHandle(thread)
            entry.dwSize = ctypes.sizeof(entry)
            if kernel32.Thread32Next(snapshot, ctypes.byref(entry)):
                continue
            error_code = ctypes.get_last_error()
            if error_code != no_more_files:
                detail = ctypes.FormatError(error_code).strip()
                raise _WindowsJobError(
                    f"Thread32Next failed with WinError {error_code}: {detail}"
                )
            break
    finally:
        kernel32.CloseHandle(snapshot)
    if resumed_threads == 0:
        raise _WindowsJobError(
            f"no suspended thread found for child process {process.pid}"
        )


class _WindowsJob:
    _KILL_ON_JOB_CLOSE = 0x00002000
    _EXTENDED_LIMIT_INFORMATION = 9
    _PROCESS_TERMINATE = 0x0001
    _PROCESS_SET_QUOTA = 0x0100

    def __init__(self, kernel32: object, handle: int) -> None:
        self._kernel32 = kernel32
        self._handle = handle

    @classmethod
    def create_and_assign(cls, process: subprocess.Popen[bytes]) -> _WindowsJob:
        import ctypes
        from ctypes import wintypes

        class BasicLimitInformation(ctypes.Structure):
            _fields_ = (
                ("PerProcessUserTimeLimit", ctypes.c_longlong),
                ("PerJobUserTimeLimit", ctypes.c_longlong),
                ("LimitFlags", wintypes.DWORD),
                ("MinimumWorkingSetSize", ctypes.c_size_t),
                ("MaximumWorkingSetSize", ctypes.c_size_t),
                ("ActiveProcessLimit", wintypes.DWORD),
                ("Affinity", ctypes.c_size_t),
                ("PriorityClass", wintypes.DWORD),
                ("SchedulingClass", wintypes.DWORD),
            )

        class IoCounters(ctypes.Structure):
            _fields_ = (
                ("ReadOperationCount", ctypes.c_ulonglong),
                ("WriteOperationCount", ctypes.c_ulonglong),
                ("OtherOperationCount", ctypes.c_ulonglong),
                ("ReadTransferCount", ctypes.c_ulonglong),
                ("WriteTransferCount", ctypes.c_ulonglong),
                ("OtherTransferCount", ctypes.c_ulonglong),
            )

        class ExtendedLimitInformation(ctypes.Structure):
            _fields_ = (
                ("BasicLimitInformation", BasicLimitInformation),
                ("IoInfo", IoCounters),
                ("ProcessMemoryLimit", ctypes.c_size_t),
                ("JobMemoryLimit", ctypes.c_size_t),
                ("PeakProcessMemoryUsed", ctypes.c_size_t),
                ("PeakJobMemoryUsed", ctypes.c_size_t),
            )

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel32.CreateJobObjectW.argtypes = (ctypes.c_void_p, wintypes.LPCWSTR)
        kernel32.CreateJobObjectW.restype = wintypes.HANDLE
        kernel32.SetInformationJobObject.argtypes = (
            wintypes.HANDLE,
            ctypes.c_int,
            ctypes.c_void_p,
            wintypes.DWORD,
        )
        kernel32.SetInformationJobObject.restype = wintypes.BOOL
        kernel32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
        kernel32.OpenProcess.restype = wintypes.HANDLE
        kernel32.AssignProcessToJobObject.argtypes = (
            wintypes.HANDLE,
            wintypes.HANDLE,
        )
        kernel32.AssignProcessToJobObject.restype = wintypes.BOOL
        kernel32.TerminateJobObject.argtypes = (wintypes.HANDLE, wintypes.UINT)
        kernel32.TerminateJobObject.restype = wintypes.BOOL
        kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
        kernel32.CloseHandle.restype = wintypes.BOOL

        job_handle = kernel32.CreateJobObjectW(None, None)
        if not job_handle:
            raise cls._last_error("CreateJobObjectW")
        try:
            limits = ExtendedLimitInformation()
            limits.BasicLimitInformation.LimitFlags = cls._KILL_ON_JOB_CLOSE
            if not kernel32.SetInformationJobObject(
                job_handle,
                cls._EXTENDED_LIMIT_INFORMATION,
                ctypes.byref(limits),
                ctypes.sizeof(limits),
            ):
                raise cls._last_error("SetInformationJobObject")

            process_handle = kernel32.OpenProcess(
                cls._PROCESS_TERMINATE | cls._PROCESS_SET_QUOTA,
                False,
                process.pid,
            )
            if not process_handle:
                raise cls._last_error("OpenProcess")
            try:
                if not kernel32.AssignProcessToJobObject(job_handle, process_handle):
                    raise cls._last_error(
                        "AssignProcessToJobObject; the child may inherit a parent job "
                        "that rejects nested assignment"
                    )
            finally:
                kernel32.CloseHandle(process_handle)
        except BaseException:
            kernel32.CloseHandle(job_handle)
            raise
        return cls(kernel32, job_handle)

    @staticmethod
    def _last_error(operation: str) -> _WindowsJobError:
        import ctypes

        error_code = ctypes.get_last_error()
        detail = ctypes.FormatError(error_code).strip()
        return _WindowsJobError(
            f"{operation} failed with WinError {error_code}: {detail}"
        )

    def terminate(self) -> None:
        import ctypes

        if not self._handle:
            return
        if not self._kernel32.TerminateJobObject(self._handle, 124):
            error_code = ctypes.get_last_error()
            detail = ctypes.FormatError(error_code).strip()
            raise _WindowsJobError(
                f"TerminateJobObject failed with WinError {error_code}: {detail}"
            )

    def close(self) -> None:
        if self._handle:
            if not self._kernel32.CloseHandle(self._handle):
                raise self._last_error("CloseHandle(Job Object)")
            self._handle = 0


def _windows_system_taskkill() -> Path:
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetSystemDirectoryW.argtypes = (wintypes.LPWSTR, wintypes.UINT)
    kernel32.GetSystemDirectoryW.restype = wintypes.UINT
    buffer = ctypes.create_unicode_buffer(32768)
    length = kernel32.GetSystemDirectoryW(buffer, len(buffer))
    if length == 0 or length >= len(buffer):
        error_code = ctypes.get_last_error()
        raise _WindowsJobError(
            f"GetSystemDirectoryW failed with WinError {error_code}: "
            f"{ctypes.FormatError(error_code).strip()}"
        )
    executable = Path(buffer.value) / "taskkill.exe"
    if not executable.is_absolute():
        raise _WindowsJobError(f"taskkill path is not absolute: {executable}")
    return executable


class _WindowsProcessTree:
    def __init__(self, process: subprocess.Popen[bytes]) -> None:
        self._process = process
        self._job: _WindowsJob | None = None
        self._closed = False
        self.diagnostics = _MutableTerminationDiagnostics("windows-job-object")
        try:
            self._job = _WindowsJob.create_and_assign(process)
            self.diagnostics.notes.append(
                "assigned child to a KILL_ON_JOB_CLOSE Job Object"
            )
        except _WindowsJobError as error:
            self.diagnostics.strategy = "windows-taskkill"
            self.diagnostics.fallback_reason = str(error)
            self.diagnostics.notes.append(
                "Job Object assignment unavailable; using System32 taskkill fallback"
            )

    @property
    def pending(self) -> bool:
        return False

    def begin_termination(self, now: float, grace: float) -> None:
        del now, grace
        diagnostics = self.diagnostics
        diagnostics.attempted = True
        diagnostics.graceful = False
        diagnostics.force_kill = True
        if self._job is not None:
            try:
                self._job.terminate()
                diagnostics.notes.append(
                    "TerminateJobObject terminated the process tree"
                )
            except _WindowsJobError as error:
                diagnostics.errors.append(str(error))
                self._taskkill()
        else:
            self._taskkill()

    def tick(self, now: float) -> None:
        del now

    def root_exited(self, timed_out: bool) -> None:
        del timed_out
        self._close_job()

    def close(self) -> None:
        if self._job is None and self._process.poll() is None:
            self.diagnostics.attempted = True
            self.diagnostics.graceful = False
            self.diagnostics.force_kill = True
            self.diagnostics.notes.append(
                "final cleanup invoked the System32 taskkill fallback"
            )
            self._taskkill()
        self._close_job()

    def _taskkill(self) -> None:
        diagnostics = self.diagnostics
        try:
            executable = _windows_system_taskkill()
            result = subprocess.run(
                (
                    os.fspath(executable),
                    "/PID",
                    str(self._process.pid),
                    "/T",
                    "/F",
                ),
                check=False,
                shell=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding=locale.getpreferredencoding(False),
                errors="replace",
                timeout=30.0,
            )
        except (OSError, subprocess.TimeoutExpired, _WindowsJobError) as error:
            diagnostics.errors.append(f"taskkill fallback failed: {error}")
            return
        diagnostics.notes.append(
            f"{executable} /PID {self._process.pid} /T /F returned "
            f"{result.returncode}"
        )
        output = "\n".join(
            part.strip() for part in (result.stdout, result.stderr) if part.strip()
        )
        if output:
            diagnostics.notes.append(f"taskkill output: {output}")
        if result.returncode != 0:
            diagnostics.errors.append(
                f"taskkill fallback returned exit code {result.returncode}"
            )

    def _close_job(self) -> None:
        if self._closed:
            return
        self._closed = True
        if self._job is not None:
            try:
                self._job.close()
            except _WindowsJobError as error:
                self.diagnostics.errors.append(str(error))


def _read_process_stream(
    name: str,
    stream: IO[bytes],
    events: queue.Queue[tuple[str, str | BaseException | None]],
) -> None:
    decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
    try:
        read = getattr(stream, "read1", stream.read)
        while chunk := read(65536):
            if decoded := decoder.decode(chunk):
                events.put((name, decoded))
    except BaseException as error:
        events.put((name, error))
    finally:
        try:
            if decoded := decoder.decode(b"", final=True):
                events.put((name, decoded))
        except UnicodeError as error:
            events.put((name, error))
        events.put((name, None))


def _start_managed_process(
    command: tuple[str, ...],
    *,
    cwd: Path | None,
    env: Mapping[str, str] | None,
) -> tuple[
    subprocess.Popen[bytes],
    _PosixProcessTree | _WindowsProcessTree,
]:
    creationflags = _WINDOWS_CREATE_SUSPENDED if os.name == "nt" else 0
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        shell=False,
        text=False,
        start_new_session=os.name != "nt",
        creationflags=creationflags,
    )
    process_tree: _PosixProcessTree | _WindowsProcessTree | None = None
    try:
        process_tree = (
            _WindowsProcessTree(process)
            if os.name == "nt"
            else _PosixProcessTree(process)
        )
        if os.name == "nt":
            _resume_windows_process(process)
            process_tree.diagnostics.notes.append(
                "resumed child after process-tree ownership was established"
            )
    except BaseException:
        if process_tree is not None:
            process_tree.close()
        if process.poll() is None:
            try:
                process.kill()
            except OSError:
                pass
            try:
                process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                pass
        for pipe in (process.stdout, process.stderr):
            if pipe is not None:
                try:
                    pipe.close()
                except OSError:
                    pass
        raise
    return process, process_tree


def _validate_duration(name: str, value: float, *, allow_zero: bool) -> float:
    duration = float(value)
    if (
        not math.isfinite(duration)
        or duration < 0
        or (duration == 0 and not allow_zero)
    ):
        comparator = "non-negative" if allow_zero else "positive"
        raise ValueError(f"{name} must be a finite {comparator} duration")
    return duration


def _same_path(left: Path, right: Path) -> bool:
    return os.path.normcase(os.path.abspath(left)) == os.path.normcase(
        os.path.abspath(right)
    )


def _write_background_events(
    events: queue.Queue[tuple[str, str | BaseException | None]],
    log_streams: Mapping[str, TextIO],
    output_streams: Mapping[str, TextIO],
    stream_errors: list[str],
) -> None:
    eof_streams: set[str] = set()
    while eof_streams != {"stdout", "stderr"}:
        name, payload = events.get()
        if payload is None:
            eof_streams.add(name)
            continue
        if isinstance(payload, BaseException):
            stream_errors.append(f"{name} reader failed: {payload}")
            continue
        try:
            log_streams[name].write(payload)
            log_streams[name].flush()
        except (OSError, UnicodeError, ValueError) as error:
            stream_errors.append(f"{name} log write failed: {error}")
        try:
            output_streams[name].write(payload)
            output_streams[name].flush()
        except (OSError, UnicodeError, ValueError) as error:
            stream_errors.append(f"{name} console write failed: {error}")


class ManagedBackgroundProcess:
    """Own a background process tree and its live stdout/stderr evidence."""

    def __init__(
        self,
        command: Sequence[str | Path],
        *,
        stdout_log: Path,
        stderr_log: Path,
        cwd: Path | None = None,
        env: Mapping[str, str] | None = None,
        termination_grace: float = 5.0,
        console_stdout: TextIO | None = None,
        console_stderr: TextIO | None = None,
    ) -> None:
        self.args = tuple(os.fspath(argument) for argument in command)
        if not self.args:
            raise ValueError("command must not be empty")
        self.stdout_log = Path(stdout_log)
        self.stderr_log = Path(stderr_log)
        if _same_path(self.stdout_log, self.stderr_log):
            raise ValueError("stdout_log and stderr_log must be different paths")
        self.termination_grace = _validate_duration(
            "termination_grace", termination_grace, allow_zero=True
        )
        self.stdout_log.parent.mkdir(parents=True, exist_ok=True)
        self.stderr_log.parent.mkdir(parents=True, exist_ok=True)
        self._stdout_file = self.stdout_log.open(
            "w", encoding="utf-8", newline=""
        )
        try:
            self._stderr_file = self.stderr_log.open(
                "w", encoding="utf-8", newline=""
            )
        except BaseException:
            self._stdout_file.close()
            raise
        self._stream_errors: list[str] = []
        self._reader_threads: list[threading.Thread] = []
        self._writer_thread: threading.Thread | None = None
        self._closed = False
        self._termination: TerminationDiagnostics | None = None
        self._output_streams: dict[str, TextIO] = {
            "stdout": sys.stdout if console_stdout is None else console_stdout,
            "stderr": sys.stderr if console_stderr is None else console_stderr,
        }
        try:
            self._process, self._process_tree = _start_managed_process(
                self.args,
                cwd=cwd,
                env=env,
            )
        except (OSError, _WindowsJobError) as error:
            rendered = subprocess.list2cmdline(self.args)
            detail = f"could not run {rendered}: {error}"
            self._stderr_file.write(f"{detail}\n")
            self._stderr_file.flush()
            try:
                self._output_streams["stderr"].write(f"{detail}\n")
                self._output_streams["stderr"].flush()
            except (OSError, UnicodeError, ValueError):
                pass
            self._stdout_file.close()
            self._stderr_file.close()
            raise ProcessStartError(
                self.args,
                detail,
                self.stdout_log,
                self.stderr_log,
            ) from error
        except BaseException:
            self._stdout_file.close()
            self._stderr_file.close()
            raise

        try:
            self._start_stream_pumps()
        except BaseException:
            self.close()
            raise

    def _start_stream_pumps(self) -> None:
        events: queue.Queue[tuple[str, str | BaseException | None]] = queue.Queue()
        assert self._process.stdout is not None
        assert self._process.stderr is not None
        for name, pipe in (
            ("stdout", self._process.stdout),
            ("stderr", self._process.stderr),
        ):
            thread = threading.Thread(
                target=_read_process_stream,
                args=(name, pipe, events),
                name=f"managed-background-{name}-{self._process.pid}",
                daemon=True,
            )
            self._reader_threads.append(thread)
            thread.start()
        self._writer_thread = threading.Thread(
            target=_write_background_events,
            args=(
                events,
                {
                    "stdout": self._stdout_file,
                    "stderr": self._stderr_file,
                },
                self._output_streams,
                self._stream_errors,
            ),
            name=f"managed-background-writer-{self._process.pid}",
            daemon=True,
        )
        self._writer_thread.start()

    @property
    def pid(self) -> int:
        return self._process.pid

    @property
    def returncode(self) -> int | None:
        return self._process.poll()

    @property
    def termination(self) -> TerminationDiagnostics | None:
        return self._termination

    @property
    def stream_errors(self) -> tuple[str, ...]:
        return tuple(self._stream_errors)

    def poll(self) -> int | None:
        """Return the background process status without changing ownership."""
        return self._process.poll()

    def close(self) -> TerminationDiagnostics:
        """Terminate the owned tree, drain both logs, and release every handle."""
        if self._closed:
            if self._termination is None:
                self._termination = self._process_tree.diagnostics.freeze()
            return self._termination
        process = self._process
        process_tree = self._process_tree
        now = time.monotonic()
        root_exit_observed = process.poll() is not None
        if root_exit_observed:
            process_tree.root_exited(False)
        else:
            process_tree.begin_termination(now, self.termination_grace)
        hard_deadline = now + self.termination_grace + 10.0
        while process.poll() is None or process_tree.pending:
            now = time.monotonic()
            process_tree.tick(now)
            if process.poll() is not None and not root_exit_observed:
                root_exit_observed = True
                process_tree.root_exited(True)
            if now >= hard_deadline:
                process_tree.diagnostics.errors.append(
                    "background tree termination exceeded its hard deadline; "
                    "killed root process"
                )
                try:
                    process.kill()
                except OSError as error:
                    process_tree.diagnostics.errors.append(
                        f"root-process kill failed: {error}"
                    )
                break
            time.sleep(0.02)
        process_tree.close()
        if process.poll() is None:
            try:
                process.kill()
            except OSError as error:
                process_tree.diagnostics.errors.append(
                    f"final root-process kill failed: {error}"
                )
        try:
            process.wait(timeout=5.0)
        except (OSError, subprocess.TimeoutExpired) as error:
            process_tree.diagnostics.errors.append(
                f"root process did not exit after final cleanup: {error}"
            )
        for pipe in (process.stdout, process.stderr):
            if pipe is not None:
                try:
                    pipe.close()
                except OSError as error:
                    self._stream_errors.append(f"pipe close failed: {error}")
        for thread in self._reader_threads:
            if thread.ident is None:
                continue
            thread.join(timeout=5.0)
            if thread.is_alive():
                self._stream_errors.append(
                    f"reader thread did not exit: {thread.name}"
                )
        if self._writer_thread is not None:
            if self._writer_thread.ident is not None:
                self._writer_thread.join(timeout=5.0)
                if self._writer_thread.is_alive():
                    self._stream_errors.append(
                        "background writer thread did not exit"
                    )
        self._termination = process_tree.diagnostics.freeze()
        for name, output in (
            ("stdout", self._stdout_file),
            ("stderr", self._stderr_file),
        ):
            try:
                output.flush()
            except (OSError, ValueError) as error:
                self._stream_errors.append(f"{name} log flush failed: {error}")
            try:
                output.close()
            except OSError as error:
                self._stream_errors.append(f"{name} log close failed: {error}")
        self._closed = True
        return self._termination

    def __enter__(self) -> ManagedBackgroundProcess:
        return self

    def __exit__(self, exc_type: object, exc_value: object, traceback: object) -> None:
        del exc_type, exc_value, traceback
        self.close()


def managed_background(
    command: Sequence[str | Path],
    *,
    stdout_log: Path,
    stderr_log: Path,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    termination_grace: float = 5.0,
    console_stdout: TextIO | None = None,
    console_stderr: TextIO | None = None,
) -> ManagedBackgroundProcess:
    """Start an owned background tree for use as a context manager."""
    return ManagedBackgroundProcess(
        command,
        stdout_log=stdout_log,
        stderr_log=stderr_log,
        cwd=cwd,
        env=env,
        termination_grace=termination_grace,
        console_stdout=console_stdout,
        console_stderr=console_stderr,
    )


def run_bounded(
    command: Sequence[str | Path],
    *,
    timeout: float,
    stdout_log: Path,
    stderr_log: Path,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    termination_grace: float = 5.0,
    console_stdout: TextIO | None = None,
    console_stderr: TextIO | None = None,
) -> BoundedProcessResult:
    """Run a command with streaming logs, a deadline, and process-tree cleanup.

    Nonzero child exits are returned to the caller. Failure to create the child raises
    ``ProcessStartError`` after both requested log files have been created and flushed.
    """
    rendered_command = tuple(os.fspath(argument) for argument in command)
    if not rendered_command:
        raise ValueError("command must not be empty")
    timeout = _validate_duration("timeout", timeout, allow_zero=False)
    termination_grace = _validate_duration(
        "termination_grace", termination_grace, allow_zero=True
    )
    stdout_log = Path(stdout_log)
    stderr_log = Path(stderr_log)
    if _same_path(stdout_log, stderr_log):
        raise ValueError("stdout_log and stderr_log must be different paths")
    stdout_log.parent.mkdir(parents=True, exist_ok=True)
    stderr_log.parent.mkdir(parents=True, exist_ok=True)

    output_streams: dict[str, TextIO] = {
        "stdout": sys.stdout if console_stdout is None else console_stdout,
        "stderr": sys.stderr if console_stderr is None else console_stderr,
    }
    started = time.monotonic()
    process: subprocess.Popen[bytes] | None = None
    process_tree: _PosixProcessTree | _WindowsProcessTree | None = None
    reader_threads: list[threading.Thread] = []
    stream_errors: list[str] = []
    timed_out = False
    root_exit_observed = False
    eof_streams: set[str] = set()

    with (
        stdout_log.open("w", encoding="utf-8", newline="") as stdout_file,
        stderr_log.open("w", encoding="utf-8", newline="") as stderr_file,
    ):
        log_streams = {"stdout": stdout_file, "stderr": stderr_file}
        try:
            try:
                process, process_tree = _start_managed_process(
                    rendered_command,
                    cwd=cwd,
                    env=env,
                )
            except (OSError, _WindowsJobError) as error:
                rendered = subprocess.list2cmdline(rendered_command)
                detail = f"could not run {rendered}: {error}"
                stderr_file.write(f"{detail}\n")
                stderr_file.flush()
                try:
                    output_streams["stderr"].write(f"{detail}\n")
                    output_streams["stderr"].flush()
                except (OSError, UnicodeError, ValueError):
                    pass
                raise ProcessStartError(
                    rendered_command, detail, stdout_log, stderr_log
                ) from error

            events: queue.Queue[tuple[str, str | BaseException | None]] = queue.Queue()
            assert process.stdout is not None
            assert process.stderr is not None
            for name, pipe in (("stdout", process.stdout), ("stderr", process.stderr)):
                thread = threading.Thread(
                    target=_read_process_stream,
                    args=(name, pipe, events),
                    name=f"bounded-process-{name}-{process.pid}",
                    daemon=True,
                )
                reader_threads.append(thread)
                thread.start()

            deadline = started + timeout
            termination_hard_deadline: float | None = None
            pipe_drain_deadline: float | None = None
            while True:
                now = time.monotonic()
                returncode = process.poll()
                if returncode is None and not timed_out and now >= deadline:
                    timed_out = True
                    termination_hard_deadline = now + termination_grace + 10.0
                    process_tree.begin_termination(now, termination_grace)
                process_tree.tick(now)

                if returncode is not None and not root_exit_observed:
                    root_exit_observed = True
                    process_tree.root_exited(timed_out)
                    pipe_drain_deadline = now + 5.0

                if (
                    termination_hard_deadline is not None
                    and returncode is None
                    and now >= termination_hard_deadline
                ):
                    process_tree.diagnostics.errors.append(
                        "tree termination exceeded its hard deadline; killed root "
                        "process"
                    )
                    try:
                        process.kill()
                    except OSError as error:
                        process_tree.diagnostics.errors.append(
                            f"root-process kill failed: {error}"
                        )
                    termination_hard_deadline = None

                if (
                    root_exit_observed
                    and eof_streams == {"stdout", "stderr"}
                    and not process_tree.pending
                ):
                    break
                if (
                    root_exit_observed
                    and pipe_drain_deadline is not None
                    and now >= pipe_drain_deadline
                    and not process_tree.pending
                ):
                    missing = sorted({"stdout", "stderr"} - eof_streams)
                    stream_errors.append(
                        "pipes did not reach EOF after tree cleanup: "
                        + ", ".join(missing)
                    )
                    break

                wait_for = 0.05
                if not timed_out:
                    wait_for = min(wait_for, max(0.0, deadline - now))
                try:
                    name, payload = events.get(timeout=wait_for)
                except queue.Empty:
                    continue
                if payload is None:
                    eof_streams.add(name)
                    continue
                if isinstance(payload, BaseException):
                    stream_errors.append(f"{name} reader failed: {payload}")
                    continue
                try:
                    log_streams[name].write(payload)
                    log_streams[name].flush()
                except (OSError, UnicodeError, ValueError) as error:
                    stream_errors.append(f"{name} log write failed: {error}")
                try:
                    output_streams[name].write(payload)
                    output_streams[name].flush()
                except (OSError, UnicodeError, ValueError) as error:
                    stream_errors.append(f"{name} console write failed: {error}")

            returncode = process.wait(timeout=5.0)
        finally:
            if process_tree is not None:
                process_tree.close()
            if process is not None:
                if process.poll() is None:
                    try:
                        process.kill()
                    except OSError:
                        pass
                    try:
                        process.wait(timeout=5.0)
                    except subprocess.TimeoutExpired:
                        pass
                for pipe in (process.stdout, process.stderr):
                    if pipe is not None:
                        try:
                            pipe.close()
                        except OSError:
                            pass
            for thread in reader_threads:
                thread.join(timeout=1.0)
                if thread.is_alive():
                    stream_errors.append(
                        f"reader thread did not exit: {thread.name}"
                    )
            stdout_file.flush()
            stderr_file.flush()

    assert process_tree is not None
    return BoundedProcessResult(
        args=rendered_command,
        returncode=returncode,
        timed_out=timed_out,
        duration=time.monotonic() - started,
        stdout_log=stdout_log,
        stderr_log=stderr_log,
        termination=process_tree.diagnostics.freeze(),
        stream_errors=tuple(stream_errors),
    )


@contextmanager
def github_group(label: str) -> Iterator[None]:
    """Keep GitHub Actions logs grouped while still closing failed groups."""
    print(f"::group::{label}", flush=True)
    try:
        yield
    finally:
        print("::endgroup::", flush=True)
