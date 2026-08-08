"""Native multi-viewport smoke contracts and backend-specific validation."""

from __future__ import annotations

import os
import platform
import sys
import tempfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from _process import ProcessStartError, environment, managed_background, run_bounded
from _runtime_gate_common import (
    GateCategory,
    GateResult,
    RuntimeContractError,
    _background_json,
    _check_stage,
    _example_binary,
    _finalize,
    _prepare_evidence,
    _process_json,
    _read_object,
    _reject_excess_attempt,
    _run_example_build,
    _sdl3_runtime_library_directories,
)
from _runtime_gate_display import (
    _check_background,
    _find_lavapipe_icd,
    _require_linux_runtime_tools,
    _require_linux_sdl3_glow_tools,
    _wait_for_window_manager,
    _wait_for_xvfb,
)
from _verification import write_json


class ViewportSmokeProfile(str, Enum):
    """Runtime routing profile for one viewport smoke contract."""

    WGPU_VULKAN = "WgpuVulkan"
    SDL3_GLOW = "Sdl3Glow"
    ASH_VULKAN = "AshVulkan"


@dataclass(frozen=True)
class ViewportSmokeSpec:
    """Backend-specific contract layered over the shared real-window harness."""

    profile: ViewportSmokeProfile
    gate: str
    binary: str
    features: str
    package_names: tuple[str, ...]
    probe_tool: str
    probe_arguments: tuple[str, ...]
    probe_log_stem: str
    probe_label: str
    probe_identities: tuple[str, ...]
    probe_identity_error: str
    probe_required_fragments: tuple[str, ...]
    build_label: str
    child_label: str
    success_summary: str
    payload_validator: Callable[[Mapping[str, object]], list[str]]


def _validate_viewport_lifecycle(
    payload: Mapping[str, object],
    lifecycle_fields: Sequence[str],
    *,
    schema_version: int = 3,
) -> list[str]:
    errors: list[str] = []
    actual_schema_version = payload.get("schema_version")
    if type(actual_schema_version) is not int or actual_schema_version != schema_version:
        errors.append(
            f"schema_version expected {schema_version}, got {actual_schema_version!r}"
        )
    for field_name in lifecycle_fields:
        if payload.get(field_name) is not True:
            errors.append(f"{field_name} expected True, got {payload.get(field_name)!r}")
    return errors


def _viewport_id_set(
    payload: Mapping[str, object], field_name: str, errors: list[str]
) -> set[int]:
    value = payload.get(field_name)
    if not isinstance(value, list) or not value:
        errors.append(f"{field_name} must be a nonempty u32 array")
        return set()
    if any(
        type(viewport_id) is not int or not 0 <= viewport_id <= 0xFFFF_FFFF
        for viewport_id in value
    ):
        errors.append(f"{field_name} must contain only u32 values")
        return set()
    viewport_ids = set(value)
    if len(viewport_ids) != len(value):
        errors.append(f"{field_name} must not contain duplicate viewport IDs")
    return viewport_ids


def _validate_software_vulkan_adapter(
    payload: Mapping[str, object], errors: list[str]
) -> None:
    adapter = payload.get("adapter")
    if not isinstance(adapter, dict):
        errors.append("adapter must be a JSON object")
        return
    if adapter.get("backend") != "Vulkan":
        errors.append(f"adapter backend must be Vulkan, got {adapter.get('backend')!r}")
    if adapter.get("device_type") != "Cpu":
        errors.append(
            f"adapter device_type must be Cpu, got {adapter.get('device_type')!r}"
        )
    identity = " ".join(
        str(adapter.get(field_name, "")).lower()
        for field_name in ("name", "driver", "driver_info")
    )
    if "lavapipe" not in identity and "llvmpipe" not in identity:
        errors.append("adapter identity does not report Lavapipe/llvmpipe")


