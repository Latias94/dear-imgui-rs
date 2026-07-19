import importlib
import io
import subprocess
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path, PurePosixPath
from tempfile import TemporaryDirectory


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))


POLICY = importlib.import_module("workflow_policy")


class WorkflowTextPolicyTests(unittest.TestCase):
    def violations(self, source: str, name: str = ".github/workflows/ci.yml"):
        return POLICY.check_workflow_text(PurePosixPath(name), source)

    def test_rejects_literal_run_blocks_with_stable_location(self):
        for indicator in ("|", "|-", "|+", "|2", "|-2", "|2-"):
            with self.subTest(indicator=indicator):
                violations = self.violations(
                    f"name: CI\nsteps:\n  - name: bad\n    run: {indicator}\n      cargo check\n"
                )

                self.assertEqual(
                    [str(violation) for violation in violations],
                    [
                        ".github/workflows/ci.yml:4: literal multi-line run blocks are not allowed"
                    ],
                )

    def test_rejects_explicit_runner_shells(self):
        for shell in ("bash", "pwsh", "powershell", "cmd", "bash {0}"):
            with self.subTest(shell=shell):
                violations = self.violations(
                    f"steps:\n  - shell: {shell}\n    run: cargo check\n"
                )
                self.assertEqual(len(violations), 1)
                self.assertEqual(violations[0].line, 2)
                self.assertIn("explicit runner shell", violations[0].message)

    def test_rejects_folded_shell_control_flow_for_each_runner_family(self):
        fixtures = {
            "bash": "run: >-\n  if test -f Cargo.toml; then cargo check; fi\n",
            "powershell": "run: >-\n  if (Test-Path Cargo.toml) { cargo check }\n",
            "powershell-tight": "run: >-\n  if(Test-Path Cargo.toml){ cargo check }\n",
            "cmd": "run: >-\n  if exist Cargo.toml cargo check\n",
            "separator": "run: cargo check && cargo test\n",
            "quoted-separator": 'run: "cargo check && cargo test"\n',
            "compact-pipeline": "run: cargo check|tee cargo-check.log\n",
        }
        for family, source in fixtures.items():
            with self.subTest(family=family):
                violations = self.violations(source)
                self.assertEqual(len(violations), 1)
                self.assertIn("shell control flow", violations[0].message)

    def test_allows_folded_single_commands_and_one_line_environment_handoffs(self):
        source = """steps:
  - name: Check packages
    run: >-
      cargo check
      --workspace
      --all-targets
  - name: Export path
    run: echo "PKG_CONFIG_PATH=$(brew --prefix)/lib/pkgconfig" >> "$GITHUB_ENV"
"""

        self.assertEqual(self.violations(source), ())

    def test_scans_yaml_as_well_as_yml(self):
        violations = self.violations(
            "steps:\n  - run: |\n      cargo check\n",
            ".github/workflows/ci.yaml",
        )

        self.assertEqual(violations[0].path.as_posix(), ".github/workflows/ci.yaml")


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

            violations = POLICY.check_repository(root)

            self.assertEqual(
                [str(violation) for violation in violations],
                ["owned.cmd:1: tracked maintained command script is not allowed"],
            )

    def test_cli_reports_each_workflow_path_and_line(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.initialized_repository(root)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            (workflows / "first.yml").write_text(
                "steps:\n  - run: |\n      cargo check\n", encoding="utf-8"
            )
            (workflows / "second.yaml").write_text(
                "steps:\n  - shell: powershell\n    run: cargo check\n",
                encoding="utf-8",
            )
            self.git(root, "add", ".github/workflows/first.yml")
            self.git(root, "add", ".github/workflows/second.yaml")
            diagnostic = io.StringIO()

            with redirect_stderr(diagnostic):
                result = POLICY.main(("--repo-root", str(root)))

            self.assertEqual(result, 1)
            self.assertEqual(
                diagnostic.getvalue().splitlines(),
                [
                    ".github/workflows/first.yml:2: literal multi-line run blocks are not allowed",
                    ".github/workflows/second.yaml:2: explicit runner shell 'powershell' is not allowed",
                ],
            )

    def test_cli_accepts_explicit_check_mode(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.initialized_repository(root)
            output = io.StringIO()

            with redirect_stdout(output):
                result = POLICY.main(("--check", "--repo-root", str(root)))

            self.assertEqual(result, 0)
            self.assertEqual(output.getvalue(), "Workflow policy passed\n")

    def test_current_repository_satisfies_the_policy(self):
        self.assertEqual(POLICY.check_repository(REPO_ROOT), ())


if __name__ == "__main__":
    unittest.main()
