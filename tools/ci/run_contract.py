#!/usr/bin/env python3
"""Run repository CI contracts without shell-specific control flow."""

from __future__ import annotations

import argparse
import os
import re
import shlex
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path


CI_DIR = Path(__file__).resolve().parent
TOOLS_DIR = CI_DIR.parent
WORKSPACE_ROOT = TOOLS_DIR.parent
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _process import CommandError, environment, github_group, run  # noqa: E402
from _runtime_gate import (  # noqa: E402
    GateResult,
    run_multi_viewport_smoke,
    run_sdl3_glow_viewport_smoke,
    run_test_engine_runtime,
)
from _verification import VerificationError, temporary_workspace  # noqa: E402
from _windows_native import (  # noqa: E402
    ForbiddenImportError,
    WindowsNativeError,
    VcpkgTriplet,
    append_github_assignments,
    append_github_paths,
    calculate_mingw_environment,
    check_sdl3_vcpkg_consumer,
    ensure_vcpkg_status_compatibility,
    install_vcpkg_packages,
    locate_vcpkg_executable,
    resolve_vcpkg_root,
    restore_cached_sdl3_runtime,
    vcpkg_github_environment,
    vcpkg_root_candidates,
    verify_mingw_imports,
)


SYS_CRATES = (
    "dear-imgui-sys",
    "dear-implot-sys",
    "dear-imnodes-sys",
    "dear-node-editor-sys",
    "dear-implot3d-sys",
    "dear-imguizmo-sys",
    "dear-imguizmo-quat-sys",
    "dear-imgui-test-engine-sys",
)

RENDERER_FEATURE_CONFLICTS = (
    (
        "wgpu-dual-major",
        "Features `wgpu-27`, `wgpu-28`, `wgpu-29`, and `wgpu-30` are "
        "mutually exclusive; enable only one.",
        (
            "cargo",
            "check",
            "-p",
            "dear-imgui-wgpu",
            "--lib",
            "--no-default-features",
            "--features",
            "wgpu-29,wgpu-30",
        ),
    ),
    (
        "wgpu-dual-platform",
        "Features `multi-viewport-winit` and `multi-viewport-sdl3` are mutually "
        "exclusive; enable only one.",
        (
            "cargo",
            "check",
            "-p",
            "dear-imgui-wgpu",
            "--lib",
            "--no-default-features",
            "--features",
            "wgpu-30,multi-viewport-winit,multi-viewport-sdl3",
        ),
    ),
    (
        "ash-dual-platform",
        "dear-imgui-ash cannot enable both `multi-viewport-winit` and "
        "`multi-viewport-sdl3`; select one platform surface adapter",
        (
            "cargo",
            "check",
            "-p",
            "dear-imgui-ash",
            "--lib",
            "--no-default-features",
            "--features",
            "multi-viewport-winit,multi-viewport-sdl3",
        ),
    ),
)


def _command_arguments(arguments: Sequence[str]) -> list[str]:
    command = list(arguments)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise VerificationError("a command is required after --")
    return command


def expect_failure(
    label: str,
    required_messages: Sequence[str],
    command: Sequence[str],
) -> None:
    """Require a command to fail with every expected diagnostic fragment."""
    if not required_messages:
        raise VerificationError(f"{label} has no required diagnostic")
    result = run(
        command,
        cwd=WORKSPACE_ROOT,
        capture_output=True,
        combine_output=True,
        accepted_returncodes=None,
    )
    output = result.stdout or ""
    if result.returncode == 0:
        raise VerificationError(f"{label} unexpectedly succeeded")
    missing = [message for message in required_messages if message not in output]
    if missing:
        rendered = subprocess.list2cmdline(command)
        details = "\n".join(f"missing diagnostic: {message}" for message in missing)
        raise VerificationError(
            f"{label} failed without its required contract\n"
            f"command: {rendered}\n{details}\n{output.rstrip()}"
        )
    print(f"Verified expected failure: {label}")


