import json
import subprocess
import sys
import tempfile
import tomllib
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
        self.assertEqual(
            {source.crate_name for source in inventory.sources},
            {
                dependency.get("package", dependency_name)
                for dependency_name, dependency in tomllib.loads(
                    (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
                )["workspace"]["dependencies"].items()
                if isinstance(dependency, dict)
                and dependency.get("path")
                and dependency.get("package", dependency_name).endswith("-sys")
            },
        )
        self.assertIn(
            "third-party/cimguizmo/ImGuizmo/src/ImGuizmo.cpp",
            inventory.archive_sentinels(REPO_ROOT)["dear-imguizmo-sys"],
        )
        packaged_submodules = inventory.package_submodules()
        self.assertEqual(
            [submodule.package_order for submodule in packaged_submodules],
            list(range(len(packaged_submodules))),
        )

    def test_cte_inventory_owns_only_extension_sources_and_one_repo_bridge(self):
        inventory = load_inventory(REPO_ROOT)
        source = inventory.source_by_id("cte")

        self.assertEqual(source.crate_name, "dear-imgui-cte-sys")
        self.assertEqual(source.crate_root.as_posix(), "extensions/dear-imgui-cte-sys")
        self.assertEqual(source.source_root.as_posix(), "third-party/cimCTE")
        self.assertEqual(
            {source_file.canonical.as_posix() for source_file in source.files},
            {
                "third-party/cimCTE/cimCTE.cpp",
                "third-party/cimCTE/ImGuiColorTextEdit/TextEditor.cpp",
                "third-party/cimCTE/ImGuiColorTextEdit/TextDiff.cpp",
                "third-party/cimCTE/ImGuiColorTextEdit/example/dejavu.cpp",
                "third-party/cimCTE/ImGuiColorTextEdit/extras/TrieAutoComplete.cpp",
                "shim/cte_bridge.cpp",
            },
        )
        self.assertIn("bridge", source.native_required_files)
        self.assertIsNotNone(source.provider)
        self.assertIn("TextEditor_", source.provider.symbol_prefixes)
        self.assertIn("dear_imgui_cte_", source.provider.symbol_prefixes)
        self.assertEqual(
            [
                submodule.path.as_posix()
                for submodule in inventory.nested_submodules
                if submodule.parent == source.crate_root / source.source_root
            ],
            ["ImGuiColorTextEdit"],
        )

    def test_extension_inventory_rejects_imgui_core_translation_units(self):
        raw = json.loads(
            (REPO_ROOT / INVENTORY_RELATIVE_PATH).read_text(encoding="utf-8")
        )
        cte = next(source for source in raw["sources"] if source["id"] == "cte")
        wrapper = next(
            source_file
            for source_file in cte["files"]
            if source_file["id"] == "wrapper"
        )
        wrapper["canonical"] = "third-party/cimCTE/cimgui/cimgui.cpp"

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_inventory(root, json.dumps(raw))
            with self.assertRaisesRegex(
                SourceInventoryError, "core translation unit"
            ):
                load_inventory(root)

    def test_workspace_metadata_registers_cte_release_pair_and_feature_forwarding(self):
        result = subprocess.run(
            [
                "cargo",
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--locked",
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        packages = {
            package["name"]: package for package in json.loads(result.stdout)["packages"]
        }
        sys_package = packages["dear-imgui-cte-sys"]
        safe_package = packages["dear-imgui-cte"]

        self.assertEqual(sys_package["links"], "dear_imgui_cte")
        self.assertIn(
            "dear-imgui-sys",
            {dependency["name"] for dependency in sys_package["dependencies"]},
        )
        for feature in ("prebuilt", "build-from-source", "wasm"):
            self.assertIn(
                f"dear-imgui-cte-sys/{feature}",
                safe_package["features"][feature],
            )
            self.assertIn(
                f"dear-imgui-rs/{feature}", safe_package["features"][feature]
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
