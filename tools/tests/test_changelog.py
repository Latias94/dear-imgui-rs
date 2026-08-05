import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tools.tests.release_test_support import load_tool


CHANGELOG = load_tool("changelog")


class ChangelogTests(unittest.TestCase):
    def test_accepts_unreleased_as_first_release_section(self):
        with TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            changelog.write_text(
                "# Changelog\n\n## [Unreleased]\n\n## [0.16.0]\n\n- Notes.\n",
                encoding="utf-8",
            )

            CHANGELOG.validate_unreleased_first(changelog)

    def test_rejects_version_before_unreleased(self):
        with TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            changelog.write_text(
                "# Changelog\n\n## [0.16.0]\n\n- Notes.\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "must keep .*Unreleased"):
                CHANGELOG.validate_unreleased_first(changelog)


if __name__ == "__main__":
    unittest.main()
