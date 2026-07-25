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

    def test_ignores_cfg_all_test_modules(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                "#[cfg(all(test, feature = \"native\"))]\n"
                "mod tests {\n"
                "    fn raw_fixture() { unsafe { sys::igSetCurrentContext(ctx) } }\n"
                "}\n",
                encoding="utf-8",
            )

            audit = context_binding_policy.audit_sources(root, (source,), {})

            self.assertTrue(audit.passed())

    def test_does_not_ignore_cfg_all_not_test_modules(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                "#[cfg(all(not(test), feature = \"native\"))]\n"
                "mod production {\n"
                "    fn bypass() { unsafe { sys::igSetCurrentContext(ctx) } }\n"
                "}\n",
                encoding="utf-8",
            )

            audit = context_binding_policy.audit_sources(root, (source,), {})

            self.assertEqual(len(audit.unexpected), 1)

    def test_does_not_treat_feature_named_test_as_a_test_module(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                '#[cfg(all(feature = "test", feature = "native"))]\n'
                "mod production {\n"
                "    fn bypass() { unsafe { sys::igSetCurrentContext(ctx) } }\n"
                "}\n",
                encoding="utf-8",
            )

            audit = context_binding_policy.audit_sources(root, (source,), {})

            self.assertEqual(len(audit.unexpected), 1)

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

    def test_ignores_any_nested_cargo_target_directory_component(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "examples-ios"
            generated = source / "smoke" / "target" / "debug" / "build"
            maintained = source / "smoke" / "target-output"
            generated.mkdir(parents=True)
            maintained.mkdir(parents=True)
            rust = "fn bypass() { unsafe { sys::igSetCurrentContext(ctx) } }\n"
            (generated / "bindings.rs").write_text(rust, encoding="utf-8")
            (maintained / "lib.rs").write_text(rust, encoding="utf-8")

            audit = context_binding_policy.audit_sources(root, (source,), {})

            self.assertEqual(
                audit.unexpected,
                (
                    context_binding_policy.DirectContextSwitch(
                        path="examples-ios/smoke/target-output/lib.rs",
                        line=1,
                    ),
                ),
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
