import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

from _source_inventory import (  # noqa: E402
    INVENTORY_RELATIVE_PATH,
    SourceInventoryError,
    load_inventory,
)


class SourceInventoryTests(unittest.TestCase):
    def write_inventory(self, root: Path, contents: str) -> None:
        path = root / INVENTORY_RELATIVE_PATH
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")

    def test_checked_in_inventory_drives_sources_archives_and_submodules(self):
        inventory = load_inventory(REPO_ROOT)

        self.assertEqual(inventory.schema, "dear-imgui-maintained-sources-v3")
        self.assertEqual(inventory.wasm_import_module, "imgui-sys-v1")
        self.assertEqual(len(inventory.sources), 8)
        self.assertIn(
            "third-party/cimguizmo/ImGuizmo/src/ImGuizmo.cpp",
            inventory.archive_sentinels(REPO_ROOT)["dear-imguizmo-sys"],
        )
        self.assertEqual(
            [submodule.package_order for submodule in inventory.package_submodules()],
            list(range(7)),
        )

    def test_duplicate_json_keys_are_rejected_before_schema_validation(self):
        contents = (REPO_ROOT / INVENTORY_RELATIVE_PATH).read_text(encoding="utf-8")
        contents = contents.replace(
            '"schema": "dear-imgui-maintained-sources-v3",',
            '"schema": "duplicate",\n  "schema": "dear-imgui-maintained-sources-v3",',
            1,
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_inventory(root, contents)
            with self.assertRaisesRegex(SourceInventoryError, "duplicate JSON key"):
                load_inventory(root)

    def test_unknown_fields_and_unsafe_paths_are_rejected(self):
        raw = json.loads(
            (REPO_ROOT / INVENTORY_RELATIVE_PATH).read_text(encoding="utf-8")
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            unknown = dict(raw)
            unknown["unexpected"] = True
            self.write_inventory(root, json.dumps(unknown))
            with self.assertRaisesRegex(SourceInventoryError, "unexpected"):
                load_inventory(root)

            unsafe = json.loads(json.dumps(raw))
            unsafe["sources"][0]["source_root"] = "../escape"
            self.write_inventory(root, json.dumps(unsafe))
            with self.assertRaisesRegex(SourceInventoryError, "relative path"):
                load_inventory(root)

    def test_provider_exports_must_be_portable_c_symbols(self):
        raw = json.loads(
            (REPO_ROOT / INVENTORY_RELATIVE_PATH).read_text(encoding="utf-8")
        )
        raw["sources"][0]["provider"]["required_exports"] = ["igBad\";alert(1)"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_inventory(root, json.dumps(raw))
            with self.assertRaisesRegex(SourceInventoryError, "portable C symbol"):
                load_inventory(root)

    def test_package_order_must_be_contiguous_from_zero(self):
        raw = json.loads(
            (REPO_ROOT / INVENTORY_RELATIVE_PATH).read_text(encoding="utf-8")
        )
        packaged = [
            submodule
            for submodule in raw["nested_submodules"]
            if submodule["package"]
        ]
        last = max(packaged, key=lambda submodule: submodule["package_order"])
        last["package_order"] += 1

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_inventory(root, json.dumps(raw))
            with self.assertRaisesRegex(
                SourceInventoryError, "package_order values must be contiguous from zero"
            ):
                load_inventory(root)

    def test_canonical_and_alternate_paths_are_mutually_exclusive(self):
        source = load_inventory(REPO_ROOT).source_by_id("imguizmo")
        with tempfile.TemporaryDirectory() as directory:
            crate_root = Path(directory)
            canonical = crate_root / source.file("implementation").canonical.as_posix()
            alternate = crate_root / source.file("implementation").alternates[0].as_posix()

            with self.assertRaisesRegex(SourceInventoryError, "expected exactly one"):
                source.resolve_file(crate_root, "implementation")

            canonical.parent.mkdir(parents=True)
            canonical.write_text("canonical", encoding="utf-8")
            self.assertEqual(
                source.resolve_file(crate_root, "implementation"), canonical
            )

            alternate.parent.mkdir(parents=True, exist_ok=True)
            alternate.write_text("alternate", encoding="utf-8")
            with self.assertRaisesRegex(SourceInventoryError, "ambiguous"):
                source.resolve_file(crate_root, "implementation")


if __name__ == "__main__":
    unittest.main()