def check_renderer_feature_conflicts() -> None:
    """Verify every renderer feature exclusivity contract."""
    for label, expected, command in RENDERER_FEATURE_CONFLICTS:
        with github_group(label):
            expect_failure(label, (expected,), command)


def check_unified_prebuilt_test_engine() -> None:
    """Prove source-only test-engine wins over an invalid prebuilt directory."""
    with temporary_workspace("dear-imgui-invalid-prebuilt-") as temporary:
        (temporary / "libdear_imgui.a").touch()
        (temporary / "dear_imgui.lib").touch()
        run(
            (
                "cargo",
                "check",
                "-p",
                "dear-imgui-rs",
                "--lib",
                "--no-default-features",
                "--features",
                "prebuilt,test-engine",
            ),
            cwd=WORKSPACE_ROOT,
            env=environment({"IMGUI_SYS_LIB_DIR": temporary}),
        )


def check_no_default_bindgen() -> None:
    """Reject bindgen and clang-sys in every sys crate's default graph."""
    for crate in SYS_CRATES:
        for dependency in ("bindgen", "clang-sys"):
            label = f"{crate} -> {dependency}"
            result = run(
                (
                    "cargo",
                    "tree",
                    "-p",
                    crate,
                    "--target",
                    "all",
                    "-i",
                    dependency,
                ),
                cwd=WORKSPACE_ROOT,
                capture_output=True,
                combine_output=True,
                accepted_returncodes=None,
            )
            output = result.stdout or ""
            if result.returncode == 0:
                raise VerificationError(
                    f"{label} is unexpectedly present in the default graph\n"
                    f"{output.rstrip()}"
                )
            if "did not match any packages" not in output:
                raise VerificationError(
                    f"cargo tree failed unexpectedly while checking {label}\n"
                    f"{output.rstrip()}"
                )
            print(f"Verified absent default dependency: {label}")


def run_clippy(cargo_arguments: Sequence[str]) -> None:
    """Run Clippy with the repository's explicit historical lint baseline."""
    arguments = _command_arguments(cargo_arguments)
    if "--" in arguments:
        raise VerificationError("run_contract.py owns the Clippy -- delimiter")
    raw_lints = os.environ.get("CLIPPY_HISTORICAL_LINTS", "")
    historical_lints = shlex.split(raw_lints)
    if not historical_lints or len(historical_lints) % 2 != 0:
        raise VerificationError("CLIPPY_HISTORICAL_LINTS must contain flag/value pairs")
    if any(
        historical_lints[index] != "-A"
        for index in range(0, len(historical_lints), 2)
    ):
        raise VerificationError("CLIPPY_HISTORICAL_LINTS may contain only -A allowances")
    run(
        ("cargo", "clippy", *arguments, "--", "-D", "warnings", *historical_lints),
        cwd=WORKSPACE_ROOT,
    )


def prepare_release_notes(tag: str, output: Path, github_output: Path) -> None:
    """Validate a release tag, extract its notes, and publish step outputs."""
    if not re.fullmatch(r"v[0-9A-Za-z][0-9A-Za-z.+-]*", tag):
        raise VerificationError(f"invalid release tag: {tag!r}")
    version = tag[1:]
    run(
        (
            sys.executable,
            TOOLS_DIR / "changelog.py",
            "check-soft-wrap",
            "--version",
            version,
        ),
        cwd=WORKSPACE_ROOT,
    )
    run(
        (
            sys.executable,
            TOOLS_DIR / "changelog.py",
            "extract",
            "--version",
            version,
            "--output",
            output,
        ),
        cwd=WORKSPACE_ROOT,
    )
    with github_output.open("a", encoding="utf-8", newline="\n") as destination:
        destination.write(f"tag={tag}\nversion={version}\n")


