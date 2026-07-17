import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
sys.path.insert(0, str(TOOLS_DIR))

import context_binding_policy  # noqa: E402


class ContextBindingPolicyTests(unittest.TestCase):
    def test_rejects_direct_switch_in_safe_production_source(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                "fn bypass() { unsafe { sys::igSetCurrentContext(ctx) } }\n",
                encoding="utf-8",
            )

            audit = context_binding_policy.audit_sources(root, (source,), {})

            self.assertEqual(len(audit.unexpected), 1)
            self.assertEqual(audit.unexpected[0].path, "crate/src/lib.rs")
            self.assertEqual(audit.unexpected[0].line, 1)

    def test_ignores_comments_strings_and_cfg_test_modules(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                '// igSetCurrentContext in a comment\n'
                'const NAME: &str = "igSetCurrentContext";\n'
                "#[cfg(test)]\n"
                "mod tests {\n"
                "    fn raw_fixture() { unsafe { sys::igSetCurrentContext(ctx) } }\n"
                "}\n",
                encoding="utf-8",
            )

            audit = context_binding_policy.audit_sources(root, (source,), {})

            self.assertTrue(audit.passed())

    def test_allowlist_count_is_reviewed_exactly(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "binding.rs").write_text(
                "fn central() { unsafe { sys::igSetCurrentContext(ctx) } }\n",
                encoding="utf-8",
            )
            allowlist = {"crate/src/binding.rs": 1}

            passing = context_binding_policy.audit_sources(root, (source,), allowlist)
            stale = context_binding_policy.audit_sources(
                root, (source,), {"crate/src/binding.rs": 2}
            )

            self.assertTrue(passing.passed())
            self.assertEqual(
                stale.allow_count_mismatches,
                ("crate/src/binding.rs: expected 2, observed 1",),
            )

    def test_repository_safe_sources_follow_context_binding_policy(self):
        audit = context_binding_policy.audit_sources(
            REPO_ROOT,
            context_binding_policy._safe_source_roots(REPO_ROOT),
            context_binding_policy.PRODUCTION_ALLOW_COUNTS,
        )

        self.assertEqual(audit.unexpected, ())
        self.assertEqual(audit.allow_count_mismatches, ())


if __name__ == "__main__":
    unittest.main()
