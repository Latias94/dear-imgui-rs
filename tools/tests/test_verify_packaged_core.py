import importlib
import io
import json
import os
import subprocess
import sys
import tarfile
import tomllib
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_DIR = REPO_ROOT / "tools" / "ci"
TOOLS_DIR = REPO_ROOT / "tools"
for import_path in (CI_DIR, TOOLS_DIR):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))
ARCHIVE = importlib.import_module("_archive")
PREBUILT = importlib.import_module("_prebuilt")
PROCESS = importlib.import_module("_process")
SOURCE_PACKAGES = importlib.import_module("_source_packages")
VERIFICATION = importlib.import_module("_verification")
CLI = importlib.import_module("verify_packaged_core")


PROFILE_FEATURES = {
    "normal": "platform-io-aggregate-hooks-v2,safe-demo-font-boundary-v1,wchar32",
    "freetype": (
        "freetype,platform-io-aggregate-hooks-v2,safe-demo-font-boundary-v1,wchar32"
    ),
    "stack-layout": (
        "platform-io-aggregate-hooks-v2,safe-demo-font-boundary-v1,stack-layout,wchar32"
    ),
    "stack-layout-freetype": (
        "freetype,platform-io-aggregate-hooks-v2,safe-demo-font-boundary-v1,stack-layout,wchar32"
    ),
}
CANDIDATE_SHA = "cccccccccccccccccccccccccccccccccccccccc"
SOURCE_CONTRACT_HASH = "fnv1a64:fedcba9876543210"


def write_archive(path: Path, members: dict[str, bytes]) -> None:
    with tarfile.open(path, "w:gz") as archive:
        for name, content in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(content)
            archive.addfile(info, io.BytesIO(content))


def write_special_archive(
    path: Path, name: str, member_type: bytes, *, linkname: str = ""
) -> None:
    with tarfile.open(path, "w:gz") as archive:
        info = tarfile.TarInfo(name)
        info.type = member_type
        info.linkname = linkname
        archive.addfile(info)


def write_prebuilt_archive(
    directory: Path,
    profile: str,
    *,
    target: str = "x86_64-unknown-linux-gnu",
    crt: str = "",
    suffix: str = "",
    candidate_sha: str | None = CANDIDATE_SHA,
    version: str = "0.16.0",
    artifact_features: str | None = None,
) -> Path:
    path = directory / f"dear-imgui-{profile}{suffix}.tar.gz"
    selected_features = (
        artifact_features
        if artifact_features is not None
        else PROFILE_FEATURES[profile]
    )
    manifest = "\n".join(
        (
            "dear-imgui prebuilt",
            f"version={version}",
            *(
                (f"candidate_sha={candidate_sha}",)
                if candidate_sha is not None
                else ()
            ),
            f"target={target}",
            "link=static",
            f"crt={crt}",
            f"features={selected_features}",
            "cimgui_revision=1261b231939fc210032f30c4ee8a8f0440372237",
            "imgui_revision=b61e56346a92cfcaf1f43a545ca37b0b32239654",
            "binding_spec_hash=fnv1a64:0123456789abcdef",
            f"source_contract_hash={SOURCE_CONTRACT_HASH}",
            "",
        )
    ).encode()
    write_archive(path, {"manifest.txt": manifest, "lib/.keep": b""})
    return path


def write_extension_prebuilt_archive(
    directory: Path,
    spec: PREBUILT.ExtensionSpec,
    profile: str,
    core_archive: Path,
    *,
    candidate_sha: str = CANDIDATE_SHA,
    overrides: dict[str, str] | None = None,
    filename: str | None = None,
) -> Path:
    core_fields = PREBUILT._read_prebuilt_manifest(core_archive)
    features = PREBUILT.extension_artifact_features(spec, profile)
    archive_name = PREBUILT._expected_extension_archive_name(
        spec,
        core_fields["version"],
        core_fields["target"],
        core_fields["crt"],
        features,
    )
    fields = {
        "version": core_fields["version"],
        "candidate_sha": candidate_sha,
        "target": core_fields["target"],
        "link": "static",
        "crt": core_fields["crt"],
        "features": ",".join(features),
        "extension": spec.extension_id,
        "safe_crate": spec.safe_crate,
        "library": spec.library_name,
        "archive": filename or archive_name,
        "core_artifact_identity": PREBUILT.core_artifact_identity(
            core_fields, core_archive
        ),
        "extension_binding_identity": (
            PREBUILT.expected_extension_binding_identity(REPO_ROOT, spec)
        ),
    }
    if overrides:
        fields.update(overrides)
    manifest = "\n".join(
        (
            f"{spec.sys_crate} prebuilt",
            *(f"{key}={value}" for key, value in fields.items()),
            "",
        )
    ).encode()
    path = directory / (filename or archive_name)
    write_archive(path, {"manifest.txt": manifest, "lib/.keep": b""})
    return path


def write_base_extension_matrix(
    directory: Path, *, version: str = "0.16.0"
) -> dict[str, Path]:
    core = {
        profile: write_prebuilt_archive(directory, profile, version=version)
        for profile in ("normal", "stack-layout")
    }
    for spec in PREBUILT.EXTENSION_SPECS:
        write_extension_prebuilt_archive(directory, spec, "normal", core["normal"])
        if "stack-layout" in spec.profiles:
            write_extension_prebuilt_archive(
                directory, spec, "stack-layout", core["stack-layout"]
            )
    return core