def configure_windows_vcpkg(
    *,
    target: str,
    crt: str,
    packages: Sequence[str],
    runner_temp: Path,
    github_environment: Path,
) -> None:
    """Install one explicit vcpkg profile and publish its validated root."""
    triplet = VcpkgTriplet.from_target(target, crt)
    executable = locate_vcpkg_executable()
    root = resolve_vcpkg_root(vcpkg_root_candidates(os.environ, executable)).path
    install_vcpkg_packages(packages, triplet, executable=executable)
    status = ensure_vcpkg_status_compatibility(root)
    append_github_assignments(
        github_environment,
        vcpkg_github_environment(root, triplet, runner_temp),
    )
    print(
        f"Validated vcpkg root {root} with {status.status_bytes} status bytes "
        f"and {status.update_bytes} update bytes for {triplet.name}"
    )


def configure_windows_mingw(
    *,
    msys2_root: Path,
    github_environment: Path,
    github_path: Path,
    current_path: str,
) -> None:
    """Publish one deterministic MinGW tool directory to later workflow steps."""
    mingw = calculate_mingw_environment(msys2_root, current_path)
    append_github_assignments(github_environment, mingw.github_environment)
    append_github_paths(github_path, mingw.github_path)
    print(f"Configured MinGW tools from {mingw.bin_directory}")


def check_windows_mingw_imports(
    *,
    deps_directory: Path,
    objdump: Path,
    evidence: Path,
) -> None:
    """Reject dynamic libstdc++ and retain complete objdump evidence."""
    try:
        inspection = verify_mingw_imports(deps_directory, objdump)
        evidence_text = inspection.evidence_text
    except ForbiddenImportError as error:
        evidence_text = error.inspection.evidence_text
        _write_mingw_evidence(evidence, evidence_text)
        raise
    except (CommandError, WindowsNativeError) as error:
        evidence_text = f"{error}\n"
        _write_mingw_evidence(evidence, evidence_text)
        raise
    _write_mingw_evidence(evidence, evidence_text)
    print(evidence_text, end="")


