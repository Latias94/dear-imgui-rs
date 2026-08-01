#!/usr/bin/env python3
"""Build and verify the maintained Emscripten import provider."""

from __future__ import annotations

import argparse
import subprocess
import sys
from collections.abc import Callable, Sequence
from pathlib import Path


CI_DIR = Path(__file__).resolve().parent
REPO_ROOT = CI_DIR.parents[1]
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _source_inventory import load_inventory, load_inventory_file  # noqa: E402


Runner = Callable[..., subprocess.CompletedProcess[str]]
WASM_TARGET = "wasm32-unknown-unknown"
RUST_ROUTE_COMMAND = (
    "cargo",
    "check",
    "--target",
    WASM_TARGET,
    "--no-default-features",
    "-p",
    "dear-imgui-rs",
    "-p",
    "dear-implot",
    "-p",
    "dear-implot3d",
    "-p",
    "dear-imnodes",
    "-p",
    "dear-imguizmo",
    "-p",
    "dear-imguizmo-quat",
    "--features",
    "dear-imgui-rs/wasm,dear-implot/wasm,dear-implot3d/wasm,"
    "dear-imnodes/wasm,dear-imguizmo/wasm,dear-imguizmo-quat/wasm",
)
PROVIDER_COMMAND = (
    "cargo",
    "run",
    "-p",
    "xtask",
    "--",
    "build-cimgui-provider",
)
LEGACY_PROVIDER_MODULES = ("imgui-sys-v0",)


class WasmProviderVerificationError(RuntimeError):
    """Raised when the provider command succeeds without complete artifacts."""


def provider_command(
    *,
    source_root: Path | None = None,
    inventory_path: Path | None = None,
    output_dir: Path | None = None,
) -> tuple[str, ...]:
    """Compose the xtask provider command for checkout or packaged sources."""
    command = list(PROVIDER_COMMAND)
    for option, value in (
        ("--source-root", source_root),
        ("--inventory", inventory_path),
        ("--out-dir", output_dir),
    ):
        if value is not None:
            command.extend((option, str(value.resolve())))
    return tuple(command)


def verify_wasm_provider(
    repo_root: Path,
    *,
    check_rust_route: bool = False,
    provider_source_root: Path | None = None,
    inventory_path: Path | None = None,
    output_dir: Path | None = None,
    runner: Runner = subprocess.run,
) -> tuple[Path, ...]:
    repo_root = repo_root.resolve()
    provider_source_root = (
        provider_source_root.resolve() if provider_source_root is not None else None
    )
    inventory_path = inventory_path.resolve() if inventory_path is not None else None
    requested_output_dir = output_dir.resolve() if output_dir is not None else None
    output_dir = requested_output_dir or repo_root / "target" / "web-demo"
    if check_rust_route:
        runner(RUST_ROUTE_COMMAND, cwd=repo_root, check=True, text=True)
    runner(
        provider_command(
            source_root=provider_source_root,
            inventory_path=inventory_path,
            output_dir=requested_output_dir,
        ),
        cwd=repo_root,
        check=True,
        text=True,
    )

    inventory = (
        load_inventory_file(inventory_path)
        if inventory_path is not None
        else load_inventory(repo_root)
    )
    module_name = inventory.wasm_import_module
    artifacts = (
        output_dir / f"{module_name}.js",
        output_dir / f"{module_name}.wasm",
        output_dir / f"{module_name}-wrapper.js",
        output_dir / "imgui_exports.json",
    )
    missing = [path for path in artifacts if not path.is_file() or path.stat().st_size == 0]
    if missing:
        legacy_artifacts = tuple(
            output_dir / f"{legacy_module}{suffix}"
            for legacy_module in LEGACY_PROVIDER_MODULES
            for suffix in (".js", ".wasm", "-wrapper.js")
        )
        legacy_present = [path for path in legacy_artifacts if path.is_file()]
        legacy_detail = (
            "; legacy provider artifacts are incompatible with "
            f"{module_name}: " + ", ".join(str(path) for path in legacy_present)
            if legacy_present
            else ""
        )
        raise WasmProviderVerificationError(
            "provider build did not produce non-empty artifacts: "
            + ", ".join(str(path) for path in missing)
            + legacy_detail
        )
    print(
        f"Verified Emscripten provider {module_name} with "
        f"{len(artifacts)} non-empty artifacts."
    )
    return artifacts


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Build the maintained Emscripten provider and verify its artifacts"
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Repository root (defaults to the checkout containing this script)",
    )
    parser.add_argument(
        "--check-rust-route",
        action="store_true",
        help="Check the complete Rust WASM route before building the provider",
    )
    parser.add_argument(
        "--provider-source-root",
        type=Path,
        help="Resolve provider sources from this staged package tree",
    )
    parser.add_argument(
        "--inventory",
        type=Path,
        help="Use this explicit maintained-source inventory",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        help="Write provider artifacts to this directory",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        verify_wasm_provider(
            args.repo_root,
            check_rust_route=args.check_rust_route,
            provider_source_root=args.provider_source_root,
            inventory_path=args.inventory,
            output_dir=args.out_dir,
        )
    except (OSError, subprocess.CalledProcessError, ValueError, WasmProviderVerificationError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
