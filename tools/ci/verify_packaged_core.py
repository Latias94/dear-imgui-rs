#!/usr/bin/env python3
"""Package and consume release artifacts from an isolated clean checkout."""

from __future__ import annotations

import argparse
import os
import sys
from collections.abc import Sequence
from pathlib import Path


CI_DIR = Path(__file__).resolve().parent
TOOLS_DIR = CI_DIR.parent
WORKSPACE_ROOT = TOOLS_DIR.parent
for import_path in (CI_DIR, TOOLS_DIR):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))

from _prebuilt import (  # noqa: E402
    build_release_prebuilt_packages,
    verify_prebuilt_packages,
)
from _process import CommandError, run  # noqa: E402
from _source_packages import verify_packaged_core, verify_source_packages  # noqa: E402
from _verification import VerificationError  # noqa: E402
from release_metadata import MetadataError  # noqa: E402
from source_metadata import SourceMetadataError  # noqa: E402


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate source packages and native Dear ImGui prebuilt artifacts",
        usage=(
            "%(prog)s [full]\n"
            "       %(prog)s build-prebuilt PACKAGE_DIR TARGET CANDIDATE_SHA [CRT]\n"
            "       %(prog)s prebuilt PACKAGE_DIR TARGET CANDIDATE_SHA [CRT]"
        ),
    )
    commands = parser.add_subparsers(dest="command", title="commands")
    commands.add_parser("full", help="Run the complete source-package gate")
    commands.add_parser(
        "source", help="Verify all publishable source packages without prebuilt output"
    )
    prebuilt = commands.add_parser(
        "prebuilt", help="Consume all prebuilt profiles for one target"
    )
    prebuilt.add_argument("package_dir", metavar="PACKAGE_DIR", type=Path)
    prebuilt.add_argument("target", metavar="TARGET")
    prebuilt.add_argument("candidate_sha", metavar="CANDIDATE_SHA")
    prebuilt.add_argument("crt", metavar="CRT", nargs="?", default="")
    build_prebuilt = commands.add_parser(
        "build-prebuilt", help="Build every native core and extension artifact profile"
    )
    build_prebuilt.add_argument("package_dir", metavar="PACKAGE_DIR", type=Path)
    build_prebuilt.add_argument("target", metavar="TARGET")
    build_prebuilt.add_argument("candidate_sha", metavar="CANDIDATE_SHA")
    build_prebuilt.add_argument("crt", metavar="CRT", nargs="?", default="")
    build_prebuilt.add_argument("--target-dir", type=Path)
    return parser


def _prebuilt_arguments(args: argparse.Namespace) -> tuple[Path, str, str, str] | None:
    if args.command == "prebuilt":
        return args.package_dir, args.target, args.candidate_sha, args.crt
    return None


def _resolve_candidate_sha(candidate_sha: str) -> str:
    if candidate_sha != "HEAD":
        return candidate_sha
    result = run(
        ("git", "-C", WORKSPACE_ROOT, "rev-parse", "--verify", "HEAD"),
        capture_output=True,
    )
    return (result.stdout or "").strip()


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    prebuilt_arguments = _prebuilt_arguments(args)
    try:
        if args.command == "build-prebuilt":
            target_dir = args.target_dir
            if target_dir is None:
                target_dir = Path(
                    os.environ.get(
                        "CARGO_TARGET_DIR", WORKSPACE_ROOT / "target/prebuilt-release"
                    )
                )
            build_release_prebuilt_packages(
                WORKSPACE_ROOT,
                target_dir,
                args.package_dir,
                args.target,
                _resolve_candidate_sha(args.candidate_sha),
                crt=args.crt,
            )
        elif args.command == "source":
            verify_source_packages()
        elif prebuilt_arguments is None:
            verify_packaged_core()
        else:
            package_dir, target, candidate_sha, crt = prebuilt_arguments
            verify_prebuilt_packages(
                package_dir,
                target,
                _resolve_candidate_sha(candidate_sha),
                crt=crt,
                source_root=WORKSPACE_ROOT,
                profile_scope="all",
            )
    except (
        CommandError,
        MetadataError,
        OSError,
        SourceMetadataError,
        VerificationError,
    ) as error:
        print(f"::error::{error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
