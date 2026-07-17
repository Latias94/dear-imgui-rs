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
from _verification import VerificationError, temporary_workspace  # noqa: E402


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
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
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
        else:  # pragma: no cover - argparse enforces the command set.
            parser.error(f"unknown contract: {args.contract}")
    except (CommandError, OSError, VerificationError) as error:
        print(f"::error::{error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
