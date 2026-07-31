import importlib
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

BUILD_GLSLANG = importlib.import_module("build_glslang")


class GlslangBuildTests(unittest.TestCase):
    def test_build_uses_the_pinned_revision_and_exports_the_compiler(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            github_env = root / "github-env"
            commands = []

            def runner(command, *, check, text):
                command = tuple(command)
                commands.append(command)
                self.assertTrue(check)
                self.assertTrue(text)
                if command[:2] == ("cmake", "--build"):
                    executable = (
                        "glslangValidator.exe"
                        if os.name == "nt"
                        else "glslangValidator"
                    )
                    compiler = (
                        root
                        / f"glslang-build-{BUILD_GLSLANG.GLSLANG_COMMIT}"
                        / "StandAlone"
                        / executable
                    )
                    compiler.parent.mkdir(parents=True)
                    compiler.write_bytes(b"compiler")
                return subprocess.CompletedProcess(command, 0)

            compiler = BUILD_GLSLANG.build_glslang(
                root,
                github_env,
                runner=runner,
            )

            self.assertEqual(len(commands), 5)
            self.assertEqual(commands[0][:2], ("git", "init"))
            self.assertIn(BUILD_GLSLANG.GLSLANG_REPOSITORY, commands[1])
            self.assertIn(BUILD_GLSLANG.GLSLANG_COMMIT, commands[1])
            self.assertIn("-DBUILD_EXTERNAL=OFF", commands[3])
            self.assertIn("-DGLSLANG_TESTS=OFF", commands[3])
            self.assertIn("glslang-standalone", commands[4])
            self.assertEqual(
                github_env.read_text(encoding="utf-8"),
                f"GLSLANG_VALIDATOR={compiler}\n",
            )

    def test_build_refuses_to_reuse_an_existing_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / f"glslang-{BUILD_GLSLANG.GLSLANG_COMMIT}").mkdir()
            with self.assertRaisesRegex(
                BUILD_GLSLANG.GlslangBuildError,
                "refusing to reuse",
            ):
                BUILD_GLSLANG.build_glslang(
                    root,
                    root / "github-env",
                    runner=lambda *args, **kwargs: subprocess.CompletedProcess(args, 0),
                )


if __name__ == "__main__":
    unittest.main()
