#!/usr/bin/env python3
"""Package and consume release artifacts from an isolated clean checkout."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path


CI_DIR = Path(__file__).resolve().parent
TOOLS_DIR = CI_DIR.parent
WORKSPACE_ROOT = TOOLS_DIR.parent
for import_path in (CI_DIR, TOOLS_DIR):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))

from _prebuilt import verify_core_prebuilt_packages  # noqa: E402
from _process import CommandError  # noqa: E402
from _source_packages import verify_packaged_core  # noqa: E402
from _verification import VerificationError  # noqa: E402
from release_metadata import MetadataError  # noqa: E402
from source_metadata import SourceMetadataError  # noqa: E402


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate source packages and native Dear ImGui prebuilt artifacts",
        usage=(
            "%(prog)s [full]\n"
            "       %(prog)s prebuilt PACKAGE_DIR TARGET [CRT]\n"
            "       %(prog)s --verify-prebuilt-packages PACKAGE_DIR TARGET [CRT]"
        ),
    )
    parser.add_argument(
        "--verify-prebuilt-packages",
        dest="legacy_prebuilt",
        nargs="+",
        metavar="ARG",
        help=argparse.SUPPRESS,
    )
    commands = parser.add_subparsers(dest="command", title="commands")
    commands.add_parser("full", help="Run the complete source-package gate")
    prebuilt = commands.add_parser(
        "prebuilt", help="Consume all prebuilt profiles for one target"
    )
    prebuilt.add_argument("package_dir", metavar="PACKAGE_DIR", type=Path)
    prebuilt.add_argument("target", metavar="TARGET")
    prebuilt.add_argument("crt", metavar="CRT", nargs="?", default="")
    return parser


def _prebuilt_arguments(
    parser: argparse.ArgumentParser, args: argparse.Namespace
) -> tuple[Path, str, str] | None:
    if args.legacy_prebuilt is not None:
        if args.command is not None:
            parser.error("--verify-prebuilt-packages cannot be combined with a command")
        if len(args.legacy_prebuilt) not in (2, 3):
            parser.error("--verify-prebuilt-packages requires PACKAGE_DIR TARGET [CRT]")
        package_dir, target, *optional_crt = args.legacy_prebuilt
        return Path(package_dir), target, optional_crt[0] if optional_crt else ""
    if args.command == "prebuilt":
        return args.package_dir, args.target, args.crt
    return None


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    prebuilt_arguments = _prebuilt_arguments(parser, args)
    try:
        if prebuilt_arguments is None:
            verify_packaged_core()
        else:
            package_dir, target, crt = prebuilt_arguments
            verify_core_prebuilt_packages(
                package_dir,
                target,
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