class PrebuiltArchiveSelectionTests(unittest.TestCase):
    def test_selects_each_required_profile_exactly_once(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            expected = {
                profile: write_prebuilt_archive(package_dir, profile)
                for profile in PROFILE_FEATURES
            }

            selected = PREBUILT.select_core_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "",
                profile_scope="all",
            )

        self.assertEqual(selected, expected)

    def test_filters_target_and_crt_before_enforcing_uniqueness(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            expected = {}
            for profile in ("normal", "stack-layout"):
                expected[profile] = write_prebuilt_archive(
                    package_dir, profile, crt="static"
                )
                write_prebuilt_archive(
                    package_dir,
                    profile,
                    target="aarch64-apple-darwin",
                    crt="static",
                    suffix="-other-target",
                )
                write_prebuilt_archive(
                    package_dir,
                    profile,
                    crt="dynamic",
                    suffix="-other-crt",
                )

            selected = PREBUILT.select_core_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "static",
                profile_scope="base",
            )

        self.assertEqual(selected, expected)

    def test_rejects_an_unknown_matching_profile(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            for profile in ("normal", "stack-layout"):
                write_prebuilt_archive(package_dir, profile)
            write_archive(
                package_dir / "dear-imgui-unknown.tar.gz",
                {
                    "manifest.txt": (
                        b"dear-imgui prebuilt\n"
                        b"version=0.16.0\n"
                        b"candidate_sha=cccccccccccccccccccccccccccccccccccccccc\n"
                        b"target=x86_64-unknown-linux-gnu\n"
                        b"link=static\ncrt=\nfeatures=unknown-feature\n"
                        b"cimgui_revision=1261b231939fc210032f30c4ee8a8f0440372237\n"
                        b"imgui_revision=b61e56346a92cfcaf1f43a545ca37b0b32239654\n"
                        b"binding_spec_hash=fnv1a64:0123456789abcdef\n"
                        b"source_contract_hash=fnv1a64:fedcba9876543210\n"
                    )
                },
            )

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "unsupported dear_imgui artifact profile",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )

    def test_rejects_legacy_aggregate_hook_profile_before_link(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            write_prebuilt_archive(
                package_dir,
                "normal",
                artifact_features=(
                    "platform-io-aggregate-hooks,"
                    "safe-demo-font-boundary-v1,wchar32"
                ),
            )

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "unsupported dear_imgui artifact profile",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )

    def test_core_profile_hash_matches_the_rust_contract_vector(self):
        with TemporaryDirectory() as temporary:
            archive = write_prebuilt_archive(
                Path(temporary),
                "normal",
                target="x86_64-pc-windows-msvc",
                crt="md",
            )
            fields = PREBUILT._read_prebuilt_manifest(archive)

            self.assertEqual(
                PREBUILT.core_artifact_profile_hash(fields, archive),
                "fnv1a64:81e611164024011c",
            )

    def test_core_identity_anchors_candidate_and_rejects_legacy_manifests(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            archive = write_prebuilt_archive(package_dir, "normal")
            fields = PREBUILT._read_prebuilt_manifest(archive)
            identity = PREBUILT.core_artifact_identity(
                fields, archive, CANDIDATE_SHA
            )

            other = dict(fields)
            other["candidate_sha"] = "dddddddddddddddddddddddddddddddddddddddd"
            self.assertNotEqual(
                identity,
                PREBUILT.core_artifact_identity(other, archive),
            )
            with self.assertRaisesRegex(
                PREBUILT.VerificationError, "candidate_sha mismatch"
            ):
                PREBUILT.core_artifact_identity(
                    other, archive, CANDIDATE_SHA
                )

            legacy = write_prebuilt_archive(
                package_dir, "stack-layout", candidate_sha=None
            )
            with self.assertRaisesRegex(
                PREBUILT.VerificationError, "missing=candidate_sha"
            ):
                PREBUILT.core_artifact_identity(
                    PREBUILT._read_prebuilt_manifest(legacy), legacy
                )

            old_contract = write_prebuilt_archive(package_dir, "freetype")
            old_fields = PREBUILT._read_prebuilt_manifest(old_contract)
            del old_fields["source_contract_hash"]
            with self.assertRaisesRegex(
                PREBUILT.VerificationError, "missing=source_contract_hash"
            ):
                PREBUILT.core_artifact_identity(old_fields, old_contract)


class ExtensionArchiveSelectionTests(unittest.TestCase):
    def test_selects_all_six_safe_extensions_and_exact_sys_artifacts(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            core = write_base_extension_matrix(package_dir)
            selected = PREBUILT.select_extension_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "",
                CANDIDATE_SHA,
                REPO_ROOT,
                core,
                profile_scope="base",
            )

        expected = {
            (spec.extension_id, profile)
            for spec in PREBUILT.EXTENSION_SPECS
            for profile in ("normal", "stack-layout")
            if profile in spec.profiles
        }
        self.assertEqual(set(selected), expected)
        self.assertEqual(len(selected), 7)

    def test_prerelease_version_is_preserved_in_extension_archive_identity(self):
        version = "0.16.0-alpha.1"
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            core = write_base_extension_matrix(package_dir, version=version)
            selected = PREBUILT.select_extension_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "",
                CANDIDATE_SHA,
                REPO_ROOT,
                core,
                profile_scope="base",
            )

            for (extension, profile), archive in selected.items():
                spec = PREBUILT.EXTENSION_BY_ID[extension]
                features = PREBUILT.extension_artifact_features(spec, profile)
                self.assertEqual(
                    archive.name,
                    PREBUILT._expected_extension_archive_name(
                        spec,
                        version,
                        "x86_64-unknown-linux-gnu",
                        "",
                        features,
                    ),
                )
                self.assertEqual(
                    PREBUILT._read_prebuilt_manifest(archive)["version"], version
                )

    def test_ignores_other_target_profile_and_candidate_routes(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            core = write_base_extension_matrix(package_dir)
            foreign_core = write_prebuilt_archive(
                package_dir,
                "normal",
                target="aarch64-unknown-linux-gnu",
                suffix="-foreign",
                candidate_sha="dddddddddddddddddddddddddddddddddddddddd",
                version="0.15.0",
            )
            write_extension_prebuilt_archive(
                package_dir,
                PREBUILT.EXTENSION_SPECS[0],
                "normal",
                foreign_core,
                candidate_sha="dddddddddddddddddddddddddddddddddddddddd",
            )

            selected_core = PREBUILT.select_core_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "",
                profile_scope="base",
                candidate_sha=CANDIDATE_SHA,
            )
            selected_extensions = PREBUILT.select_extension_prebuilt_archives(
                package_dir,
                "x86_64-unknown-linux-gnu",
                "",
                CANDIDATE_SHA,
                REPO_ROOT,
                selected_core,
                profile_scope="base",
            )

        self.assertEqual(selected_core, core)
        self.assertEqual(len(selected_extensions), 7)

    def test_rejects_missing_and_alternate_duplicate_extension_archives(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            core = write_base_extension_matrix(package_dir)
            missing = PREBUILT.EXTENSION_SPECS[0]
            expected_name = PREBUILT._expected_extension_archive_name(
                missing,
                "0.16.0",
                "x86_64-unknown-linux-gnu",
                "",
                ("wchar32",),
            )
            (package_dir / expected_name).unlink()
            with self.assertRaisesRegex(
                PREBUILT.VerificationError, "expected exactly one implot normal archive"
            ):
                PREBUILT.select_extension_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    CANDIDATE_SHA,
                    REPO_ROOT,
                    core,
                    profile_scope="base",
                )