def _write_mingw_evidence(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    normalized = content.replace("\r\n", "\n").replace("\r", "\n")
    if normalized and not normalized.endswith("\n"):
        normalized += "\n"
    path.write_bytes(normalized.encode("utf-8"))


def _positive_float(value: str) -> float:
    parsed = float(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def _add_runtime_arguments(
    parser: argparse.ArgumentParser,
    *,
    evidence_dir: Path,
    child_timeout: float,
) -> None:
    parser.add_argument("--evidence-dir", type=Path, default=evidence_dir)
    parser.add_argument(
        "--child-timeout",
        type=_positive_float,
        default=child_timeout,
        help="Maximum runtime for each built child, in seconds",
    )
    parser.add_argument(
        "--build-timeout",
        type=_positive_float,
        default=900.0,
        help="Maximum Cargo build time, in seconds",
    )
    parser.add_argument(
        "--attempt",
        type=_positive_int,
        default=1,
        help="Fresh-runner attempt recorded in gate-result.json",
    )
    parser.add_argument(
        "--defer-infrastructure-retry",
        action="store_true",
        help=(
            "Return success for an eligible first infrastructure failure so CI can "
            "start one fresh runner"
        ),
    )


def _runtime_exit_code(result: GateResult, *, defer_infrastructure_retry: bool) -> int:
    github_output = os.environ.get("GITHUB_OUTPUT")
    if github_output:
        with Path(github_output).open("a", encoding="utf-8", newline="\n") as output:
            output.write(f"gate_success={str(result.success).lower()}\n")
            output.write(f"gate_category={result.category.value}\n")
            output.write(f"retry_eligible={str(result.retry_eligible).lower()}\n")
            output.write(f"gate_attempt={result.attempt}\n")
    if result.success or (defer_infrastructure_retry and result.retry_eligible):
        return 0
    print(
        f"::error::{result.category.value}: {result.summary}",
        file=sys.stderr,
    )
    return 1


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run cross-platform repository CI contracts"
    )
    commands = parser.add_subparsers(dest="contract", required=True)

    expected_failure = commands.add_parser(
        "expect-failure", help="Require a command to fail with expected diagnostics"
    )
    expected_failure.add_argument("--label", required=True)
    expected_failure.add_argument(
        "--contains", dest="required_messages", action="append", required=True
    )
    expected_failure.add_argument("command", nargs=argparse.REMAINDER)

    clippy = commands.add_parser(
        "clippy", help="Run Clippy with the historical lint baseline"
    )
    clippy.add_argument("cargo_arguments", nargs=argparse.REMAINDER)

    commands.add_parser(
        "renderer-feature-conflicts",
        help="Reject mutually exclusive renderer feature combinations",
    )
    commands.add_parser(
        "prebuilt-test-engine",
        help="Verify source-only test-engine wins over prebuilt selection",
    )
    commands.add_parser(
        "no-default-bindgen",
        help="Reject bindgen dependencies from default sys crate graphs",
    )
    release = commands.add_parser(
        "release-notes", help="Extract release notes from RELEASE_TAG"
    )
    release.add_argument("--output", type=Path, default=Path("release-notes.md"))

    vcpkg = commands.add_parser(
        "windows-vcpkg", help="Install and validate one Windows vcpkg profile"
    )
    vcpkg.add_argument("--target", required=True)
    vcpkg.add_argument("--crt", required=True, choices=("md", "mt"))
    vcpkg.add_argument("--package", action="append", required=True)
    vcpkg.add_argument("--runner-temp", required=True, type=Path)
    vcpkg.add_argument("--github-env", required=True, type=Path)

    sdl3_consumer = commands.add_parser(
        "windows-sdl3-consumer",
        help="Run the temporary SDL3 vcpkg discovery consumer",
    )
    sdl3_consumer.add_argument("--workspace", required=True, type=Path)
    sdl3_consumer.add_argument("--repo-root", type=Path, default=WORKSPACE_ROOT)

    sdl3_runtime = commands.add_parser(
        "windows-sdl3-runtime",
        help="Restore SDL3.dll after Cargo build-cache restoration",
    )
    sdl3_runtime.add_argument("--target-dir", type=Path, default=Path("target"))
    sdl3_runtime.add_argument("--profile", default="debug")
    sdl3_runtime.add_argument("--github-path", required=True, type=Path)

    mingw_environment = commands.add_parser(
        "windows-mingw-env", help="Publish the selected MinGW tool directory"
    )
    mingw_environment.add_argument("--msys2-root", required=True, type=Path)
    mingw_environment.add_argument("--github-env", required=True, type=Path)
    mingw_environment.add_argument("--github-path", required=True, type=Path)

    mingw_imports = commands.add_parser(
        "windows-mingw-imports", help="Inspect MinGW test executable imports"
    )
    mingw_imports.add_argument("--deps", required=True, type=Path)
    mingw_imports.add_argument("--objdump", required=True, type=Path)
    mingw_imports.add_argument("--evidence", required=True, type=Path)

    test_engine_runtime = commands.add_parser(
        "test-engine-runtime",
        help="Execute every stable Test Engine runner outcome",
    )
    _add_runtime_arguments(
        test_engine_runtime,
        evidence_dir=Path("target/ci-runtime/test-engine-runtime"),
        child_timeout=120.0,
    )

    viewport_runtime = commands.add_parser(
        "multi-viewport-smoke",
        help="Execute the real Winit/WGPU secondary-window lifecycle",
    )
    _add_runtime_arguments(
        viewport_runtime,
        evidence_dir=Path("target/ci-runtime/multi-viewport-smoke"),
        child_timeout=180.0,
    )

    sdl3_glow_viewport_runtime = commands.add_parser(
        "sdl3-glow-multi-viewport-smoke",
        help="Execute the real SDL3/Glow secondary-window lifecycle",
    )
    _add_runtime_arguments(
        sdl3_glow_viewport_runtime,
        evidence_dir=Path("target/ci-runtime/sdl3-glow-multi-viewport-smoke"),
        child_timeout=180.0,
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    exit_code = 0
    try:
        if args.contract == "expect-failure":
            expect_failure(
                args.label,
                args.required_messages,
                _command_arguments(args.command),
            )
        elif args.contract == "clippy":
            run_clippy(args.cargo_arguments)
        elif args.contract == "renderer-feature-conflicts":
            check_renderer_feature_conflicts()
        elif args.contract == "prebuilt-test-engine":
            check_unified_prebuilt_test_engine()
        elif args.contract == "no-default-bindgen":
            check_no_default_bindgen()
        elif args.contract == "release-notes":
            release_tag = os.environ.get("RELEASE_TAG", "")
            github_output = os.environ.get("GITHUB_OUTPUT")
            if not github_output:
                raise VerificationError("GITHUB_OUTPUT is required for release-notes")
            prepare_release_notes(release_tag, args.output, Path(github_output))
        elif args.contract == "windows-vcpkg":
            configure_windows_vcpkg(
                target=args.target,
                crt=args.crt,
                packages=args.package,
                runner_temp=args.runner_temp,
                github_environment=args.github_env,
            )
        elif args.contract == "windows-sdl3-consumer":
            check_sdl3_vcpkg_consumer(args.workspace, args.repo_root)
        elif args.contract == "windows-sdl3-runtime":
            runtime = restore_cached_sdl3_runtime(args.target_dir, args.profile)
            append_github_paths(args.github_path, (runtime.parent.resolve(),))
        elif args.contract == "windows-mingw-env":
            configure_windows_mingw(
                msys2_root=args.msys2_root,
                github_environment=args.github_env,
                github_path=args.github_path,
                current_path=os.environ.get("PATH", ""),
            )
        elif args.contract == "windows-mingw-imports":
            check_windows_mingw_imports(
                deps_directory=args.deps,
                objdump=args.objdump,
                evidence=args.evidence,
            )
        elif args.contract == "test-engine-runtime":
            result = run_test_engine_runtime(
                workspace_root=WORKSPACE_ROOT,
                evidence_dir=args.evidence_dir,
                child_timeout=args.child_timeout,
                build_timeout=args.build_timeout,
                attempt=args.attempt,
            )
            exit_code = _runtime_exit_code(
                result,
                defer_infrastructure_retry=args.defer_infrastructure_retry,
            )
        elif args.contract == "multi-viewport-smoke":
            result = run_multi_viewport_smoke(
                workspace_root=WORKSPACE_ROOT,
                evidence_dir=args.evidence_dir,
                child_timeout=args.child_timeout,
                build_timeout=args.build_timeout,
                attempt=args.attempt,
            )
            exit_code = _runtime_exit_code(
                result,
                defer_infrastructure_retry=args.defer_infrastructure_retry,
            )
        elif args.contract == "sdl3-glow-multi-viewport-smoke":
            result = run_sdl3_glow_viewport_smoke(
                workspace_root=WORKSPACE_ROOT,
                evidence_dir=args.evidence_dir,
                child_timeout=args.child_timeout,
                build_timeout=args.build_timeout,
                attempt=args.attempt,
            )
            exit_code = _runtime_exit_code(
                result,
                defer_infrastructure_retry=args.defer_infrastructure_retry,
            )
        else:  # pragma: no cover - argparse enforces the command set.
            parser.error(f"unknown contract: {args.contract}")
    except CommandError as error:
        print(f"::error::{error}", file=sys.stderr)
        return error.returncode if error.returncode > 0 else 1
    except (OSError, VerificationError, WindowsNativeError) as error:
        print(f"::error::{error}", file=sys.stderr)
        return 1
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
