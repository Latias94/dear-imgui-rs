import importlib.util
import sys
from dataclasses import replace
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLS_DIR = REPO_ROOT / "tools"
sys.path.insert(0, str(TOOLS_DIR))

import release_metadata  # noqa: E402


def load_tool(name: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS_DIR / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module

TEST_PRIVATE_PACKAGES = (
    "dear-imgui-examples",
    "dear-imgui-web-demo",
    "xtask",
)


def package(
    name: str,
    path: str,
    *,
    version: str = "0.16.0",
    publish=True,
    dependencies=(),
):
    return {
        "id": f"path+file:///repo/{path}#{name}@{version}",
        "name": name,
        "version": version,
        "manifest_path": f"/repo/{path}/Cargo.toml",
        "publish": None if publish else [],
        "dependencies": list(dependencies),
    }


def dependency(name: str, path: str, requirement: str, *, kind=None):
    return {
        "name": name,
        "req": requirement,
        "path": f"/repo/{path}",
        "kind": kind,
    }


def metadata_for(packages):
    packages = list(packages)
    private_policy = {
        item["name"]: {
            "path": Path(item["manifest_path"])
            .parent.relative_to("/repo")
            .as_posix(),
            "version": item["version"],
        }
        for item in packages
        if item.get("publish") == []
    }
    if not private_policy:
        private_policy = {
            "fixture-private": {"path": "private/fixture", "version": "0.1.0"}
        }
    return release_metadata.WorkspaceMetadata.from_json(
        {
            "workspace_root": "/repo",
            "workspace_members": [item["id"] for item in packages],
            "packages": packages,
            "metadata": {
                "dear-imgui-release": {
                    "core-package": "dear-imgui-rs",
                    "private-packages": private_policy,
                }
            },
        }
    )


def complete_release_metadata(
    *,
    version="0.16.0",
    sys_version=None,
    sys_requirement=None,
    private_versions=None,
):
    sys_version = sys_version or version
    sys_requirement = sys_requirement or release_metadata.expected_internal_requirement(
        sys_version
    )
    packages = [
        package(
            "dear-imgui-rs",
            "dear-imgui",
            version=version,
            dependencies=[
                dependency("dear-imgui-sys", "dear-imgui-sys", sys_requirement)
            ],
        ),
        package("dear-imgui-sys", "dear-imgui-sys", version=sys_version),
    ]
    packages.extend(
        package(f"release-{index}", f"release/{index}", version=version)
        for index in range(25)
    )
    private_versions = private_versions or {}
    packages.extend(
        package(
            name,
            f"private/{name}",
            version=private_versions.get(name, "0.1.0"),
            publish=False,
        )
        for name in TEST_PRIVATE_PACKAGES
    )
    metadata = metadata_for(packages)
    policy = replace(
        metadata.release_policy,
        private_packages=tuple(
            replace(package, version="0.1.0")
            for package in metadata.release_policy.private_packages
        ),
    )
    return replace(metadata, release_policy=policy)