def _validate_viewport_payload(payload: Mapping[str, object]) -> list[str]:
    errors = _validate_viewport_lifecycle(
        payload,
        (
            "secondary_viewport_while_held_observed",
            "merge_observed",
            "main_present_bracketed_by_test_engine",
        ),
    )
    rendered = _viewport_id_set(
        payload,
        "secondary_render_submitted_before_main_acquire_viewport_ids",
        errors,
    )
    presented = _viewport_id_set(
        payload,
        "secondary_present_submitted_before_main_acquire_viewport_ids",
        errors,
    )
    if rendered and presented and rendered.isdisjoint(presented):
        errors.append(
            "secondary render and present submissions before main acquisition "
            "must share a viewport ID"
        )
    _validate_software_vulkan_adapter(payload, errors)
    return errors


def _validate_upstream_viewport_suite_payload(
    payload: Mapping[str, object],
) -> list[str]:
    errors: list[str] = []
    schema_version = payload.get("schema_version")
    if type(schema_version) is not int or schema_version != 1:
        errors.append(f"schema_version expected 1, got {schema_version!r}")
    expected_fields = {
        "suite": "upstream-viewports",
        "category": "viewport",
        "platform_backend": "Winit",
        "renderer_backend": "WGPU",
    }
    for field_name, expected in expected_fields.items():
        if payload.get(field_name) != expected:
            errors.append(
                f"{field_name} expected {expected!r}, got {payload.get(field_name)!r}"
            )
    for field_name in ("real_platform_backend", "runtime_teardown_complete"):
        if payload.get(field_name) is not True:
            errors.append(f"{field_name} expected True, got {payload.get(field_name)!r}")
    for field_name in ("registered_count", "tested", "success", "in_queue"):
        value = payload.get(field_name)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"{field_name} must be a nonnegative integer")
    registered_tests = payload.get("registered_tests")
    if (
        not isinstance(registered_tests, list)
        or not registered_tests
        or any(not isinstance(name, str) or not name for name in registered_tests)
        or len(set(registered_tests)) != len(registered_tests)
    ):
        errors.append("registered_tests must contain unique, nonempty test names")
        registered_tests = []
    registered_count = payload.get("registered_count")
    if registered_count != len(registered_tests):
        errors.append(
            "registered_count must match the dynamically registered test manifest"
        )
    if (
        not isinstance(registered_count, int)
        or isinstance(registered_count, bool)
        or registered_count <= 0
        or payload.get("tested") != registered_count
        or payload.get("success") != registered_count
        or payload.get("in_queue") != 0
    ):
        errors.append(
            "upstream viewport suite requires every dynamically registered test "
            "to finish successfully"
        )
    _validate_software_vulkan_adapter(payload, errors)
    return errors


def _validate_sdl3_glow_viewport_payload(
    payload: Mapping[str, object],
) -> list[str]:
    errors = _validate_viewport_lifecycle(
        payload,
        (
            "merge_observed",
            "main_present_bracketed_by_test_engine",
            "external_texture_filters_preserved",
            "sampler_pixels_prove_isolation",
            "raw_callback_typed_state_observed",
            "reset_render_state_recovered",
            "render_state_cleared_after_callback",
            "application_gl_state_restored",
        ),
        schema_version=6,
    )
    sampler_strategy = payload.get("sampler_strategy")
    if sampler_strategy not in ("sampler_objects", "texture_parameters"):
        errors.append(
            "sampler_strategy must be sampler_objects or texture_parameters, "
            f"got {sampler_strategy!r}"
        )
    _viewport_id_set(
        payload,
        "secondary_draw_issued_before_main_present_viewport_ids",
        errors,
    )
    renderer = payload.get("renderer")
    if not isinstance(renderer, dict):
        errors.append("renderer must be a JSON object")
        return errors
    if renderer.get("backend") != "OpenGL":
        errors.append(
            f"renderer backend must be OpenGL, got {renderer.get('backend')!r}"
        )
    for field_name in ("vendor", "name", "version"):
        if not isinstance(renderer.get(field_name), str) or not renderer[field_name]:
            errors.append(f"renderer {field_name} must be a non-empty string")
    identity = " ".join(
        str(renderer.get(field_name, "")).lower()
        for field_name in ("vendor", "name", "version")
    )
    if "lavapipe" not in identity and "llvmpipe" not in identity:
        errors.append("renderer identity does not report Mesa llvmpipe")
    return errors


