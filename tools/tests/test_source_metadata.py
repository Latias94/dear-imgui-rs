import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
sys.path.insert(0, str(TOOLS_DIR))

import source_metadata  # noqa: E402


CIMGUI_REVISION = "1" * 40
IMGUI_REVISION = "2" * 40
EXTENSION_REVISION = "3" * 40
NESTED_EXTENSION_REVISION = "4" * 40


class SourceMetadataTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo_root = Path(self.temporary_directory.name)
        self.manifest_path = self.repo_root / source_metadata.CORE_MANIFEST_PATH
        self.cimgui_path = (
            self.repo_root / "dear-imgui-sys/third-party/cimgui"
        )
        self.imgui_path = self.cimgui_path / "imgui"
        self.imgui_path.mkdir(parents=True)
        self.write_manifest(CIMGUI_REVISION, IMGUI_REVISION)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def write_manifest(
        self,
        cimgui_revision: str,
        imgui_revision: str,
        *,
        extra: str = "",
    ) -> None:
        self.manifest_path.write_text(
            "[package]\n"
            'name = "dear-imgui-sys"\n\n'
            "[package.metadata.dear-imgui-sources]\n"
            f'cimgui-revision = "{cimgui_revision}"\n'
            f'imgui-revision = "{imgui_revision}"\n'
            f"{extra}",
            encoding="utf-8",
        )

    def fake_git(
        self,
        *,
        statuses: dict[Path, str] | None = None,
        revisions: dict[Path, str] | None = None,
        top_levels: dict[Path, Path] | None = None,
    ):
        statuses = statuses or {}
        revisions = revisions or {
            self.cimgui_path: CIMGUI_REVISION,
            self.imgui_path: IMGUI_REVISION,
        }
        top_levels = top_levels or {}

        def output(path: Path, arguments):
            if tuple(arguments) == ("rev-parse", "--show-toplevel"):
                return f"{top_levels.get(path, path).resolve()}\n"
            if arguments[0] == "status":
                self.assertEqual(
                    tuple(arguments),
                    (
                        "status",
                        "--porcelain=v1",
                        "--untracked-files=all",
                        "--ignore-submodules=none",
                    ),
                )
                return statuses.get(path, "")
            if tuple(arguments) == ("rev-parse", "HEAD"):
                return f"{revisions[path]}\n"
            self.fail(f"unexpected git command: {arguments}")

        return output

    def test_schema_allows_only_the_two_revision_keys(self):
        self.write_manifest(
            CIMGUI_REVISION,
            IMGUI_REVISION,
            extra='unexpected-revision = "3333333333333333333333333333333333333333"\n',
        )

        with self.assertRaises(source_metadata.SourceMetadataError) as raised:
            source_metadata.read_core_source_metadata(self.manifest_path)

        self.assertIn("must contain exactly", str(raised.exception))
        self.assertIn("unexpected-revision", str(raised.exception))

    def test_schema_requires_every_allowed_key(self):
        self.manifest_path.write_text(
            "[package]\n"
            'name = "dear-imgui-sys"\n\n'
            "[package.metadata.dear-imgui-sources]\n"
            f'cimgui-revision = "{CIMGUI_REVISION}"\n',
            encoding="utf-8",
        )

        with self.assertRaises(source_metadata.SourceMetadataError) as raised:
            source_metadata.read_core_source_metadata(self.manifest_path)

        self.assertIn("imgui-revision", str(raised.exception))

    def test_schema_requires_exact_40_character_hex_revisions(self):
        invalid_revisions = ("a" * 39, "a" * 41, "g" * 40)
        for revision in invalid_revisions:
            with self.subTest(revision=revision):
                self.write_manifest(revision, IMGUI_REVISION)
                with self.assertRaises(source_metadata.SourceMetadataError) as raised:
                    source_metadata.read_core_source_metadata(self.manifest_path)
                self.assertIn(
                    "exactly 40 ASCII hexadecimal characters", str(raised.exception)
                )

    def test_vendored_paths_are_fixed_by_the_shared_schema(self):
        self.assertEqual(
            [source.relative_path for source in source_metadata.CORE_SOURCE_SPECS],
            [
                Path("dear-imgui-sys/third-party/cimgui"),
                Path("dear-imgui-sys/third-party/cimgui/imgui"),
            ],
        )
        self.assertEqual(
            {source.metadata_key for source in source_metadata.CORE_SOURCE_SPECS},
            source_metadata.SOURCE_METADATA_KEYS,
        )

    def test_verification_rejects_a_path_that_falls_back_to_another_worktree(self):
        git_output = self.fake_git(top_levels={self.imgui_path: self.cimgui_path})

        with (
            patch.object(source_metadata, "_git_output", side_effect=git_output),
            self.assertRaises(source_metadata.SourceMetadataError) as raised,
        ):
            source_metadata.verify_core_source_metadata(self.repo_root)

        self.assertIn("not the expected Git worktree", str(raised.exception))
        self.assertIn(str(self.imgui_path), str(raised.exception))

    def test_verification_rejects_a_dirty_nested_submodule(self):
        git_output = self.fake_git(statuses={self.imgui_path: " M imgui.cpp\n"})

        with (
            patch.object(source_metadata, "_git_output", side_effect=git_output),
            self.assertRaises(source_metadata.SourceMetadataError) as raised,
        ):
            source_metadata.verify_core_source_metadata(self.repo_root)

        self.assertIn("Dear ImGui source tree is dirty", str(raised.exception))
        self.assertIn("imgui.cpp", str(raised.exception))

    def test_verification_rejects_head_mismatch(self):
        actual_imgui_revision = "3" * 40
        git_output = self.fake_git(
            revisions={
                self.cimgui_path: CIMGUI_REVISION,
                self.imgui_path: actual_imgui_revision,
            }
        )

        with (
            patch.object(source_metadata, "_git_output", side_effect=git_output),
            self.assertRaises(source_metadata.SourceMetadataError) as raised,
        ):
            source_metadata.verify_core_source_metadata(self.repo_root)

        self.assertIn(
            f"metadata {IMGUI_REVISION}, HEAD {actual_imgui_revision}",
            str(raised.exception),
        )

    def test_verification_returns_exact_clean_revisions(self):
        with patch.object(
            source_metadata, "_git_output", side_effect=self.fake_git()
        ):
            revisions = source_metadata.verify_core_source_metadata(self.repo_root)

        self.assertEqual(
            revisions,
            {
                "cimgui-revision": CIMGUI_REVISION,
                "imgui-revision": IMGUI_REVISION,
            },
        )

    def test_update_atomically_rewrites_only_the_shared_metadata(self):
        next_cimgui_revision = "3" * 40
        next_imgui_revision = "4" * 40
        self.write_manifest(CIMGUI_REVISION, IMGUI_REVISION, extra="\n[features]\ndefault = []\n")
        git_output = self.fake_git(
            revisions={
                self.cimgui_path: next_cimgui_revision,
                self.imgui_path: next_imgui_revision,
            }
        )

        with patch.object(source_metadata, "_git_output", side_effect=git_output):
            result = source_metadata.update_core_source_metadata(self.repo_root)

        self.assertTrue(result.changed)
        self.assertTrue(result.written)
        self.assertEqual(
            source_metadata.read_core_source_metadata(self.manifest_path),
            result.revisions,
        )
        self.assertIn("[features]\ndefault = []", self.manifest_path.read_text())
        self.assertEqual(list(self.manifest_path.parent.glob(".Cargo.toml.*.tmp")), [])

    def test_dry_run_reports_change_without_writing(self):
        original = self.manifest_path.read_text(encoding="utf-8")
        git_output = self.fake_git(
            revisions={
                self.cimgui_path: "3" * 40,
                self.imgui_path: "4" * 40,
            }
        )

        with patch.object(source_metadata, "_git_output", side_effect=git_output):
            result = source_metadata.update_core_source_metadata(
                self.repo_root, dry_run=True
            )

        self.assertTrue(result.changed)
        self.assertFalse(result.written)
        self.assertEqual(self.manifest_path.read_text(encoding="utf-8"), original)

    def test_verify_cli_uses_the_shared_verifier(self):
        revisions = {
            "cimgui-revision": CIMGUI_REVISION,
            "imgui-revision": IMGUI_REVISION,
        }
        stdout = io.StringIO()
        with (
            patch.object(
                source_metadata,
                "verify_core_source_metadata",
                return_value=revisions,
            ) as verify,
            redirect_stdout(stdout),
        ):
            result = source_metadata.main(
                ["verify", "--repo-root", str(self.repo_root)]
            )

        self.assertEqual(result, 0)
        verify.assert_called_once_with(self.repo_root.resolve())
        self.assertEqual(
            stdout.getvalue().splitlines(),
            [
                f"cimgui-revision={CIMGUI_REVISION}",
                f"imgui-revision={IMGUI_REVISION}",
            ],
        )

    def test_verify_cli_reports_structured_errors_without_traceback(self):
        stderr = io.StringIO()
        with (
            patch.object(
                source_metadata,
                "verify_core_source_metadata",
                side_effect=source_metadata.SourceMetadataError(
                    ("first failure", "second failure")
                ),
            ),
            redirect_stderr(stderr),
        ):
            result = source_metadata.main(
                ["verify", "--repo-root", str(self.repo_root)]
            )

        self.assertEqual(result, 1)
        self.assertEqual(
            stderr.getvalue().splitlines(),
            ["error: first failure", "error: second failure"],
        )


class CrateBindingSourceMetadataTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo_root = Path(self.temporary_directory.name)
        self.spec = next(
            spec
            for spec in source_metadata.BINDING_SOURCE_SPECS
            if not spec.nested_sources
        )
        self.manifest_path = self.repo_root / self.spec.manifest_path
        self.source_path = self.repo_root / self.spec.relative_path
        self.source_path.mkdir(parents=True)
        self.manifest_path.parent.mkdir(parents=True, exist_ok=True)
        self.write_manifest(EXTENSION_REVISION)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def write_manifest(self, revision: str, *, extra: str = "") -> None:
        self.manifest_path.write_text(
            "[package]\n"
            f'name = "{self.spec.crate_name}"\n\n'
            "[package.metadata.dear-imgui-binding]\n"
            f'source-revision = "{revision}"\n'
            f"{extra}",
            encoding="utf-8",
        )

    def test_registry_is_discovered_from_the_maintained_source_inventory(self):
        self.assertEqual(
            {spec.crate_name for spec in source_metadata.BINDING_SOURCE_SPECS},
            {
                source.crate_name
                for source in source_metadata.SOURCE_INVENTORY.sources
                if source.id != "core"
            },
        )
        self.assertIn(
            "dear-imgui-cte-sys",
            {spec.crate_name for spec in source_metadata.BINDING_SOURCE_SPECS},
        )

    def test_binding_metadata_is_exact_and_rejects_unknown_or_malformed_values(self):
        self.assertEqual(
            source_metadata.read_binding_source_metadata(self.manifest_path, self.spec),
            {source_metadata.BINDING_SOURCE_METADATA_KEY: EXTENSION_REVISION},
        )
        for revision, extra in [
            ("short", ""),
            (EXTENSION_REVISION, 'unexpected = "4"\n'),
        ]:
            with self.subTest(revision=revision, extra=extra):
                self.write_manifest(revision, extra=extra)
                with self.assertRaises(source_metadata.SourceMetadataError):
                    source_metadata.read_binding_source_metadata(
                        self.manifest_path, self.spec
                    )

    def test_binding_metadata_rejects_missing_and_duplicate_revisions(self):
        fixtures = (
            '[package]\nname = "missing-section"\n',
            "[package.metadata.dear-imgui-binding]\n",
            (
                "[package.metadata.dear-imgui-binding]\n"
                f'source-revision = "{EXTENSION_REVISION}"\n'
                'source-revision = "' + "4" * 40 + '"\n'
            ),
        )
        for fixture in fixtures:
            with self.subTest(fixture=fixture):
                self.manifest_path.write_text(fixture, encoding="utf-8")
                with self.assertRaises(source_metadata.SourceMetadataError):
                    source_metadata.read_binding_source_metadata(
                        self.manifest_path, self.spec
                    )

    def test_verification_rejects_dirty_or_mismatched_owning_source(self):
        def dirty_git_output(path: Path, arguments):
            self.assertEqual(path, self.source_path)
            if tuple(arguments) == ("rev-parse", "--show-toplevel"):
                return f"{self.source_path.resolve()}\n"
            if arguments[0] == "status":
                return " M generated-header.h\n"
            self.fail(f"unexpected git command: {arguments}")

        with (
            patch.object(source_metadata, "_git_output", side_effect=dirty_git_output),
            self.assertRaises(source_metadata.SourceMetadataError) as raised,
        ):
            source_metadata.verify_binding_source_metadata(self.repo_root, self.spec)
        self.assertIn("source tree is dirty", str(raised.exception))

        def mismatched_git_output(path: Path, arguments):
            self.assertEqual(path, self.source_path)
            if tuple(arguments) == ("rev-parse", "--show-toplevel"):
                return f"{self.source_path.resolve()}\n"
            if arguments[0] == "status":
                return ""
            if tuple(arguments) == ("rev-parse", "HEAD"):
                return f'{"4" * 40}\n'
            self.fail(f"unexpected git command: {arguments}")

        with (
            patch.object(
                source_metadata, "_git_output", side_effect=mismatched_git_output
            ),
            self.assertRaises(source_metadata.SourceMetadataError) as raised,
        ):
            source_metadata.verify_binding_source_metadata(self.repo_root, self.spec)
        self.assertIn("revision mismatch", str(raised.exception))

    def test_update_rewrites_only_the_owning_manifest(self):
        other_manifest = self.repo_root / "other/Cargo.toml"
        other_manifest.parent.mkdir(parents=True)
        other_manifest.write_text("untouched\n", encoding="utf-8")

        def git_output(path: Path, arguments):
            self.assertEqual(path, self.source_path)
            if tuple(arguments) == ("rev-parse", "--show-toplevel"):
                return f"{self.source_path.resolve()}\n"
            if arguments[0] == "status":
                return ""
            if tuple(arguments) == ("rev-parse", "HEAD"):
                return f'{"5" * 40}\n'
            self.fail(f"unexpected git command: {arguments}")

        with patch.object(source_metadata, "_git_output", side_effect=git_output):
            result = source_metadata.update_binding_source_metadata(
                self.repo_root, self.spec
            )

        self.assertTrue(result.changed)
        self.assertTrue(result.written)
        self.assertEqual(
            source_metadata.read_binding_source_metadata(self.manifest_path, self.spec),
            {source_metadata.BINDING_SOURCE_METADATA_KEY: "5" * 40},
        )
        self.assertEqual(other_manifest.read_text(encoding="utf-8"), "untouched\n")


class NestedCrateBindingSourceMetadataTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo_root = Path(self.temporary_directory.name)
        self.spec = next(
            spec
            for spec in source_metadata.BINDING_SOURCE_SPECS
            if spec.crate_name == "dear-imgui-cte-sys"
        )
        self.nested = self.spec.nested_sources[0]
        self.manifest_path = self.repo_root / self.spec.manifest_path
        self.source_path = self.repo_root / self.spec.relative_path
        self.nested_path = self.repo_root / self.nested.relative_path
        self.nested_path.mkdir(parents=True)
        self.manifest_path.parent.mkdir(parents=True, exist_ok=True)
        self.write_manifest(EXTENSION_REVISION, NESTED_EXTENSION_REVISION)

    def tearDown(self):
        self.temporary_directory.cleanup()

    def write_manifest(self, source_revision: str, nested_revision: str | None) -> None:
        nested_line = (
            f'{self.nested.metadata_key} = "{nested_revision}"\n'
            if nested_revision is not None
            else ""
        )
        self.manifest_path.write_text(
            "[package]\n"
            f'name = "{self.spec.crate_name}"\n\n'
            "[package.metadata.dear-imgui-binding]\n"
            f'source-revision = "{source_revision}"\n'
            f"{nested_line}",
            encoding="utf-8",
        )

    def fake_git(
        self,
        *,
        source_revision: str = EXTENSION_REVISION,
        nested_revision: str = NESTED_EXTENSION_REVISION,
        gitlink_revision: str = NESTED_EXTENSION_REVISION,
    ):
        def output(path: Path, arguments):
            if tuple(arguments) == ("rev-parse", "--show-toplevel"):
                return f"{path.resolve()}\n"
            if arguments[0] == "status":
                expected_ignore = "all" if path == self.source_path else "none"
                self.assertEqual(arguments[-1], f"--ignore-submodules={expected_ignore}")
                return ""
            if tuple(arguments) == ("rev-parse", "HEAD"):
                revision = (
                    source_revision if path == self.source_path else nested_revision
                )
                return f"{revision}\n"
            if tuple(arguments) == (
                "ls-tree",
                "HEAD",
                "--",
                self.nested.path.as_posix(),
            ):
                return (
                    f"160000 commit {gitlink_revision}\t"
                    f"{self.nested.path.as_posix()}\n"
                )
            self.fail(f"unexpected git command for {path}: {arguments}")

        return output

    def test_nested_metadata_requires_both_revisions_and_verifies_gitlink(self):
        expected = {
            source_metadata.BINDING_SOURCE_METADATA_KEY: EXTENSION_REVISION,
            self.nested.metadata_key: NESTED_EXTENSION_REVISION,
        }
        self.assertEqual(
            source_metadata.read_binding_source_metadata(
                self.manifest_path, self.spec
            ),
            expected,
        )
        with patch.object(
            source_metadata, "_git_output", side_effect=self.fake_git()
        ):
            self.assertEqual(
                source_metadata.verify_binding_source_metadata(
                    self.repo_root, self.spec
                ),
                expected,
            )

        self.write_manifest(EXTENSION_REVISION, None)
        with self.assertRaisesRegex(
            source_metadata.SourceMetadataError, self.nested.metadata_key
        ):
            source_metadata.read_binding_source_metadata(
                self.manifest_path, self.spec
            )

    def test_verification_rejects_nested_checkout_that_does_not_match_gitlink(self):
        next_nested_revision = "5" * 40
        self.write_manifest(EXTENSION_REVISION, next_nested_revision)
        with (
            patch.object(
                source_metadata,
                "_git_output",
                side_effect=self.fake_git(nested_revision=next_nested_revision),
            ),
            self.assertRaisesRegex(
                source_metadata.SourceMetadataError, "gitlink revision mismatch"
            ),
        ):
            source_metadata.verify_binding_source_metadata(
                self.repo_root, self.spec
            )

    def test_update_rewrites_wrapper_and_nested_revisions_together(self):
        next_source_revision = "5" * 40
        next_nested_revision = "6" * 40
        with patch.object(
            source_metadata,
            "_git_output",
            side_effect=self.fake_git(
                source_revision=next_source_revision,
                nested_revision=next_nested_revision,
                gitlink_revision=next_nested_revision,
            ),
        ):
            result = source_metadata.update_binding_source_metadata(
                self.repo_root, self.spec
            )

        self.assertTrue(result.changed)
        self.assertTrue(result.written)
        self.assertEqual(
            source_metadata.read_binding_source_metadata(
                self.manifest_path, self.spec
            ),
            {
                source_metadata.BINDING_SOURCE_METADATA_KEY: next_source_revision,
                self.nested.metadata_key: next_nested_revision,
            },
        )

if __name__ == "__main__":
    unittest.main()
