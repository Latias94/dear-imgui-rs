import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
sys.path.insert(0, str(TOOLS_DIR))

import aggregate_callback_abi_policy  # noqa: E402


class AggregateCallbackAbiPolicyTests(unittest.TestCase):
    def test_rejects_by_value_aggregate_parameters_and_returns(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                'unsafe extern "C" fn set_size(v: sys::ImVec2) {}\n'
                'unsafe extern "C" fn get_insets() -> sys::ImVec4_c { todo!() }\n',
                encoding="utf-8",
            )

            violations = aggregate_callback_abi_policy.audit_sources(root, (source,))

            self.assertEqual(
                [(violation.function, violation.line) for violation in violations],
                [("set_size", 1), ("get_insets", 2)],
            )

    def test_rejects_safe_and_parenthesized_aggregate_callbacks(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                'extern "C" fn safe_set_size(v: sys::ImVec2) {}\n'
                'extern "C-unwind" fn safe_get_insets() -> sys::ImVec4_c { todo!() }\n'
                'unsafe extern "C" fn grouped_set_size(v: (sys::ImVec2)) {}\n'
                'unsafe extern "C-unwind" fn grouped_get_insets() -> (sys::ImVec4_c) { todo!() }\n',
                encoding="utf-8",
            )

            violations = aggregate_callback_abi_policy.audit_sources(root, (source,))

            self.assertEqual(
                [(violation.function, violation.line) for violation in violations],
                [
                    ("safe_set_size", 1),
                    ("safe_get_insets", 2),
                    ("grouped_set_size", 3),
                    ("grouped_get_insets", 4),
                ],
            )

    def test_allows_pointer_parameters_and_ignores_tests_and_text(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "crate" / "src"
            source.mkdir(parents=True)
            (source / "lib.rs").write_text(
                '// unsafe extern "C" fn commented(v: sys::ImVec2) {}\n'
                'const TEXT: &str = "unsafe extern \\"C\\" fn text(v: ImVec2)";\n'
                'unsafe extern "C" fn set_size(v: *const sys::ImVec2) {}\n'
                'unsafe extern "C" fn get_size(out: *mut sys::ImVec2_c) {}\n'
                'unsafe extern "C" fn wrapped(v: Option<*const sys::ImVec2>) {}\n'
                '#[cfg(test)]\n'
                'mod tests {\n'
                '    unsafe extern "C" fn fixture(v: sys::ImVec2) {}\n'
                '}\n',
                encoding="utf-8",
            )

            violations = aggregate_callback_abi_policy.audit_sources(root, (source,))

            self.assertEqual(violations, ())

    def test_repository_safe_sources_do_not_cross_aggregate_c_abi_by_value(self):
        violations = aggregate_callback_abi_policy.audit_sources(
            REPO_ROOT,
            aggregate_callback_abi_policy.context_binding_policy._safe_source_roots(
                REPO_ROOT
            ),
        )

        self.assertEqual(violations, ())


if __name__ == "__main__":
    unittest.main()
