import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "tools"))

import generate_ash_shaders  # noqa: E402


class EmbeddedShaderSourceTests(unittest.TestCase):
    def test_relative_explicit_compiler_is_resolved_before_shader_cwd_changes(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=Path.cwd()) as temporary:
            compiler = Path(temporary) / "vendor" / "glslangValidator.exe"
            compiler.parent.mkdir()
            compiler.touch()

            relative_compiler = os.path.relpath(compiler, Path.cwd())
            resolved = generate_ash_shaders.find_compiler(str(relative_compiler))

        self.assertEqual(resolved, compiler.resolve())

    def test_checked_in_artifacts_embed_the_exact_sources(self) -> None:
        for name in generate_ash_shaders.SHADERS:
            source = generate_ash_shaders.SHADER_ROOT / name
            spirv = generate_ash_shaders.SHADER_ROOT / f"{name}.spv"
            self.assertTrue(generate_ash_shaders.verify_embedded_source(source, spirv))

    def test_source_prefix_deletion_cannot_match_an_old_artifact(self) -> None:
        source = generate_ash_shaders.SHADER_ROOT / "shader.vert"
        spirv = generate_ash_shaders.SHADER_ROOT / "shader.vert.spv"
        original = source.read_bytes()
        _, modified = original.split(b"\n", 1)

        with tempfile.TemporaryDirectory() as temporary:
            modified_source = Path(temporary) / source.name
            modified_source.write_bytes(modified)
            self.assertFalse(
                generate_ash_shaders.verify_embedded_source(modified_source, spirv)
            )

    def test_checked_in_artifacts_use_stable_debug_source_names(self) -> None:
        for name in generate_ash_shaders.SHADERS:
            spirv = generate_ash_shaders.SHADER_ROOT / f"{name}.spv"
            self.assertEqual(
                generate_ash_shaders.spirv_debug_source_paths(spirv),
                [name],
            )

    def test_compile_uses_shader_directory_and_relative_names(self) -> None:
        source = generate_ash_shaders.SHADER_ROOT / "shader.vert"
        output = generate_ash_shaders.SHADER_ROOT / "shader.vert.spv"
        compiler = Path("glslangValidator")

        with mock.patch.object(generate_ash_shaders.subprocess, "run") as run:
            generate_ash_shaders.compile_shader(
                compiler, source, output, timeout_seconds=17.0
            )

        command = run.call_args.args[0]
        self.assertEqual(run.call_args.kwargs["cwd"], source.parent)
        self.assertEqual(run.call_args.kwargs["timeout"], 17.0)
        self.assertEqual(command[-3:], ["-o", str(output.resolve()), source.name])

    def test_compile_timeout_reports_the_shader_name(self) -> None:
        source = generate_ash_shaders.SHADER_ROOT / "shader.vert"
        output = generate_ash_shaders.SHADER_ROOT / "shader.vert.spv"

        with mock.patch.object(
            generate_ash_shaders.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["glslangValidator"], 3.0),
        ):
            with self.assertRaisesRegex(RuntimeError, "shader.vert.*3"):
                generate_ash_shaders.compile_shader(
                    Path("glslangValidator"),
                    source,
                    output,
                    timeout_seconds=3.0,
                )

    def test_compiler_identity_timeout_is_bounded(self) -> None:
        with mock.patch.object(
            generate_ash_shaders.subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["glslangValidator"], 5.0),
        ):
            with self.assertRaisesRegex(RuntimeError, "version.*5"):
                generate_ash_shaders.compiler_identity(
                    Path("glslangValidator"), timeout_seconds=5.0
                )

    def test_failed_second_compile_does_not_publish_partial_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            shader_root = Path(temporary)
            for name in generate_ash_shaders.SHADERS:
                (shader_root / name).write_text(f"source:{name}", encoding="utf-8")
                (shader_root / f"{name}.spv").write_bytes(f"old:{name}".encode())
            manifest = shader_root / "manifest.json"
            manifest.write_text("old manifest\n", encoding="utf-8")
            original = {
                path.name: path.read_bytes()
                for path in (*shader_root.glob("*.spv"), manifest)
            }

            def compile_side_effect(
                _compiler: Path,
                source: Path,
                output: Path,
                *,
                timeout_seconds: float,
            ) -> None:
                self.assertEqual(timeout_seconds, 11.0)
                if source.name == generate_ash_shaders.SHADERS[1]:
                    raise RuntimeError("second shader failed")
                output.write_bytes(b"new first shader")

            with (
                mock.patch.object(generate_ash_shaders, "SHADER_ROOT", shader_root),
                mock.patch.object(generate_ash_shaders, "MANIFEST", manifest),
                mock.patch.object(
                    generate_ash_shaders,
                    "compile_shader",
                    side_effect=compile_side_effect,
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "second shader failed"):
                    generate_ash_shaders.generate(
                        Path("glslangValidator"), timeout_seconds=11.0
                    )

            self.assertEqual(
                {
                    path.name: path.read_bytes()
                    for path in (*shader_root.glob("*.spv"), manifest)
                },
                original,
            )

    def test_publish_failure_restores_every_previous_artifact(self) -> None:
        for failed_replace in (2, 3):
            with self.subTest(failed_replace=failed_replace):
                with tempfile.TemporaryDirectory() as temporary:
                    shader_root = Path(temporary)
                    manifest = shader_root / "manifest.json"
                    original = {
                        **{
                            f"{name}.spv": f"old:{name}".encode()
                            for name in generate_ash_shaders.SHADERS
                        },
                        manifest.name: b"old manifest\n",
                    }
                    for filename, data in original.items():
                        (shader_root / filename).write_bytes(data)

                    staging_root = shader_root / "staging"
                    staging_root.mkdir()
                    outputs = {}
                    for name in generate_ash_shaders.SHADERS:
                        output = staging_root / f"{name}.spv"
                        output.write_bytes(f"new:{name}".encode())
                        outputs[name] = output
                    staged_manifest = staging_root / manifest.name
                    staged_manifest.write_bytes(b"new manifest\n")

                    replace_calls = 0

                    def replace_with_failure(source: Path, destination: Path) -> None:
                        nonlocal replace_calls
                        replace_calls += 1
                        if replace_calls == failed_replace:
                            raise OSError("injected publish failure")
                        os.replace(source, destination)

                    with (
                        mock.patch.object(
                            generate_ash_shaders, "SHADER_ROOT", shader_root
                        ),
                        mock.patch.object(generate_ash_shaders, "MANIFEST", manifest),
                    ):
                        with self.assertRaisesRegex(
                            RuntimeError, "restored previous artifacts"
                        ):
                            generate_ash_shaders.publish_artifacts(
                                outputs,
                                staged_manifest,
                                replace=replace_with_failure,
                            )

                    self.assertEqual(
                        {
                            filename: (shader_root / filename).read_bytes()
                            for filename in original
                        },
                        original,
                    )

    def test_recompile_check_detects_non_reproducible_spirv(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            shader_root = Path(temporary)
            for name in generate_ash_shaders.SHADERS:
                (shader_root / name).write_text(f"source:{name}", encoding="utf-8")
                (shader_root / f"{name}.spv").write_bytes(f"checked:{name}".encode())

            def compile_side_effect(
                _compiler: Path,
                source: Path,
                output: Path,
                *,
                timeout_seconds: float,
            ) -> None:
                self.assertEqual(timeout_seconds, 13.0)
                output.write_bytes(f"rebuilt:{source.name}".encode())

            with (
                mock.patch.object(generate_ash_shaders, "SHADER_ROOT", shader_root),
                mock.patch.object(
                    generate_ash_shaders,
                    "compile_shader",
                    side_effect=compile_side_effect,
                ),
            ):
                errors = generate_ash_shaders.recompile_errors(
                    Path("glslangValidator"), timeout_seconds=13.0
                )

            self.assertEqual(
                errors,
                [
                    "shader.vert.spv differs from a clean compiler regeneration",
                    "shader.frag.spv differs from a clean compiler regeneration",
                ],
            )

    def test_verify_recompile_rejects_compiler_identity_drift(self) -> None:
        with (
            mock.patch.object(
                generate_ash_shaders,
                "compiler_identity",
                return_value="Glslang Version: unexpected",
            ),
            mock.patch.object(generate_ash_shaders, "recompile_errors") as recompile,
            mock.patch("builtins.print"),
        ):
            result = generate_ash_shaders.verify(
                Path("glslangValidator"), timeout_seconds=19.0
            )

        self.assertEqual(result, 1)
        recompile.assert_not_called()

    def test_verify_recompile_accepts_byte_identical_artifacts(self) -> None:
        generator = json.loads(
            generate_ash_shaders.MANIFEST.read_text(encoding="utf-8")
        )["generator"]
        with (
            mock.patch.object(
                generate_ash_shaders,
                "compiler_identity",
                return_value=generator,
            ) as identity,
            mock.patch.object(
                generate_ash_shaders,
                "recompile_errors",
                return_value=[],
            ) as recompile,
            mock.patch("builtins.print"),
        ):
            result = generate_ash_shaders.verify(
                Path("glslangValidator"), timeout_seconds=23.0
            )

        self.assertEqual(result, 0)
        identity.assert_called_once_with(
            Path("glslangValidator"), timeout_seconds=23.0
        )
        recompile.assert_called_once_with(
            Path("glslangValidator"), timeout_seconds=23.0
        )

    def test_same_source_is_reproducible_across_checkout_paths(self) -> None:
        compiler_path = shutil.which("glslangValidator")
        if compiler_path is None:
            self.skipTest("glslangValidator is unavailable")

        source = generate_ash_shaders.SHADER_ROOT / "shader.vert"
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            outputs = []
            for root in (Path(first), Path(second)):
                copied_source = root / source.name
                copied_output = root / "shader.vert.spv"
                copied_source.write_bytes(source.read_bytes())
                generate_ash_shaders.compile_shader(
                    Path(compiler_path), copied_source, copied_output
                )
                outputs.append(hashlib.sha256(copied_output.read_bytes()).digest())

        self.assertEqual(outputs[0], outputs[1])


if __name__ == "__main__":
    unittest.main()
