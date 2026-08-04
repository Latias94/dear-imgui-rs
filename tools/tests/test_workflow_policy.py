import importlib
import io
import subprocess
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))


POLICY = importlib.import_module("workflow_policy")


class RepositoryPolicyTests(unittest.TestCase):
    def git(self, root: Path, *arguments: str) -> str:
        result = subprocess.run(
            ("git", *arguments),
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        return result.stdout.strip()

    def initialized_repository(self, root: Path) -> str:
        self.git(root, "init", "--quiet")
        seed = root / "README.md"
        seed.write_text("fixture\n", encoding="utf-8")
        self.git(root, "add", "README.md")
        self.git(
            root,
            "-c",
            "user.name=Policy Test",
            "-c",
            "user.email=policy@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "seed",
        )
        return self.git(root, "rev-parse", "HEAD")

    def test_uses_staged_paths_and_excludes_real_gitlinks(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            commit = self.initialized_repository(root)
            tracked = root / "owned.cmd"
            tracked.write_text("cargo check\n", encoding="utf-8")
            (root / "local.ps1").write_text("cargo check\n", encoding="utf-8")
            self.git(root, "add", "owned.cmd")
            self.git(
                root,
                "update-index",
                "--add",
                "--cacheinfo",
                f"160000,{commit},vendor/upstream.ps1",
            )

            self.assertEqual(
                [str(violation) for violation in POLICY.check_repository(root)],
                ["owned.cmd:1: tracked maintained command script is not allowed"],
            )

    def test_cli_reports_tracked_script_paths(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.initialized_repository(root)
            script = root / "tools" / "legacy.sh"
            script.parent.mkdir()
            script.write_text("cargo check\n", encoding="utf-8")
            self.git(root, "add", "tools/legacy.sh")
            diagnostic = io.StringIO()

            with redirect_stderr(diagnostic):
                result = POLICY.main(("--repo-root", str(root)))

            self.assertEqual(result, 1)
            self.assertEqual(
                diagnostic.getvalue(),
                "tools/legacy.sh:1: tracked maintained command script is not allowed\n",
            )

    def test_rejects_an_extensionless_shell_entry_point(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.initialized_repository(root)
            script = root / "tools" / "legacy"
            script.parent.mkdir()
            script.write_text("#!/usr/bin/env bash\ncargo check\n", encoding="utf-8")
            self.git(root, "add", "tools/legacy")

            self.assertEqual(
                [str(violation) for violation in POLICY.check_repository(root)],
                ["tools/legacy:1: extensionless shell entry point is not allowed"],
            )

    def test_cli_accepts_the_current_repository(self):
        output = io.StringIO()
        with redirect_stdout(output):
            result = POLICY.main(("--check", "--repo-root", str(REPO_ROOT)))

        self.assertEqual(result, 0)
        self.assertEqual(output.getvalue(), "Workflow policy passed\n")


if __name__ == "__main__":
    unittest.main()
