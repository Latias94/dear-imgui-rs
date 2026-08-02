import importlib
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
if str(CI_DIR) not in sys.path:
    sys.path.insert(0, str(CI_DIR))

WASM_PROVIDER = importlib.import_module("verify_wasm_provider")


class WasmProviderTests(unittest.TestCase):
    def test_checked_in_import_bindings_share_inventory_provider_abi(self):
        inventory = WASM_PROVIDER.load_inventory(REPO_ROOT)
        expected_module = inventory.wasm_import_module
        module_pattern = re.compile(r'wasm_import_module = "([^"]+)"')

        binding_paths = []
        for source in inventory.sources:
            if source.provider is None:
                continue
            binding_paths.append(
                REPO_ROOT
                / Path(source.crate_root.as_posix())
                / Path(source.provider.wasm_bindings.as_posix())
            )
        binding_paths.append(REPO_ROOT / "dear-imgui-sys/src/lib.rs")

        for path in binding_paths:
            modules = set(module_pattern.findall(path.read_text(encoding="utf-8")))
            self.assertEqual(modules, {expected_module}, path)

    def test_orchestrator_checks_rust_before_building_and_requires_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inventory = root / "tools/build-support/maintained_sources.json"
            inventory.parent.mkdir(parents=True)
            inventory.write_text(
                (REPO_ROOT / "tools/build-support/maintained_sources.json").read_text(
                    encoding="utf-8"
                ),
                encoding="utf-8",
            )
            commands = []

            def runner(command, *, cwd, check, text):
                commands.append(tuple(command))
                self.assertEqual(cwd, root.resolve())
                self.assertTrue(check)
                self.assertTrue(text)
                if tuple(command) == WASM_PROVIDER.PROVIDER_COMMAND:
                    output = root / "target/web-demo"
                    output.mkdir(parents=True)
                    for name in (
                        "imgui-sys-v1.js",
                        "imgui-sys-v1.wasm",
                        "imgui-sys-v1-wrapper.js",
                        "imgui_exports.json",
                    ):
                        (output / name).write_bytes(b"artifact")
                return subprocess.CompletedProcess(command, 0)

            artifacts = WASM_PROVIDER.verify_wasm_provider(
                root,
                check_rust_route=True,
                runner=runner,
            )

            self.assertEqual(
                commands,
                [WASM_PROVIDER.RUST_ROUTE_COMMAND, WASM_PROVIDER.PROVIDER_COMMAND],
            )
            self.assertEqual(len(artifacts), 4)

    def test_orchestrator_rejects_a_missing_provider_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inventory = root / "tools/build-support/maintained_sources.json"
            inventory.parent.mkdir(parents=True)
            inventory.write_text(
                (REPO_ROOT / "tools/build-support/maintained_sources.json").read_text(
                    encoding="utf-8"
                ),
                encoding="utf-8",
            )

            with self.assertRaises(WASM_PROVIDER.WasmProviderVerificationError):
                WASM_PROVIDER.verify_wasm_provider(
                    root,
                    runner=lambda *args, **kwargs: subprocess.CompletedProcess(args, 0),
                )

    def test_orchestrator_rejects_legacy_provider_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inventory = root / "tools/build-support/maintained_sources.json"
            inventory.parent.mkdir(parents=True)
            inventory.write_text(
                (REPO_ROOT / "tools/build-support/maintained_sources.json").read_text(
                    encoding="utf-8"
                ),
                encoding="utf-8",
            )

            def runner(command, *, cwd, check, text):
                output = root / "target/web-demo"
                output.mkdir(parents=True)
                for name in (
                    "imgui-sys-v0.js",
                    "imgui-sys-v0.wasm",
                    "imgui-sys-v0-wrapper.js",
                ):
                    (output / name).write_bytes(b"legacy artifact")
                return subprocess.CompletedProcess(command, 0)

            with self.assertRaisesRegex(
                WASM_PROVIDER.WasmProviderVerificationError,
                "legacy provider artifacts are incompatible with imgui-sys-v1",
            ):
                WASM_PROVIDER.verify_wasm_provider(root, runner=runner)

    def test_orchestrator_passes_packaged_source_contract_to_xtask(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_root = root / "packaged-sources"
            inventory = root / "packaged-helper/maintained_sources.json"
            output = root / "packaged-provider-output"
            inventory.parent.mkdir(parents=True)
            inventory.write_text(
                (REPO_ROOT / "tools/build-support/maintained_sources.json").read_text(
                    encoding="utf-8"
                ),
                encoding="utf-8",
            )
            commands = []

            def runner(command, *, cwd, check, text):
                commands.append(tuple(command))
                output.mkdir(parents=True, exist_ok=True)
                for name in (
                    "imgui-sys-v1.js",
                    "imgui-sys-v1.wasm",
                    "imgui-sys-v1-wrapper.js",
                    "imgui_exports.json",
                ):
                    (output / name).write_bytes(b"artifact")
                return subprocess.CompletedProcess(command, 0)

            WASM_PROVIDER.verify_wasm_provider(
                root,
                provider_source_root=source_root,
                inventory_path=inventory,
                output_dir=output,
                runner=runner,
            )

            self.assertEqual(
                commands,
                [
                    WASM_PROVIDER.provider_command(
                        source_root=source_root,
                        inventory_path=inventory,
                        output_dir=output,
                    )
                ],
            )


if __name__ == "__main__":
    unittest.main()
