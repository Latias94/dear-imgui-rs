"""Linux display and software-renderer infrastructure for runtime gates."""

from __future__ import annotations

import platform
import re
import shutil
import sys
import time
from collections.abc import Mapping, Sequence
from pathlib import Path

from _process import BoundedProcessResult, run_bounded
from _runtime_gate_common import GateCategory, RuntimeContractError, _check_stage


def _find_lavapipe_icd() -> Path:
    roots = (
        Path("/usr/share/vulkan/icd.d"),
        Path("/usr/local/share/vulkan/icd.d"),
    )
    candidates = sorted(
        candidate
        for root in roots
        if root.is_dir()
        for pattern in ("lvp_icd*.json", "*lavapipe*.json")
        for candidate in root.glob(pattern)
    )
    if not candidates:
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "Mesa Lavapipe ICD was not found",
        )
    architecture = platform.machine().lower()
    aliases = {
        "amd64": ("x86_64", "amd64"),
        "x86_64": ("x86_64", "amd64"),
        "arm64": ("aarch64", "arm64"),
        "aarch64": ("aarch64", "arm64"),
    }.get(architecture, (architecture,))
    for alias in aliases:
        for candidate in candidates:
            if alias in candidate.name.lower():
                return candidate
    if len(candidates) == 1:
        return candidates[0]
    raise RuntimeContractError(
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        f"no Lavapipe ICD matches host architecture {architecture!r}",
    )


def _require_linux_tools(
    tool_names: Sequence[str],
    *,
    platform_error: str,
) -> dict[str, Path]:
    if not sys.platform.startswith("linux"):
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            platform_error,
        )
    tools: dict[str, Path] = {}
    for name in tool_names:
        executable = shutil.which(name)
        if executable is None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"required runtime program is unavailable: {name}",
            )
        tools[name] = Path(executable)
    return tools


def _require_linux_runtime_tools() -> dict[str, Path]:
    return _require_linux_tools(
        ("Xvfb", "openbox", "xdpyinfo", "xprop", "vulkaninfo", "dpkg-query"),
        platform_error="multi-viewport-smoke requires Linux, Xvfb, and Mesa Lavapipe",
    )


def _require_linux_sdl3_glow_tools() -> dict[str, Path]:
    return _require_linux_tools(
        ("Xvfb", "openbox", "xdpyinfo", "xprop", "glxinfo", "dpkg-query"),
        platform_error=(
            "sdl3-glow-multi-viewport-smoke requires Linux, Xvfb, and Mesa llvmpipe"
        ),
    )


def _wait_for_xvfb(process: object, display: str, timeout: float = 10.0) -> None:
    match = re.fullmatch(r":([0-9]+)", display)
    if match is None:
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"invalid Xvfb display: {display!r}",
        )
    socket = Path("/tmp/.X11-unix") / f"X{match.group(1)}"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        returncode = getattr(process, "poll")()
        if returncode is not None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"Xvfb exited during startup with status {returncode}",
            )
        if socket.exists():
            return
        time.sleep(0.05)
    raise RuntimeContractError(
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        f"Xvfb did not publish {socket} within {timeout:g}s",
    )


def _wait_for_window_manager(
    *,
    process: object,
    executable: Path,
    workspace_root: Path,
    evidence_dir: Path,
    child_environment: Mapping[str, str],
    timeout: float = 10.0,
    log_stem: str = "window-manager",
) -> BoundedProcessResult:
    deadline = time.monotonic() + timeout
    last_result: BoundedProcessResult | None = None
    while time.monotonic() < deadline:
        returncode = getattr(process, "poll")()
        if returncode is not None:
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"openbox exited during startup with status {returncode}",
            )
        last_result = run_bounded(
            (executable, "-root", "_NET_SUPPORTING_WM_CHECK"),
            cwd=workspace_root,
            env=child_environment,
            timeout=3.0,
            stdout_log=evidence_dir / f"{log_stem}.stdout.log",
            stderr_log=evidence_dir / f"{log_stem}.stderr.log",
        )
        if last_result.stream_errors or last_result.termination.errors:
            _check_stage(
                last_result,
                label="Openbox readiness probe",
                nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            )
        output = "\n".join(
            path.read_text(encoding="utf-8", errors="replace")
            for path in last_result.log_paths
        )
        if (
            not last_result.timed_out
            and last_result.returncode == 0
            and "_NET_SUPPORTING_WM_CHECK(WINDOW)" in output
        ):
            if getattr(process, "poll")() is not None:
                raise RuntimeContractError(
                    GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                    "openbox exited after publishing its readiness property",
                )
            return last_result
        time.sleep(0.1)
    if last_result is not None and last_result.timed_out:
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            "Openbox readiness probe timed out",
        )
    raise RuntimeContractError(
        GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        f"Openbox did not claim the window manager selection within {timeout:g}s",
    )


def _check_background(process: object, label: str) -> None:
    stream_errors = tuple(getattr(process, "stream_errors"))
    termination = getattr(process, "termination")
    termination_errors = () if termination is None else termination.errors
    if stream_errors or termination_errors:
        messages = (*stream_errors, *termination_errors)
        raise RuntimeContractError(
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"{label} cleanup or logging failed: {'; '.join(messages)}",
        )