class ExtensionFeatureRouteTests(unittest.TestCase):
    def test_all_safe_extensions_forward_both_native_build_routes(self):
        workspace = tomllib.loads(REPO_ROOT.joinpath("Cargo.toml").read_text(encoding="utf-8"))
        catalog = workspace["workspace"]["dependencies"]
        self.assertFalse(catalog["dear-imgui-rs"]["default-features"])
        self.assertFalse(catalog["dear-imgui-sys"]["default-features"])

        for spec in PREBUILT.EXTENSION_SPECS:
            with self.subTest(extension=spec.extension_id):
                self.assertFalse(catalog[spec.sys_crate]["default-features"])
                manifest = tomllib.loads(
                    REPO_ROOT.joinpath(
                        "extensions", spec.safe_crate, "Cargo.toml"
                    ).read_text(encoding="utf-8")
                )
                features = manifest["features"]
                self.assertEqual(
                    features["prebuilt"],
                    ["dear-imgui-rs/prebuilt", f"{spec.sys_crate}/prebuilt"],
                )
                self.assertEqual(
                    features["build-from-source"],
                    [
                        "dear-imgui-rs/build-from-source",
                        f"{spec.sys_crate}/build-from-source",
                    ],
                )
                self.assertTrue(
                    manifest["dependencies"]["dear-imgui-rs"]["workspace"]
                )
                self.assertTrue(
                    manifest["dependencies"][spec.sys_crate]["workspace"]
                )

    def test_supported_wasm_routes_remain_explicit_and_node_editor_native_only(self):
        for spec in PREBUILT.EXTENSION_SPECS:
            manifest = tomllib.loads(
                REPO_ROOT.joinpath(
                    "extensions", spec.safe_crate, "Cargo.toml"
                ).read_text(encoding="utf-8")
            )
            features = manifest["features"]
            if spec.extension_id == "node-editor":
                self.assertNotIn("wasm", features)
            else:
                self.assertEqual(
                    features["wasm"],
                    ["dear-imgui-rs/wasm", f"{spec.sys_crate}/wasm"],
                )

    def test_workspace_inheritance_blocks_a_hypothetical_future_sys_default(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            safe = root / "safe"
            sys_crate = root / "sys"
            safe.joinpath("src").mkdir(parents=True)
            sys_crate.joinpath("src").mkdir(parents=True)
            root.joinpath("Cargo.toml").write_text(
                """[workspace]
resolver = "2"
members = ["safe", "sys"]

[workspace.dependencies]
fixture-sys = { path = "sys", default-features = false }
""",
                encoding="utf-8",
            )
            safe.joinpath("Cargo.toml").write_text(
                """[package]
name = "fixture-safe"
version = "0.0.0"
edition = "2024"

[dependencies]
fixture-sys.workspace = true
""",
                encoding="utf-8",
            )
            sys_crate.joinpath("Cargo.toml").write_text(
                """[package]
name = "fixture-sys"
version = "0.0.0"
edition = "2024"

[features]
default = ["future-default"]
future-default = []
""",
                encoding="utf-8",
            )
            safe.joinpath("src/lib.rs").write_text("", encoding="utf-8")
            sys_crate.joinpath("src/lib.rs").write_text("", encoding="utf-8")
            result = subprocess.run(
                (
                    "cargo",
                    "metadata",
                    "--no-deps",
                    "--format-version",
                    "1",
                    "--manifest-path",
                    root / "Cargo.toml",
                ),
                check=True,
                capture_output=True,
                text=True,
            )
            metadata = json.loads(result.stdout)
            safe_package = next(
                package
                for package in metadata["packages"]
                if package["name"] == "fixture-safe"
            )
            dependency = next(
                dependency
                for dependency in safe_package["dependencies"]
                if dependency["name"] == "fixture-sys"
            )
            self.assertFalse(dependency["uses_default_features"])

        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            core = write_base_extension_matrix(package_dir)
            spec = PREBUILT.EXTENSION_SPECS[0]
            write_extension_prebuilt_archive(
                package_dir,
                spec,
                "normal",
                core["normal"],
                filename="dear-implot-prebuilt-duplicate.tar.gz",
            )
            with self.assertRaisesRegex(
                PREBUILT.VerificationError, "extension archive identity mismatch"
            ):
                PREBUILT.select_extension_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    CANDIDATE_SHA,
                    REPO_ROOT,
                    core,
                    profile_scope="base",
                )

    def test_rejects_every_extension_identity_mismatch(self):
        mismatches = {
            "candidate_sha": "dddddddddddddddddddddddddddddddddddddddd",
            "extension": "test-engine",
            "safe_crate": "dear-imgui-test-engine",
            "library": "foreign_library",
            "core_artifact_identity": "fnv1a64:1111111111111111",
            "extension_binding_identity": "fnv1a64:2222222222222222",
            "features": "stack-layout,wchar32",
        }
        for field, value in mismatches.items():
            with self.subTest(field=field), TemporaryDirectory() as temporary:
                package_dir = Path(temporary)
                core = write_base_extension_matrix(package_dir)
                spec = PREBUILT.EXTENSION_SPECS[0]
                archive_name = PREBUILT._expected_extension_archive_name(
                    spec,
                    "0.16.0",
                    "x86_64-unknown-linux-gnu",
                    "",
                    ("wchar32",),
                )
                write_extension_prebuilt_archive(
                    package_dir,
                    spec,
                    "normal",
                    core["normal"],
                    overrides={field: value},
                    filename=archive_name,
                )
                with self.assertRaises(PREBUILT.VerificationError):
                    PREBUILT.select_extension_prebuilt_archives(
                        package_dir,
                        "x86_64-unknown-linux-gnu",
                        "",
                        CANDIDATE_SHA,
                        REPO_ROOT,
                        core,
                        profile_scope="base",
                    )

    def test_requires_an_explicit_valid_candidate_sha(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            core = write_base_extension_matrix(package_dir)
            with self.assertRaisesRegex(
                PREBUILT.VerificationError, "exactly 40 hexadecimal"
            ):
                PREBUILT.select_extension_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    "ambient-or-empty",
                    REPO_ROOT,
                    core,
                    profile_scope="base",
                )

    def test_rejects_an_archive_without_a_root_manifest(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            write_archive(
                package_dir / "dear-imgui-missing-manifest.tar.gz",
                {"nested/manifest.txt": b"not at root"},
            )

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "must contain exactly one root manifest.txt",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )

    def test_rejects_duplicate_required_profile(self):
        with TemporaryDirectory() as temporary:
            package_dir = Path(temporary)
            write_prebuilt_archive(package_dir, "normal")
            write_prebuilt_archive(package_dir, "normal", suffix="-duplicate")
            write_prebuilt_archive(package_dir, "stack-layout")

            with self.assertRaisesRegex(
                PREBUILT.VerificationError,
                "expected exactly one normal archive",
            ):
                PREBUILT.select_core_prebuilt_archives(
                    package_dir,
                    "x86_64-unknown-linux-gnu",
                    "",
                    profile_scope="base",
                )