def _validate_ash_vulkan_viewport_payload(
    payload: Mapping[str, object],
) -> list[str]:
    errors = _validate_viewport_lifecycle(
        payload,
        (
            "dynamic_rendering_enabled",
            "validation_layer_enabled",
            "secondary_viewport_created",
            "secondary_viewport_resized",
            "merge_observed",
            "callback_only_frame_executed",
            "raw_callback_typed_state_observed",
            "nearest_sampler_descriptor_set_observed",
            "linear_sampler_descriptor_set_observed",
            "sampler_descriptor_sets_distinct",
            "reset_render_state_recovered",
            "render_state_cleared_after_callback",
            "managed_texture_updated",
            "managed_texture_removed",
            "texture_retirement_null_fence_rejected",
            "texture_retirement_queue_drained",
            "main_present_completed",
            "renderer_shutdown_complete",
            "viewport_runtime_shutdown_complete",
            "platform_shutdown_complete",
            "gpu_idle_before_teardown",
            "vulkan_resources_dropped",
        ),
        schema_version=2,
    )
    rendered_ids = _viewport_id_set(
        payload,
        "secondary_render_submitted_viewport_ids",
        errors,
    )
    presented_ids = _viewport_id_set(
        payload,
        "secondary_present_submitted_viewport_ids",
        errors,
    )
    if rendered_ids and presented_ids and rendered_ids.isdisjoint(presented_ids):
        errors.append(
            "secondary render and present submissions must share a viewport ID"
        )
    validation_error_count = payload.get("validation_error_count")
    if type(validation_error_count) is not int or validation_error_count != 0:
        errors.append(
            "validation_error_count expected 0, "
            f"got {validation_error_count!r}"
        )
    validation_warning_count = payload.get("validation_warning_count")
    if type(validation_warning_count) is not int or validation_warning_count != 0:
        errors.append(
            "validation_warning_count expected 0, "
            f"got {validation_warning_count!r}"
        )
    retirement_count = payload.get("texture_retirement_fence_completion_count")
    if type(retirement_count) is not int or retirement_count < 2:
        errors.append(
            "texture_retirement_fence_completion_count must be at least 2, "
            f"got {retirement_count!r}"
        )
    _validate_software_vulkan_adapter(payload, errors)
    return errors


_WGPU_VIEWPORT_SMOKE = ViewportSmokeSpec(
    profile=ViewportSmokeProfile.WGPU_VULKAN,
    gate="multi-viewport-smoke",
    binary="wgpu_multi_viewport_smoke",
    features="multi-viewport,test-engine",
    package_names=(
        "xvfb",
        "openbox",
        "mesa-vulkan-drivers",
        "vulkan-tools",
        "libxkbcommon-x11-0",
    ),
    probe_tool="vulkaninfo",
    probe_arguments=("--summary",),
    probe_log_stem="adapter",
    probe_label="Lavapipe adapter probe",
    probe_identities=("lavapipe", "llvmpipe"),
    probe_identity_error="vulkaninfo did not expose a Lavapipe/llvmpipe adapter",
    probe_required_fragments=(),
    build_label="WGPU multi-viewport example build",
    child_label="WGPU multi-viewport child",
    success_summary=(
        "secondary Winit/WGPU viewport lifecycle and every registered official "
        "upstream viewport test passed"
    ),
    payload_validator=_validate_viewport_payload,
)


