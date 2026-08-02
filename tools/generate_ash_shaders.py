#!/usr/bin/env python3
"""Generate or verify dear-imgui-ash's checked-in SPIR-V shaders."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterator, Mapping
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[1]
SHADER_ROOT = REPO_ROOT / "backends" / "dear-imgui-ash" / "src" / "shaders"
SHADERS = ("shader.vert", "shader.frag")
MANIFEST = SHADER_ROOT / "manifest.json"
DEFAULT_COMPILER_TIMEOUT_SECONDS = 60.0
EMBEDDED_SOURCE_PREFIX = (
    b"// OpModuleProcessed client vulkan100\n"
    b"// OpModuleProcessed target-env vulkan1.0\n"
    b"// OpModuleProcessed entry-point main\n"
    b"#line 1\n"
)


def find_compiler(explicit: str | None) -> Path:
    candidates: list[str] = []
    if explicit:
        candidates.append(explicit)
    vulkan_sdk = os.environ.get("VULKAN_SDK")
    if vulkan_sdk:
        candidates.extend(
            str(Path(vulkan_sdk) / directory / "glslangValidator")
            for directory in ("Bin", "bin")
        )
    discovered = shutil.which("glslangValidator")
    if discovered:
        candidates.append(discovered)

    for candidate in candidates:
        path = Path(candidate)
        if path.is_file():
            return path.resolve()
        executable = path.with_suffix(".exe")
        if executable.is_file():
            return executable.resolve()
    raise RuntimeError(
        "glslangValidator was not found; pass --compiler or set VULKAN_SDK"
    )


def compile_shader(
    compiler: Path,
    source: Path,
    output: Path,
    *,
    timeout_seconds: float = DEFAULT_COMPILER_TIMEOUT_SECONDS,
) -> None:
    try:
        subprocess.run(
            [
                str(compiler),
                "-V",
                "-g",
                "--target-env",
                "vulkan1.0",
                "-o",
                str(output.resolve()),
                source.name,
            ],
            cwd=source.parent,
            check=True,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(
            f"compiling {source.name} exceeded the {timeout_seconds:g}-second timeout"
        ) from error


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def spirv_literal(words: tuple[int, ...]) -> bytes:
    encoded = struct.pack(f"<{len(words)}I", *words)
    return encoded.split(b"\0", 1)[0]


def spirv_words(path: Path, data: bytes | None = None) -> tuple[int, ...]:
    if data is None:
        data = path.read_bytes()
    if len(data) < 20 or len(data) % 4 != 0:
        raise RuntimeError(f"{path} is not a complete SPIR-V module")
    words = struct.unpack(f"<{len(data) // 4}I", data)
    if words[0] != 0x07230203:
        raise RuntimeError(f"{path} has an invalid SPIR-V magic number")
    return words


def iter_spirv_instructions(
    path: Path, words: tuple[int, ...]
) -> Iterator[tuple[int, tuple[int, ...]]]:
    cursor = 5
    while cursor < len(words):
        instruction = words[cursor]
        word_count = instruction >> 16
        opcode = instruction & 0xFFFF
        if word_count == 0 or cursor + word_count > len(words):
            raise RuntimeError(f"{path} contains an invalid SPIR-V instruction")
        yield opcode, words[cursor + 1 : cursor + word_count]
        cursor += word_count


def inspect_spirv(path: Path, data: bytes | None = None) -> tuple[bytes, list[str]]:
    words = spirv_words(path, data)

    source = bytearray()
    source_found = False
    paths: list[str] = []
    for opcode, operands in iter_spirv_instructions(path, words):
        if opcode == 3 and len(operands) >= 4:  # OpSource with file and source operands.
            source = bytearray(spirv_literal(operands[3:]))
            source_found = True
        elif opcode == 2 and source_found:  # OpSourceContinued.
            source.extend(spirv_literal(operands))
        elif opcode == 7 and len(operands) >= 2:  # OpString result id and UTF-8 text.
            paths.append(spirv_literal(operands[1:]).decode("utf-8"))
    if not source_found:
        raise RuntimeError(f"{path} does not embed its GLSL source with OpSource")
    return bytes(source), paths


def embedded_glsl_source(path: Path) -> bytes:
    return inspect_spirv(path)[0]


def spirv_debug_source_paths(path: Path) -> list[str]:
    return inspect_spirv(path)[1]


def verify_embedded_source(source: Path, spirv: Path) -> bool:
    return embedded_glsl_source(spirv) == EMBEDDED_SOURCE_PREFIX + source.read_bytes()


def verify_debug_source_path(source: Path, spirv: Path) -> bool:
    return spirv_debug_source_paths(spirv) == [source.name]


def compiler_identity(
    compiler: Path,
    *,
    timeout_seconds: float = DEFAULT_COMPILER_TIMEOUT_SECONDS,
) -> str:
    try:
        result = subprocess.run(
            [str(compiler), "--version"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(
            f"reading the compiler version exceeded the {timeout_seconds:g}-second timeout"
        ) from error
    return result.stdout.splitlines()[0].strip()


def manifest_payload(
    compiler: Path,
    spirv_paths: Mapping[str, Path] | None = None,
    *,
    timeout_seconds: float = DEFAULT_COMPILER_TIMEOUT_SECONDS,
) -> dict[str, object]:
    if spirv_paths is None:
        spirv_paths = {name: SHADER_ROOT / f"{name}.spv" for name in SHADERS}
    return {
        "schema_version": 1,
        "generator": compiler_identity(compiler, timeout_seconds=timeout_seconds),
        "source_embedding": (
            "canonical glslang OpSource prefix, exact source, and relative OpString path"
        ),
        "target_environment": "vulkan1.0",
        "shaders": {
            name: {
                "source_sha256": file_sha256(SHADER_ROOT / name),
                "spirv_sha256": file_sha256(spirv_paths[name]),
            }
            for name in SHADERS
        },
    }


def compile_all(
    compiler: Path,
    output_root: Path,
    *,
    timeout_seconds: float,
) -> dict[str, Path]:
    outputs: dict[str, Path] = {}
    for name in SHADERS:
        output = output_root / f"{name}.spv"
        compile_shader(
            compiler,
            SHADER_ROOT / name,
            output,
            timeout_seconds=timeout_seconds,
        )
        outputs[name] = output
    return outputs


def recompile_errors(
    compiler: Path,
    *,
    timeout_seconds: float = DEFAULT_COMPILER_TIMEOUT_SECONDS,
) -> list[str]:
    with tempfile.TemporaryDirectory(
        prefix=".ash-shader-check-", dir=SHADER_ROOT
    ) as temporary:
        outputs = compile_all(
            compiler,
            Path(temporary),
            timeout_seconds=timeout_seconds,
        )
        return [
            f"{name}.spv differs from a clean compiler regeneration"
            for name in SHADERS
            if outputs[name].read_bytes()
            != (SHADER_ROOT / f"{name}.spv").read_bytes()
        ]


def verify(
    recompile_compiler: Path | None = None,
    *,
    timeout_seconds: float = DEFAULT_COMPILER_TIMEOUT_SECONDS,
) -> int:
    try:
        manifest_text = MANIFEST.read_text(encoding="utf-8")
    except FileNotFoundError:
        print(f"Ash shader manifest is missing: {MANIFEST.relative_to(REPO_ROOT)}", file=sys.stderr)
        return 1
    payload = json.loads(manifest_text)
    errors: list[str] = []
    if payload.get("schema_version") != 1:
        errors.append("manifest schema_version must be 1")
    if payload.get("target_environment") != "vulkan1.0":
        errors.append("manifest target_environment must be vulkan1.0")
    if (
        payload.get("source_embedding")
        != "canonical glslang OpSource prefix, exact source, and relative OpString path"
    ):
        errors.append(
            "manifest source_embedding must require exact source and a relative debug path"
        )
    shaders = payload.get("shaders")
    if not isinstance(shaders, dict):
        errors.append("manifest shaders must be an object")
        shaders = {}
    for name in SHADERS:
        record = shaders.get(name)
        if not isinstance(record, dict):
            errors.append(f"manifest entry is missing for {name}")
            continue
        source = SHADER_ROOT / name
        spirv = SHADER_ROOT / f"{name}.spv"
        try:
            source_data = source.read_bytes()
        except FileNotFoundError:
            source_data = None
            errors.append(f"shader file is missing: {source.relative_to(REPO_ROOT)}")
        try:
            spirv_data = spirv.read_bytes()
        except FileNotFoundError:
            spirv_data = None
            errors.append(f"shader file is missing: {spirv.relative_to(REPO_ROOT)}")

        if source_data is not None and record.get("source_sha256") != hashlib.sha256(
            source_data
        ).hexdigest():
            errors.append(
                f"{source.relative_to(REPO_ROOT)} does not match manifest source_sha256"
            )
        if spirv_data is not None and record.get("spirv_sha256") != hashlib.sha256(
            spirv_data
        ).hexdigest():
            errors.append(
                f"{spirv.relative_to(REPO_ROOT)} does not match manifest spirv_sha256"
            )
        if source_data is None or spirv_data is None:
            continue

        embedded_source, debug_paths = inspect_spirv(spirv, spirv_data)
        if embedded_source != EMBEDDED_SOURCE_PREFIX + source_data:
            errors.append(
                f"{spirv.relative_to(REPO_ROOT)} does not embed the current {name} source"
            )
        if debug_paths != [source.name]:
            errors.append(
                f"{spirv.relative_to(REPO_ROOT)} does not use the stable debug source name {name}"
            )
    if recompile_compiler is not None and not errors:
        identity = compiler_identity(
            recompile_compiler, timeout_seconds=timeout_seconds
        )
        if payload.get("generator") != identity:
            errors.append(
                "compiler identity does not match the generator recorded in the manifest: "
                f"expected {payload.get('generator')!r}, got {identity!r}"
            )
        else:
            errors.extend(
                recompile_errors(
                    recompile_compiler,
                    timeout_seconds=timeout_seconds,
                )
            )
    if errors:
        print("Ash shader artifact contract failed:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("Ash shader sources and SPIR-V artifacts match their manifest")
    return 0


def publish_artifacts(
    outputs: Mapping[str, Path],
    temporary_manifest: Path,
    *,
    replace: Callable[[Path, Path], None] = os.replace,
) -> None:
    artifacts = [
        *((outputs[name], SHADER_ROOT / f"{name}.spv") for name in SHADERS),
        (temporary_manifest, MANIFEST),
    ]
    backup_root = temporary_manifest.parent / ".publish-backup"
    backup_root.mkdir()

    originals: dict[Path, Path | None] = {}
    for _, destination in artifacts:
        if destination.exists():
            backup = backup_root / destination.name
            shutil.copy2(destination, backup)
            originals[destination] = backup
        else:
            originals[destination] = None

    published: list[Path] = []
    destination = artifacts[0][1]
    try:
        for source, destination in artifacts:
            replace(source, destination)
            published.append(destination)
    except OSError as publish_error:
        rollback_errors: list[str] = []
        for published_destination in reversed(published):
            backup = originals[published_destination]
            try:
                if backup is None:
                    published_destination.unlink(missing_ok=True)
                else:
                    replace(backup, published_destination)
            except OSError as rollback_error:
                rollback_errors.append(
                    f"{published_destination.name}: {rollback_error}"
                )

        message = f"failed to publish {destination.name}: {publish_error}"
        if rollback_errors:
            raise RuntimeError(
                f"{message}; rollback also failed: {'; '.join(rollback_errors)}"
            ) from publish_error
        raise RuntimeError(
            f"{message}; restored previous artifacts"
        ) from publish_error


def generate(
    compiler: Path,
    *,
    timeout_seconds: float = DEFAULT_COMPILER_TIMEOUT_SECONDS,
) -> int:
    with tempfile.TemporaryDirectory(
        prefix=".ash-shader-generate-", dir=SHADER_ROOT
    ) as temporary:
        temporary_root = Path(temporary)
        outputs = compile_all(
            compiler,
            temporary_root,
            timeout_seconds=timeout_seconds,
        )
        for name, output in outputs.items():
            source = SHADER_ROOT / name
            if not verify_embedded_source(source, output):
                raise RuntimeError(f"generated {name}.spv does not embed the current source")
            if not verify_debug_source_path(source, output):
                raise RuntimeError(
                    f"generated {name}.spv does not use the stable debug source name"
                )

        temporary_manifest = temporary_root / MANIFEST.name
        temporary_manifest.write_text(
            json.dumps(
                manifest_payload(
                    compiler,
                    outputs,
                    timeout_seconds=timeout_seconds,
                ),
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )

        publish_artifacts(outputs, temporary_manifest)

    for name in SHADERS:
        print(f"generated {(SHADER_ROOT / f'{name}.spv').relative_to(REPO_ROOT)}")
    print(f"generated {MANIFEST.relative_to(REPO_ROOT)}")
    return 0


def positive_timeout(value: str) -> float:
    timeout = float(value)
    if timeout <= 0:
        raise argparse.ArgumentTypeError("compiler timeout must be greater than zero")
    return timeout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when artifacts are stale")
    parser.add_argument("--compiler", help="path to glslangValidator")
    parser.add_argument(
        "--recompile",
        action="store_true",
        help="with --check, rebuild shaders and require byte-identical SPIR-V",
    )
    parser.add_argument(
        "--compiler-timeout",
        type=positive_timeout,
        default=DEFAULT_COMPILER_TIMEOUT_SECONDS,
        metavar="SECONDS",
        help="maximum time for each compiler invocation",
    )
    args = parser.parse_args()
    if args.recompile and not args.check:
        parser.error("--recompile requires --check")
    try:
        if args.check:
            compiler = find_compiler(args.compiler) if args.recompile else None
            return verify(compiler, timeout_seconds=args.compiler_timeout)
        return generate(
            find_compiler(args.compiler),
            timeout_seconds=args.compiler_timeout,
        )
    except (json.JSONDecodeError, OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