class ArchiveSafetyTests(unittest.TestCase):
    def test_safe_extract_rejects_parent_directory_traversal(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "malicious.tar.gz"
            destination = root / "output"
            write_archive(archive, {"../escaped.txt": b"no"})

            with self.assertRaisesRegex(
                ARCHIVE.VerificationError, "unsafe archive member"
            ):
                ARCHIVE.safe_extract_tar(archive, destination)

            self.assertFalse((root / "escaped.txt").exists())

    def test_safe_extract_rejects_absolute_drive_and_backslash_paths(self):
        unsafe_names = (
            "/absolute.txt",
            "C:/drive-qualified.txt",
            "C:\\backslash-drive.txt",
            "nested\\backslash.txt",
        )
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, unsafe_name in enumerate(unsafe_names):
                with self.subTest(name=unsafe_name):
                    archive = root / f"malicious-{index}.tar.gz"
                    write_archive(archive, {unsafe_name: b"no"})
                    with self.assertRaisesRegex(
                        ARCHIVE.VerificationError, "unsafe archive member"
                    ):
                        ARCHIVE.safe_extract_tar(archive, root / f"output-{index}")

    def test_safe_extract_rejects_symbolic_and_hard_links(self):
        link_types = (tarfile.SYMTYPE, tarfile.LNKTYPE)
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, member_type in enumerate(link_types):
                with self.subTest(member_type=member_type):
                    archive = root / f"link-{index}.tar.gz"
                    write_special_archive(
                        archive,
                        "link",
                        member_type,
                        linkname="../outside",
                    )
                    with self.assertRaisesRegex(
                        ARCHIVE.VerificationError, "unsafe archive link"
                    ):
                        ARCHIVE.safe_extract_tar(archive, root / f"output-{index}")

    def test_safe_extract_rejects_special_members(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "fifo.tar.gz"
            write_special_archive(archive, "named-pipe", tarfile.FIFOTYPE)

            with self.assertRaisesRegex(
                ARCHIVE.VerificationError, "unsupported archive member type"
            ):
                ARCHIVE.safe_extract_tar(archive, root / "output")

    def test_source_archive_requires_every_configured_sentinel(self):
        with TemporaryDirectory() as temporary:
            archive_dir = Path(temporary)
            write_archive(
                archive_dir / "example-sys-1.2.3.crate",
                {"example-sys-1.2.3/src/native.cpp": b"source"},
            )
            packages = (ARCHIVE.PackageRecord("example-sys", Path("example"), "1.2.3"),)
            sentinels = {"example-sys": ("src/native.cpp", "src/missing.cpp")}

            with self.assertRaisesRegex(
                ARCHIVE.VerificationError, "missing native source sentinel"
            ):
                ARCHIVE.verify_source_archives(
                    archive_dir, packages, sys_sentinels=sentinels
                )

    def test_source_archive_rejects_git_metadata(self):
        with TemporaryDirectory() as temporary:
            archive_dir = Path(temporary)
            write_archive(
                archive_dir / "example-sys-1.2.3.crate",
                {
                    "example-sys-1.2.3/src/native.cpp": b"source",
                    "example-sys-1.2.3/.git/config": b"metadata",
                },
            )
            packages = (ARCHIVE.PackageRecord("example-sys", Path("example"), "1.2.3"),)

            with self.assertRaisesRegex(ARCHIVE.VerificationError, r"\.git entry"):
                ARCHIVE.verify_source_archives(
                    archive_dir,
                    packages,
                    sys_sentinels={"example-sys": ("src/native.cpp",)},
                )


class SharedHelperTests(unittest.TestCase):
    def test_command_runner_can_explicitly_accept_any_return_code(self):
        completed = subprocess.CompletedProcess(("tool",), returncode=-9)
        with patch.object(PROCESS.subprocess, "run", return_value=completed):
            result = PROCESS.run(("tool",), accepted_returncodes=None)

        self.assertIs(result, completed)

    def test_temporary_workspace_uses_and_cleans_runner_temp(self):
        with TemporaryDirectory() as temporary:
            runner_temp = Path(temporary)
            with patch.dict(
                os.environ, {"RUNNER_TEMP": os.fspath(runner_temp)}, clear=False
            ):
                with VERIFICATION.temporary_workspace("package-test.") as workspace:
                    self.assertEqual(workspace.parent, runner_temp)
                    self.assertTrue(workspace.is_dir())
                self.assertFalse(workspace.exists())

    def test_temporary_workspace_rejects_an_invalid_runner_temp(self):
        with TemporaryDirectory() as temporary:
            missing = Path(temporary) / "missing"
            with patch.dict(
                os.environ, {"RUNNER_TEMP": os.fspath(missing)}, clear=False
            ):
                with self.assertRaisesRegex(
                    VERIFICATION.VerificationError,
                    "RUNNER_TEMP is not a directory",
                ):
                    with VERIFICATION.temporary_workspace("package-test."):
                        self.fail(
                            "invalid RUNNER_TEMP unexpectedly created a workspace"
                        )


class ProfileMismatchTests(unittest.TestCase):
    def test_accepts_only_the_strict_profile_diagnostic(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"),
            returncode=101,
            stdout="error: selected an incompatible dear_imgui artifact\n",
        )

        PREBUILT.verify_profile_mismatch_result("normal-with-stack", result)

    def test_rejects_an_unexpected_success(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"), returncode=0, stdout=""
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError, "profile mismatch unexpectedly succeeded"
        ):
            PREBUILT.verify_profile_mismatch_result("normal-with-stack", result)

    def test_rejects_a_failure_without_the_strict_diagnostic(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"), returncode=101, stdout="different failure\n"
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError,
            "failed without the strict artifact profile diagnostic",
        ):
            PREBUILT.verify_profile_mismatch_result("normal-with-stack", result)


class CandidateMismatchTests(unittest.TestCase):
    def test_accepts_rejection_before_metadata_export(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"),
            returncode=101,
            stdout="error: artifact candidate mismatch: expected 0, found 1\n",
        )

        PREBUILT.verify_candidate_mismatch_result("core-prebuilt", result)

    def test_rejects_an_unexpected_success(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"), returncode=0, stdout=""
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError, "candidate mismatch unexpectedly succeeded"
        ):
            PREBUILT.verify_candidate_mismatch_result("core-prebuilt", result)

    def test_rejects_a_failure_without_the_candidate_diagnostic(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"), returncode=101, stdout="different failure\n"
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError,
            "failed without the strict artifact candidate diagnostic",
        ):
            PREBUILT.verify_candidate_mismatch_result("core-prebuilt", result)

    def test_rejects_metadata_emitted_before_candidate_failure(self):
        result = subprocess.CompletedProcess(
            ("cargo", "check"),
            returncode=101,
            stdout=(
                "cargo:ARTIFACT_IDENTITY_HASH=fnv1a64:0000000000000000\n"
                "error: artifact candidate mismatch: expected 0, found 1\n"
            ),
        )

        with self.assertRaisesRegex(
            PREBUILT.VerificationError,
            "emitted validated metadata before candidate rejection",
        ):
            PREBUILT.verify_candidate_mismatch_result("core-prebuilt", result)


class ConsumerContractTests(unittest.TestCase):
    def test_consumer_manifest_round_trips_windows_paths_and_profile_features(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source root"
            source_root.mkdir()
            source_root.joinpath("dear-imgui").mkdir()
            source_root.joinpath("Cargo.lock").write_text(
                "version = 4\n", encoding="utf-8"
            )
            destination = root / "consumer"

            PREBUILT.write_prebuilt_consumer(
                destination, source_root, "stack-layout-freetype"
            )

            manifest = tomllib.loads(
                destination.joinpath("Cargo.toml").read_text(encoding="utf-8")
            )
            dependency = manifest["dependencies"]["dear-imgui-rs"]
            self.assertEqual(
                Path(dependency["path"]), (source_root / "dear-imgui").resolve()
            )
            self.assertFalse(dependency["default-features"])
            self.assertEqual(
                dependency["features"], ["prebuilt", "stack-layout", "freetype"]
            )

    def test_extension_consumer_uses_safe_and_exact_matching_sys_crates(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source root"
            spec = next(
                spec
                for spec in PREBUILT.EXTENSION_SPECS
                if spec.extension_id == "node-editor"
            )
            (source_root / "extensions" / spec.safe_crate).mkdir(parents=True)
            (source_root / "extensions" / spec.sys_crate).mkdir(parents=True)
            source_root.joinpath("Cargo.lock").write_text(
                "version = 4\n", encoding="utf-8"
            )
            destination = root / "consumer"

            PREBUILT.write_extension_prebuilt_consumer(
                destination,
                source_root,
                spec,
                "stack-layout-freetype",
            )

            manifest = tomllib.loads(
                destination.joinpath("Cargo.toml").read_text(encoding="utf-8")
            )
            safe = manifest["dependencies"][spec.safe_crate]
            sys_dependency = manifest["dependencies"][spec.sys_crate]
            self.assertFalse(safe["default-features"])
            self.assertEqual(
                safe["features"], ["prebuilt", "freetype", "blueprints"]
            )
            self.assertFalse(sys_dependency["default-features"])
            self.assertNotIn("features", sys_dependency)
            source = destination.joinpath("src/main.rs").read_text(encoding="utf-8")
            self.assertIn("use dear_node_editor as _;", source)
            self.assertIn("dear_node_editor_sys::dne_create_editor", source)

    def test_all_extension_route_consumers_use_safe_and_exact_sys_crates(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source"
            for spec in PREBUILT.EXTENSION_SPECS:
                (source_root / "extensions" / spec.safe_crate).mkdir(parents=True)
                (source_root / "extensions" / spec.sys_crate).mkdir(parents=True)
            source_root.joinpath("Cargo.lock").write_text(
                "version = 4\n", encoding="utf-8"
            )

            source_consumer = root / "source-consumer"
            specs = PREBUILT.write_extension_route_consumer(
                source_consumer, source_root, "source-plus-prebuilt"
            )
            manifest = tomllib.loads(
                source_consumer.joinpath("Cargo.toml").read_text(encoding="utf-8")
            )
            self.assertEqual(specs, PREBUILT.EXTENSION_SPECS)
            for spec in specs:
                safe = manifest["dependencies"][spec.safe_crate]
                sys_dependency = manifest["dependencies"][spec.sys_crate]
                self.assertEqual(
                    safe["features"], ["build-from-source", "prebuilt"]
                )
                self.assertFalse(safe["default-features"])
                self.assertFalse(sys_dependency["default-features"])
                self.assertNotIn("features", sys_dependency)

            wasm_consumer = root / "wasm-consumer"
            wasm_specs = PREBUILT.write_extension_route_consumer(
                wasm_consumer, source_root, "wasm"
            )
            self.assertEqual(
                wasm_specs, PREBUILT.SUPPORTED_WASM_EXTENSION_SPECS
            )
            self.assertNotIn(
                "node-editor", {spec.extension_id for spec in wasm_specs}
            )
            wasm_manifest = tomllib.loads(
                wasm_consumer.joinpath("Cargo.toml").read_text(encoding="utf-8")
            )
            for spec in wasm_specs:
                self.assertEqual(
                    wasm_manifest["dependencies"][spec.safe_crate]["features"],
                    ["wasm"],
                )
                self.assertNotIn(
                    "features", wasm_manifest["dependencies"][spec.sys_crate]
                )

    @patch.object(PREBUILT, "run")
    def test_locked_consumer_preparation_resolves_before_fetch(self, run_mock):
        manifest = Path("consumer/Cargo.toml")
        target_dir = Path("target")

        PREBUILT._prepare_locked_consumer(
            manifest, "wasm32-unknown-unknown", target_dir
        )

        self.assertEqual(run_mock.call_count, 2)
        metadata_command = run_mock.call_args_list[0].args[0]
        fetch_command = run_mock.call_args_list[1].args[0]
        self.assertEqual(metadata_command[0:2], ("cargo", "metadata"))
        self.assertNotIn("--locked", metadata_command)
        self.assertEqual(fetch_command[0:2], ("cargo", "fetch"))
        self.assertIn("--locked", fetch_command)
        self.assertIn("wasm32-unknown-unknown", fetch_command)

    def test_source_plus_prebuilt_environment_exposes_only_stale_artifact_paths(self):
        ambient = {
            "DEAR_IMGUI_RS_CANDIDATE_SHA": CANDIDATE_SHA,
            "IMGUI_SYS_FORCE_BUILD": "ambient",
            "IMPLOT_SYS_SKIP_CC": "ambient",
        }
        with TemporaryDirectory() as temporary, patch.dict(
            os.environ, ambient, clear=False
        ):
            stale_root = Path(temporary) / "stale"
            PREBUILT.write_stale_extension_prebuilts(stale_root)
            result = PREBUILT.extension_source_consumer_environment(
                Path("target"), stale_root
            )

            self.assertTrue((stale_root / "core/lib/dear_imgui.lib").is_file())
            self.assertEqual(
                result["IMGUI_SYS_LIB_DIR"],
                os.fspath(stale_root / "core/lib"),
            )
            for spec in PREBUILT.EXTENSION_SPECS:
                self.assertEqual(
                    result[f"{spec.env_stem}_LIB_DIR"],
                    os.fspath(stale_root / spec.extension_id / "lib"),
                )
            for name in ambient:
                self.assertNotIn(name, result)

    def test_cargo_path_patch_is_valid_toml_for_the_host_path(self):
        with TemporaryDirectory() as temporary:
            dependency_path = Path(temporary) / "path with spaces"
            config = SOURCE_PACKAGES.cargo_path_patch("example-sys", dependency_path)

            parsed = tomllib.loads(config)

        self.assertEqual(
            parsed["patch"]["crates-io"]["example-sys"]["path"],
            os.fspath(dependency_path.resolve()),
        )

    def test_prebuilt_environment_removes_ambient_build_modes(self):
        ambient = {
            "IMGUI_SYS_FORCE_BUILD": "1",
            "IMGUI_SYS_PREBUILT_URL": "https://invalid.example",
            "IMGUI_SYS_USE_PREBUILT": "1",
        }
        with patch.dict(os.environ, ambient, clear=False):
            result = PREBUILT.prebuilt_consumer_environment(
                Path("artifact"), Path("target")
            )

        for name in ambient:
            self.assertNotIn(name, result)
        self.assertEqual(result["IMGUI_SYS_SKIP_CC"], "1")
        self.assertEqual(result["IMGUI_SYS_LIB_DIR"], os.fspath(Path("artifact/lib")))
        self.assertEqual(result["CARGO_TARGET_DIR"], os.fspath(Path("target")))

    def test_extension_prebuilt_environment_binds_both_exact_artifacts(self):
        spec = PREBUILT.EXTENSION_SPECS[0]
        ambient = {
            "IMPLOT_SYS_FORCE_BUILD": "1",
            "IMPLOT_SYS_PREBUILT_URL": "https://invalid.example",
            "IMPLOT_SYS_USE_PREBUILT": "1",
        }
        with patch.dict(os.environ, ambient, clear=False):
            result = PREBUILT.extension_prebuilt_consumer_environment(
                Path("core"),
                Path("extension"),
                spec,
                Path("target"),
            )

        for name in ambient:
            self.assertNotIn(name, result)
        self.assertEqual(result["IMGUI_SYS_LIB_DIR"], os.fspath(Path("core/lib")))
        self.assertEqual(
            result["IMPLOT_SYS_LIB_DIR"], os.fspath(Path("extension/lib"))
        )
        self.assertNotIn("DEAR_IMGUI_RS_CANDIDATE_SHA", result)
        self.assertNotIn("DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH", result)

    def test_native_build_environments_remove_ambient_build_modes(self):
        ambient = {
            "IMGUI_SYS_FORCE_BUILD": "ambient",
            "IMGUI_SYS_LIB_DIR": "ambient",
            "IMGUI_SYS_PKG_FEATURES": "ambient",
            "IMGUI_SYS_PREBUILT_URL": "https://invalid.example",
            "IMGUI_SYS_SKIP_CC": "ambient",
            "IMGUI_SYS_USE_PREBUILT": "ambient",
        }
        with (
            patch.dict(os.environ, ambient, clear=False),
            patch.object(PREBUILT, "run") as run_command,
            patch.object(
                PREBUILT,
                "_built_core_archive",
                return_value=Path("core.tar.gz"),
            ),
            patch.object(
                PREBUILT, "_read_prebuilt_manifest", return_value={"crt": "md"}
            ),
            patch.object(
                PREBUILT,
                "core_artifact_identity",
                return_value="fnv1a64:0123456789abcdef",
            ),
        ):
            built_crt = PREBUILT.build_host_prebuilt_packages(
                Path("repository"), Path("target"), Path("packages"), CANDIDATE_SHA
            )

        self.assertEqual(built_crt, "md")
        normal_env = run_command.call_args_list[0].kwargs["env"]
        stack_env = run_command.call_args_list[7].kwargs["env"]
        normal_command = run_command.call_args_list[0].args[0]
        stack_command = run_command.call_args_list[7].args[0]
        for name in (
            "IMGUI_SYS_LIB_DIR",
            "IMGUI_SYS_PREBUILT_URL",
            "IMGUI_SYS_SKIP_CC",
            "IMGUI_SYS_USE_PREBUILT",
        ):
            self.assertNotIn(name, normal_env)
            self.assertNotIn(name, stack_env)
        self.assertNotIn("IMGUI_SYS_PKG_FEATURES", normal_env)
        self.assertEqual(normal_env["IMGUI_SYS_FORCE_BUILD"], "1")
        self.assertEqual(normal_env["DEAR_IMGUI_RS_CANDIDATE_SHA"], CANDIDATE_SHA)
        self.assertEqual(stack_env["IMGUI_SYS_FORCE_BUILD"], "1")
        self.assertEqual(stack_env["IMGUI_SYS_PKG_FEATURES"], "stack-layout")
        self.assertNotIn("--no-default-features", normal_command)
        self.assertIn("package-bin", normal_command)
        self.assertIn("--no-default-features", stack_command)
        self.assertIn("package-bin,stack-layout", stack_command)
        extension_calls = run_command.call_args_list[1:7]
        self.assertEqual(len(extension_calls), len(PREBUILT.EXTENSION_SPECS))
        for call in extension_calls:
            self.assertEqual(
                call.kwargs["env"]["DEAR_IMGUI_RS_CANDIDATE_SHA"], CANDIDATE_SHA
            )
            self.assertEqual(
                call.kwargs["env"]["DEAR_IMGUI_CORE_ARTIFACT_IDENTITY_HASH"],
                "fnv1a64:0123456789abcdef",
            )

    def test_release_build_covers_every_core_and_extension_profile(self):
        with (
            patch.object(PREBUILT, "run") as run_command,
            patch.object(
                PREBUILT,
                "_built_core_archive",
                return_value=Path("core.tar.gz"),
            ),
            patch.object(PREBUILT, "_read_prebuilt_manifest", return_value={}),
            patch.object(
                PREBUILT,
                "core_artifact_identity",
                return_value="fnv1a64:0123456789abcdef",
            ),
        ):
            PREBUILT.build_release_prebuilt_packages(
                Path("repository"),
                Path("target"),
                Path("packages"),
                "x86_64-pc-windows-msvc",
                CANDIDATE_SHA,
                crt="md",
            )

        self.assertEqual(len(run_command.call_args_list), 17)
        core_calls = [run_command.call_args_list[index] for index in (0, 7, 9, 15)]
        self.assertEqual(
            {
                next(
                    argument.removeprefix("package-bin,")
                    for argument in call.args[0]
                    if argument.startswith("package-bin")
                )
                for call in core_calls
            },
            {"package-bin", "stack-layout", "freetype", "stack-layout,freetype"},
        )
        for call in run_command.call_args_list:
            self.assertIn("--target", call.args[0])
            self.assertIn("x86_64-pc-windows-msvc", call.args[0])
            self.assertEqual(call.kwargs["env"].get("IMGUI_SYS_PKG_CRT", "md"), "md")


class PrebuiltWorkflowTests(unittest.TestCase):
    def test_workflow_delegates_candidate_and_profile_mapping_to_python(self):
        workflow = REPO_ROOT.joinpath(
            ".github/workflows/prebuilt-binaries.yml"
        ).read_text(encoding="utf-8")

        self.assertIn("verify_packaged_core.py build-prebuilt", workflow)
        self.assertIn("verify_packaged_core.py prebuilt", workflow)
        self.assertIn("timeout-minutes: 90", workflow)
        self.assertIn("run_contract.py windows-vcpkg", workflow)
        self.assertIn('--target "${{ matrix.target }}"', workflow)
        self.assertIn('--crt "${{ matrix.crt }}"', workflow)
        self.assertIn("--package freetype", workflow)
        self.assertEqual(
            workflow.count(
                '          "${{ inputs.candidate_sha }}"\n'
                '          "${{ matrix.crt }}"'
            ),
            2,
        )
        self.assertNotIn("cargo run -p", workflow)
        self.assertNotIn("configure_prebuilt_windows.py", workflow)
        self.assertNotIn("DEAR_IMGUI_CORE_ARTIFACT_PROFILE_HASH", workflow)
        self.assertNotIn("DEAR_IMGUI_RS_CANDIDATE_SHA:", workflow)


class PackageWorkspaceTests(unittest.TestCase):
    def test_packaged_provider_sources_follow_inventory_crate_roots(self):
        inventory_source = REPO_ROOT / "tools/build-support/maintained_sources.json"
        inventory_data = json.loads(inventory_source.read_text(encoding="utf-8"))
        provider_sources = [
            source for source in inventory_data["sources"] if source["provider"] is not None
        ]

        with TemporaryDirectory() as directory:
            root = Path(directory)
            archive_dir = root / "archives"
            helper_path = root / "helper"
            destination = root / "provider-sources"
            archive_dir.mkdir()
            helper_path.mkdir()
            (helper_path / "maintained_sources.json").write_text(
                inventory_source.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            packages = []
            for source in provider_sources:
                version = "0.16.0-alpha.1"
                package = ARCHIVE.PackageRecord(
                    source["crate_name"], Path(source["crate_root"]), version
                )
                packages.append(package)
                archive_root = f"{package.name}-{version}"
                write_archive(
                    archive_dir / f"{package.name}-{version}.crate",
                    {
                        f"{archive_root}/Cargo.toml": b"[package]\n",
                        f"{archive_root}/provider-marker": source["id"].encode(),
                    },
                )

            staged_root, staged_inventory = (
                SOURCE_PACKAGES.stage_packaged_wasm_provider_sources(
                    archive_dir,
                    packages,
                    helper_path,
                    destination,
                )
            )

            self.assertEqual(staged_root, destination)
            self.assertEqual(
                staged_inventory,
                helper_path / "maintained_sources.json",
            )
            for source in provider_sources:
                marker = destination / source["crate_root"] / "provider-marker"
                self.assertEqual(marker.read_text(encoding="utf-8"), source["id"])

    def test_lock_commit_keeps_commit_message_and_dirty_error_distinct(self):
        command_results = (
            subprocess.CompletedProcess(("git", "add"), 0),
            subprocess.CompletedProcess(("git", "diff"), 1),
            subprocess.CompletedProcess(("git", "commit"), 0),
        )
        with (
            patch.object(
                SOURCE_PACKAGES, "run", side_effect=command_results
            ) as run_command,
            patch.object(
                SOURCE_PACKAGES, "_require_clean_package_workspace"
            ) as require_clean,
        ):
            SOURCE_PACKAGES._commit_lockfile_if_changed(
                Path("repository"),
                "ci: test lock commit",
                "temporary workspace remained dirty",
            )

        commit_command = run_command.call_args_list[2].args[0]
        self.assertEqual(commit_command[-1], "ci: test lock commit")
        require_clean.assert_called_once_with(
            Path("repository"), "temporary workspace remained dirty"
        )

    def test_package_submodules_follow_the_declared_nested_topology(self):
        nested = SOURCE_PACKAGES.PACKAGE_NESTED_SUBMODULES
        declarations = [{"top-level": "top-level-module"}]
        declarations.extend(
            {item.path.as_posix(): f"nested-{index}"}
            for index, item in enumerate(nested)
        )
        with (
            patch.object(
                SOURCE_PACKAGES,
                "_git_submodule_declarations",
                side_effect=declarations,
            ),
            patch.object(SOURCE_PACKAGES, "_configure_local_submodule") as configure,
            patch.object(SOURCE_PACKAGES, "run") as run_command,
        ):
            SOURCE_PACKAGES.initialize_package_submodules(
                Path("source"), Path("repository")
            )

        self.assertEqual(configure.call_count, len(nested) + 1)
        commands = [call.args[0] for call in run_command.call_args_list]
        self.assertEqual(
            commands[0],
            (
                "git",
                "-C",
                Path("repository"),
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "update",
                "--init",
            ),
        )
        self.assertEqual(
            commands[1:],
            [
                (
                    "git",
                    "-C",
                    Path("repository").joinpath(item.parent),
                    "-c",
                    "protocol.file.allow=always",
                    "submodule",
                    "update",
                    "--init",
                    item.path.as_posix(),
                )
                for item in nested
            ],
        )


class CliTests(unittest.TestCase):
    def test_default_and_full_commands_run_the_complete_gate(self):
        with patch.object(CLI, "verify_packaged_core") as verify:
            self.assertEqual(CLI.main([]), 0)
            self.assertEqual(CLI.main(["full"]), 0)

        self.assertEqual(verify.call_count, 2)

    def test_prebuilt_command_routes_named_arguments(self):
        with patch.object(CLI, "verify_prebuilt_packages") as verify:
            result = CLI.main(
                [
                    "prebuilt",
                    "packages",
                    "x86_64-unknown-linux-gnu",
                    CANDIDATE_SHA,
                    "static",
                ]
            )

        self.assertEqual(result, 0)
        verify.assert_called_once_with(
            Path("packages"),
            "x86_64-unknown-linux-gnu",
            CANDIDATE_SHA,
            crt="static",
            source_root=CLI.WORKSPACE_ROOT,
            profile_scope="all",
        )

    def test_build_prebuilt_command_routes_target_candidate_and_crt(self):
        with patch.object(CLI, "build_release_prebuilt_packages") as build:
            result = CLI.main(
                [
                    "build-prebuilt",
                    "packages",
                    "x86_64-pc-windows-msvc",
                    CANDIDATE_SHA,
                    "md",
                    "--target-dir",
                    "target",
                ]
            )

        self.assertEqual(result, 0)
        build.assert_called_once_with(
            CLI.WORKSPACE_ROOT,
            Path("target"),
            Path("packages"),
            "x86_64-pc-windows-msvc",
            CANDIDATE_SHA,
            crt="md",
        )

    def test_head_candidate_resolves_from_the_checked_out_commit(self):
        completed = subprocess.CompletedProcess(
            ("git", "rev-parse"), 0, stdout=f"{CANDIDATE_SHA}\n"
        )
        with patch.object(CLI, "run", return_value=completed) as command:
            resolved = CLI._resolve_candidate_sha("HEAD")

        self.assertEqual(resolved, CANDIDATE_SHA)
        command.assert_called_once()

    def test_legacy_prebuilt_alias_remains_compatible(self):
        with patch.object(CLI, "verify_prebuilt_packages") as verify:
            result = CLI.main(
                [
                    "--verify-prebuilt-packages",
                    "packages",
                    "aarch64-apple-darwin",
                    CANDIDATE_SHA,
                ]
            )

        self.assertEqual(result, 0)
        verify.assert_called_once_with(
            Path("packages"),
            "aarch64-apple-darwin",
            CANDIDATE_SHA,
            crt="",
            source_root=CLI.WORKSPACE_ROOT,
            profile_scope="all",
        )

    def test_verification_failure_returns_one(self):
        stderr = io.StringIO()
        with (
            patch.object(
                CLI,
                "verify_packaged_core",
                side_effect=CLI.VerificationError("failed"),
            ),
            redirect_stderr(stderr),
        ):
            result = CLI.main([])

        self.assertEqual(result, 1)
        self.assertEqual(stderr.getvalue(), "::error::failed\n")

    def test_usage_failure_exits_with_two(self):
        with (
            redirect_stderr(io.StringIO()),
            self.assertRaises(SystemExit) as raised,
        ):
            CLI.main(["prebuilt", "packages"])

        self.assertEqual(raised.exception.code, 2)

    def test_help_names_the_prebuilt_arguments(self):
        help_text = CLI._build_parser().format_help()

        self.assertIn(
            "prebuilt PACKAGE_DIR TARGET CANDIDATE_SHA [CRT]", help_text
        )
        self.assertIn(
            "--verify-prebuilt-packages PACKAGE_DIR TARGET CANDIDATE_SHA [CRT]",
            help_text,
        )


if __name__ == "__main__":
    unittest.main()