_SDL3_GLOW_VIEWPORT_SMOKE = ViewportSmokeSpec(
    profile=ViewportSmokeProfile.SDL3_GLOW,
    gate="sdl3-glow-multi-viewport-smoke",
    binary="sdl3_glow_multi_viewport_smoke",
    features="sdl3-glow-multi-viewport,test-engine",
    package_names=(
        "xvfb",
        "openbox",
        "mesa-utils",
        "libgl1-mesa-dri",
        "libxkbcommon-x11-0",
    ),
    probe_tool="glxinfo",
    probe_arguments=("-B",),
    probe_log_stem="renderer",
    probe_label="Mesa llvmpipe OpenGL probe",
    probe_identities=("llvmpipe", "lavapipe"),
    probe_identity_error="glxinfo did not expose a Mesa llvmpipe renderer",
    probe_required_fragments=(),
    build_label="SDL3/Glow multi-viewport example build",
    child_label="SDL3/Glow multi-viewport child",
    success_summary="secondary SDL3/Glow viewport create, render, merge, and teardown passed",
    payload_validator=_validate_sdl3_glow_viewport_payload,
)


_ASH_VULKAN_VIEWPORT_SMOKE = ViewportSmokeSpec(
    profile=ViewportSmokeProfile.ASH_VULKAN,
    gate="ash-vulkan-validation-smoke",
    binary="ash_vulkan_validation_smoke",
    features="ash-winit-multi-viewport,ash-dynamic-rendering",
    package_names=(
        "xvfb",
        "openbox",
        "mesa-vulkan-drivers",
        "vulkan-tools",
        "vulkan-validationlayers",
        "libxkbcommon-x11-0",
    ),
    probe_tool="vulkaninfo",
    probe_arguments=("--summary",),
    probe_log_stem="adapter",
    probe_label="Lavapipe and Vulkan validation-layer probe",
    probe_identities=("lavapipe", "llvmpipe"),
    probe_identity_error="vulkaninfo did not expose a Lavapipe/llvmpipe adapter",
    probe_required_fragments=("vk_layer_khronos_validation",),
    build_label="Ash dynamic-rendering multi-viewport example build",
    child_label="Ash Vulkan validation multi-viewport child",
    success_summary=(
        "Ash dynamic-rendering secondary viewport create, resize, callbacks, "
        "present, merge, validation, and teardown passed"
    ),
    payload_validator=_validate_ash_vulkan_viewport_payload,
)


