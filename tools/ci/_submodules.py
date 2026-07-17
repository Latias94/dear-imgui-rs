"""Authoritative nested submodule topology used by repository CI tools."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import PurePosixPath


@dataclass(frozen=True)
class NestedSubmodule:
    """A nested submodule and the parent repository that declares it."""

    parent: PurePosixPath
    path: PurePosixPath
    shallow: bool

    def update_command(self) -> tuple[str, ...]:
        command = [
            "git",
            "-C",
            self.parent.as_posix(),
            "submodule",
            "update",
            "--init",
        ]
        if self.shallow:
            command.append("--depth=1")
        command.append(self.path.as_posix())
        return tuple(command)


CIMGUI_IMGUI = NestedSubmodule(
    PurePosixPath("dear-imgui-sys/third-party/cimgui"),
    PurePosixPath("imgui"),
    True,
)
CIMPLOT_IMPLOT = NestedSubmodule(
    PurePosixPath("extensions/dear-implot-sys/third-party/cimplot"),
    PurePosixPath("implot"),
    False,
)
CIMPLOT3D_IMPLOT3D = NestedSubmodule(
    PurePosixPath("extensions/dear-implot3d-sys/third-party/cimplot3d"),
    PurePosixPath("implot3d"),
    False,
)
CIMGUIZMO_IMGUIZMO = NestedSubmodule(
    PurePosixPath("extensions/dear-imguizmo-sys/third-party/cimguizmo"),
    PurePosixPath("ImGuizmo"),
    True,
)
CIMGUIZMO_QUAT = NestedSubmodule(
    PurePosixPath("extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat"),
    PurePosixPath("imGuIZMO.quat"),
    True,
)
CIMGUIZMO_QUAT_IMGUI = NestedSubmodule(
    PurePosixPath(
        "extensions/dear-imguizmo-quat-sys/third-party/cimguizmo_quat/imGuIZMO.quat"
    ),
    PurePosixPath("libs/imgui"),
    True,
)
CIMNODES_IMNODES = NestedSubmodule(
    PurePosixPath("extensions/dear-imnodes-sys/third-party/cimnodes"),
    PurePosixPath("imnodes"),
    True,
)
CIMNODES_EDITOR_NODE_EDITOR = NestedSubmodule(
    PurePosixPath("extensions/dear-node-editor-sys/third-party/cimnodes_editor"),
    PurePosixPath("imgui-node-editor"),
    True,
)


SELECTIVE_NESTED_SUBMODULES = (
    CIMGUI_IMGUI,
    CIMPLOT_IMPLOT,
    CIMPLOT3D_IMPLOT3D,
    CIMGUIZMO_IMGUIZMO,
    CIMGUIZMO_QUAT,
    CIMGUIZMO_QUAT_IMGUI,
    CIMNODES_IMNODES,
    CIMNODES_EDITOR_NODE_EDITOR,
)

# Packaging needs a deliberately different order and does not consume the second-level
# imGuIZMO.quat imgui checkout.
PACKAGE_NESTED_SUBMODULES = (
    CIMGUI_IMGUI,
    CIMPLOT_IMPLOT,
    CIMNODES_IMNODES,
    CIMNODES_EDITOR_NODE_EDITOR,
    CIMGUIZMO_IMGUIZMO,
    CIMPLOT3D_IMPLOT3D,
    CIMGUIZMO_QUAT,
)


SUBMODULE_COMMANDS = tuple(
    submodule.update_command() for submodule in SELECTIVE_NESTED_SUBMODULES
)