def _run_viewport_smoke(
    *,
    spec: ViewportSmokeSpec,
    workspace_root: Path,
    evidence_dir: Path,
    candidate_sha: str,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run one real secondary-window lifecycle under a software renderer."""
    gate = spec.gate
    if not evidence_dir.is_absolute():
        evidence_dir = workspace_root / evidence_dir
    _prepare_evidence(
        evidence_dir=evidence_dir,
        gate=gate,
        attempt=attempt,
        candidate_sha=candidate_sha,
        owned_files=(
            "runtime-environment.json",
            "build.stdout.log",
            "build.stderr.log",
            "package-versions.stdout.log",
            "package-versions.stderr.log",
            "xvfb.stdout.log",
            "xvfb.stderr.log",
            "display.stdout.log",
            "display.stderr.log",
            "openbox.stdout.log",
            "openbox.stderr.log",
            "window-manager.stdout.log",
            "window-manager.stderr.log",
            "adapter.stdout.log",
            "adapter.stderr.log",
            "renderer.stdout.log",
            "renderer.stderr.log",
            "viewport.stdout.log",
            "viewport.stderr.log",
            "viewport-result.json",
            "upstream-viewports.stdout.log",
            "upstream-viewports.stderr.log",
            "upstream-viewports-result.json",
            "viewport-texture-parameters.stdout.log",
            "viewport-texture-parameters.stderr.log",
            "viewport-texture-parameters-result.json",
            "viewport-sampler-objects.stdout.log",
            "viewport-sampler-objects.stderr.log",
            "viewport-sampler-objects-result.json",
        ),
    )
    if rejected := _reject_excess_attempt(
        gate=gate,
        attempt=attempt,
        candidate_sha=candidate_sha,
        evidence_dir=evidence_dir,
    ):
        return rejected
    details: dict[str, object] = {}
    xvfb = None
    openbox = None
    xdg_runtime_owner = None
    try:
        if spec.profile in (
            ViewportSmokeProfile.WGPU_VULKAN,
            ViewportSmokeProfile.ASH_VULKAN,
        ):
            tools = _require_linux_runtime_tools()
            lavapipe_icd = _find_lavapipe_icd()
            route_diagnostics = {"lavapipe_icd": str(lavapipe_icd)}
            route_environment: dict[str, str | Path] = {
                "WINIT_UNIX_BACKEND": "x11",
                "VK_DRIVER_FILES": lavapipe_icd,
                "VK_ICD_FILENAMES": lavapipe_icd,
                "DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN": "1",
            }
            if spec.profile is ViewportSmokeProfile.WGPU_VULKAN:
                route_environment.update(
                    {
                        "WGPU_BACKEND": "vulkan",
                        "DEAR_IMGUI_VIEWPORT_DRAG_SMOKE": "1",
                    }
                )
            else:
                route_environment["DEAR_IMGUI_REQUIRE_VULKAN_VALIDATION"] = "1"
        elif spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            tools = _require_linux_sdl3_glow_tools()
            route_diagnostics = {"required_opengl_renderer": "Mesa llvmpipe"}
            route_environment = {
                "SDL_VIDEODRIVER": "x11",
                "DEAR_IMGUI_REQUIRE_SOFTWARE_OPENGL": "1",
            }
        else:  # pragma: no cover - specs are module-owned constants.
            raise RuntimeContractError(
                GateCategory.PRODUCT_FAILURE,
                f"unknown viewport smoke profile: {spec.profile.value}",
            )
        display = os.environ.get("DEAR_IMGUI_XVFB_DISPLAY", ":99")
        # Keep Wayland's AF_UNIX socket path below Linux's 108-byte limit.
        runtime_temp_root = "/tmp" if sys.platform.startswith("linux") else None
        xdg_runtime_owner = tempfile.TemporaryDirectory(
            prefix="dear-imgui-xdg-", dir=runtime_temp_root
        )
        xdg_runtime = Path(xdg_runtime_owner.name)
        xdg_runtime.chmod(0o700)
        diagnostics = {
            "display": display,
            "screen": "2560x1440x24",
            "architecture": platform.machine(),
            "runner_image": os.environ.get("ImageOS"),
            "runner_image_version": os.environ.get("ImageVersion"),
            "xdg_runtime_dir": str(xdg_runtime),
            "tools": {name: str(path) for name, path in sorted(tools.items())},
            **route_diagnostics,
        }
        write_json(evidence_dir / "runtime-environment.json", diagnostics)
        details["environment"] = diagnostics

        package_versions = run_bounded(
            (
                tools["dpkg-query"],
                "--show",
                "--showformat=${Package}=${Version}\\n",
                *spec.package_names,
            ),
            cwd=workspace_root,
            timeout=15.0,
            stdout_log=evidence_dir / "package-versions.stdout.log",
            stderr_log=evidence_dir / "package-versions.stderr.log",
        )
        details["package_versions"] = _process_json(package_versions, evidence_dir)
        _check_stage(
            package_versions,
            label="native runtime package version probe",
            nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
        )

        child_values = {
            "DISPLAY": display,
            "LIBGL_ALWAYS_SOFTWARE": "1",
            "GALLIUM_DRIVER": "llvmpipe",
            "DEAR_IMGUI_VIEWPORT_SMOKE_JSON": evidence_dir
            / "viewport-result.json",
            "IMGUI_SYS_FORCE_BUILD": "1",
            **route_environment,
        }
        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            child_values["DEAR_IMGUI_VIEWPORT_SMOKE"] = "1"
        child_environment = environment(child_values)
        child_environment.pop("DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE", None)
        child_environment["XDG_RUNTIME_DIR"] = str(xdg_runtime)

        build = _run_example_build(
            workspace_root=workspace_root,
            evidence_dir=evidence_dir,
            binary=spec.binary,
            features=spec.features,
            timeout=build_timeout,
            child_environment=child_environment,
        )
        details["build"] = _process_json(build, evidence_dir)
        _check_stage(
            build,
            label=spec.build_label,
            nonzero_category=GateCategory.PRODUCT_FAILURE,
        )
        binary = _example_binary(workspace_root, spec.binary)
        if not binary.is_file():
            raise RuntimeContractError(
                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                f"cargo succeeded without producing {binary}",
            )
        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            sdl3_library_dirs = _sdl3_runtime_library_directories(workspace_root)
            inherited_library_path = child_environment.get("LD_LIBRARY_PATH", "")
            child_environment["LD_LIBRARY_PATH"] = os.pathsep.join(
                (
                    *(str(path) for path in sdl3_library_dirs),
                    *((inherited_library_path,) if inherited_library_path else ()),
                )
            )
            diagnostics["sdl3_library_dirs"] = [
                str(path) for path in sdl3_library_dirs
            ]
            write_json(evidence_dir / "runtime-environment.json", diagnostics)

        xvfb = managed_background(
            (
                tools["Xvfb"],
                display,
                "-screen",
                "0",
                "2560x1440x24",
                "-nolisten",
                "tcp",
                "-ac",
            ),
            cwd=workspace_root,
            env=child_environment,
            stdout_log=evidence_dir / "xvfb.stdout.log",
            stderr_log=evidence_dir / "xvfb.stderr.log",
        )
        try:
            with xvfb:
                _wait_for_xvfb(xvfb, display)
                display_info = run_bounded(
                    (tools["xdpyinfo"], "-display", display),
                    cwd=workspace_root,
                    env=child_environment,
                    timeout=15.0,
                    stdout_log=evidence_dir / "display.stdout.log",
                    stderr_log=evidence_dir / "display.stderr.log",
                )
                details["display_probe"] = _process_json(display_info, evidence_dir)
                _check_stage(
                    display_info,
                    label="Xvfb display probe",
                    nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                )

                openbox = managed_background(
                    (tools["openbox"],),
                    cwd=workspace_root,
                    env=child_environment,
                    stdout_log=evidence_dir / "openbox.stdout.log",
                    stderr_log=evidence_dir / "openbox.stderr.log",
                )
                try:
                    with openbox:
                        window_manager_probe = _wait_for_window_manager(
                            process=openbox,
                            executable=tools["xprop"],
                            workspace_root=workspace_root,
                            evidence_dir=evidence_dir,
                            child_environment=child_environment,
                        )
                        details["window_manager_probe"] = _process_json(
                            window_manager_probe, evidence_dir
                        )
                        renderer_probe = run_bounded(
                            (tools[spec.probe_tool], *spec.probe_arguments),
                            cwd=workspace_root,
                            env=child_environment,
                            timeout=30.0,
                            stdout_log=evidence_dir
                            / f"{spec.probe_log_stem}.stdout.log",
                            stderr_log=evidence_dir
                            / f"{spec.probe_log_stem}.stderr.log",
                        )
                        details["renderer_probe"] = _process_json(
                            renderer_probe, evidence_dir
                        )
                        _check_stage(
                            renderer_probe,
                            label=spec.probe_label,
                            nonzero_category=GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                        )
                        renderer_output = "\n".join(
                            path.read_text(encoding="utf-8", errors="replace").lower()
                            for path in renderer_probe.log_paths
                        )
                        if not any(
                            identity in renderer_output
                            for identity in spec.probe_identities
                        ):
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                spec.probe_identity_error,
                            )
                        missing_probe_fragments = tuple(
                            fragment
                            for fragment in spec.probe_required_fragments
                            if fragment.lower() not in renderer_output
                        )
                        if missing_probe_fragments:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "runtime probe is missing required capabilities: "
                                + ", ".join(missing_probe_fragments),
                            )

                        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
                            profiles = (
                                (
                                    "texture-parameters",
                                    "3.2",
                                    "-GL_ARB_sampler_objects",
                                    "texture_parameters",
                                ),
                                ("sampler-objects", "3.3", None, "sampler_objects"),
                            )
                            profile_details: dict[str, object] = {}
                            for slug, version_override, extension_override, _ in profiles:
                                profile_environment = dict(child_environment)
                                profile_environment["MESA_GL_VERSION_OVERRIDE"] = version_override
                                if extension_override is None:
                                    profile_environment.pop("MESA_EXTENSION_OVERRIDE", None)
                                else:
                                    profile_environment["MESA_EXTENSION_OVERRIDE"] = (
                                        extension_override
                                    )
                                profile_result = (
                                    evidence_dir / f"viewport-{slug}-result.json"
                                )
                                profile_result.unlink(missing_ok=True)
                                profile_environment[
                                    "DEAR_IMGUI_VIEWPORT_SMOKE_JSON"
                                ] = str(profile_result)
                                child = run_bounded(
                                    (binary,),
                                    cwd=workspace_root,
                                    env=profile_environment,
                                    timeout=child_timeout,
                                    stdout_log=evidence_dir / f"viewport-{slug}.stdout.log",
                                    stderr_log=evidence_dir / f"viewport-{slug}.stderr.log",
                                )
                                profile_details[slug] = _process_json(child, evidence_dir)
                                _check_stage(
                                    child,
                                    label=f"{spec.child_label} ({slug})",
                                    nonzero_category=GateCategory.PRODUCT_FAILURE,
                                )
                            details["viewport_profiles"] = profile_details
                        else:
                            viewport_result = evidence_dir / "viewport-result.json"
                            viewport_result.unlink(missing_ok=True)
                            child = run_bounded(
                                (binary,),
                                cwd=workspace_root,
                                env=child_environment,
                                timeout=child_timeout,
                                stdout_log=evidence_dir / "viewport.stdout.log",
                                stderr_log=evidence_dir / "viewport.stderr.log",
                            )
                            details["viewport"] = _process_json(child, evidence_dir)
                            _check_stage(
                                child,
                                label=spec.child_label,
                                nonzero_category=GateCategory.PRODUCT_FAILURE,
                            )
                            if spec.profile is ViewportSmokeProfile.WGPU_VULKAN:
                                upstream_environment = dict(child_environment)
                                upstream_environment.pop(
                                    "DEAR_IMGUI_VIEWPORT_DRAG_SMOKE", None
                                )
                                upstream_environment[
                                    "DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE"
                                ] = "1"
                                upstream_result = (
                                    evidence_dir / "upstream-viewports-result.json"
                                )
                                upstream_result.unlink(missing_ok=True)
                                upstream_environment[
                                    "DEAR_IMGUI_VIEWPORT_SMOKE_JSON"
                                ] = str(upstream_result)
                                upstream_child = run_bounded(
                                    (binary,),
                                    cwd=workspace_root,
                                    env=upstream_environment,
                                    timeout=child_timeout,
                                    stdout_log=evidence_dir
                                    / "upstream-viewports.stdout.log",
                                    stderr_log=evidence_dir
                                    / "upstream-viewports.stderr.log",
                                )
                                details["upstream_viewports"] = _process_json(
                                    upstream_child, evidence_dir
                                )
                                _check_stage(
                                    upstream_child,
                                    label="official upstream viewport Test Engine child",
                                    nonzero_category=GateCategory.PRODUCT_FAILURE,
                                )
                        if xvfb.poll() is not None:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                f"Xvfb exited while the child ran with status {xvfb.returncode}",
                            )
                        if openbox.poll() is not None:
                            raise RuntimeContractError(
                                GateCategory.INFRASTRUCTURE_UNAVAILABLE,
                                "openbox exited while the child ran with status "
                                f"{openbox.returncode}",
                            )
                finally:
                    details["openbox"] = _background_json(openbox, evidence_dir)
                    _check_background(openbox, "openbox")
        finally:
            details["xvfb"] = _background_json(xvfb, evidence_dir)
            _check_background(xvfb, "Xvfb")

        if spec.profile is ViewportSmokeProfile.SDL3_GLOW:
            results: dict[str, object] = {}
            for slug, expected_strategy in (
                ("texture-parameters", "texture_parameters"),
                ("sampler-objects", "sampler_objects"),
            ):
                payload = _read_object(evidence_dir / f"viewport-{slug}-result.json")
                errors = spec.payload_validator(payload)
                if payload.get("sampler_strategy") != expected_strategy:
                    errors.append(
                        f"sampler_strategy expected {expected_strategy!r}, "
                        f"got {payload.get('sampler_strategy')!r}"
                    )
                results[slug] = payload
                if errors:
                    raise RuntimeContractError(
                        GateCategory.PRODUCT_FAILURE,
                        f"{slug}: " + "; ".join(errors),
                    )
            details["results"] = results
        else:
            payload = _read_object(evidence_dir / "viewport-result.json")
            errors = spec.payload_validator(payload)
            details["result"] = payload
            if errors:
                raise RuntimeContractError(
                    GateCategory.PRODUCT_FAILURE,
                    "; ".join(errors),
                )
            if spec.profile is ViewportSmokeProfile.WGPU_VULKAN:
                upstream_payload = _read_object(
                    evidence_dir / "upstream-viewports-result.json"
                )
                upstream_errors = _validate_upstream_viewport_suite_payload(
                    upstream_payload
                )
                details["upstream_viewport_suite"] = upstream_payload
                if upstream_errors:
                    raise RuntimeContractError(
                        GateCategory.PRODUCT_FAILURE,
                        "; ".join(upstream_errors),
                    )
        result = GateResult(
            gate,
            True,
            GateCategory.PASSED,
            spec.success_summary,
            attempt,
            details,
        )
    except RuntimeContractError as error:
        result = GateResult(gate, False, error.category, str(error), attempt, details)
    except ProcessStartError as error:
        result = GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            str(error),
            attempt,
            details,
        )
    except OSError as error:
        result = GateResult(
            gate,
            False,
            GateCategory.INFRASTRUCTURE_UNAVAILABLE,
            f"runtime environment operation failed: {error}",
            attempt,
            details,
        )
    finally:
        if xdg_runtime_owner is not None:
            xdg_runtime_owner.cleanup()
    return _finalize(result, evidence_dir, candidate_sha)


def run_multi_viewport_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    candidate_sha: str,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run a real Winit/WGPU secondary-window lifecycle under Lavapipe."""
    return _run_viewport_smoke(
        spec=_WGPU_VIEWPORT_SMOKE,
        workspace_root=workspace_root,
        evidence_dir=evidence_dir,
        candidate_sha=candidate_sha,
        child_timeout=child_timeout,
        build_timeout=build_timeout,
        attempt=attempt,
    )


def run_sdl3_glow_viewport_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    candidate_sha: str,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run a real SDL3/Glow secondary-window lifecycle under Mesa llvmpipe."""
    return _run_viewport_smoke(
        spec=_SDL3_GLOW_VIEWPORT_SMOKE,
        workspace_root=workspace_root,
        evidence_dir=evidence_dir,
        candidate_sha=candidate_sha,
        child_timeout=child_timeout,
        build_timeout=build_timeout,
        attempt=attempt,
    )


def run_ash_vulkan_validation_smoke(
    *,
    workspace_root: Path,
    evidence_dir: Path,
    candidate_sha: str,
    child_timeout: float = 180.0,
    build_timeout: float = 900.0,
    attempt: int = 1,
) -> GateResult:
    """Run Ash dynamic-rendering multi-viewport under Lavapipe validation."""
    return _run_viewport_smoke(
        spec=_ASH_VULKAN_VIEWPORT_SMOKE,
        workspace_root=workspace_root,
        evidence_dir=evidence_dir,
        candidate_sha=candidate_sha,
        child_timeout=child_timeout,
        build_timeout=build_timeout,
        attempt=attempt,
    )
