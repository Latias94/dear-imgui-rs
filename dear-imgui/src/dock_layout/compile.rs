use super::model::{DockLayout, DockLayoutApply, DockLayoutError, DockSplit, DockspaceTarget};
use crate::{ConfigFlags, Id, sys, ui::Ui};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ptr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NodeIndex(pub(super) usize);

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Command<'layout> {
    Split {
        parent: NodeIndex,
        direction: DockSplit,
        ratio: f32,
        first: NodeIndex,
        second: NodeIndex,
    },
    DockWindow {
        node: NodeIndex,
        title: &'layout str,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CompiledLayout<'layout> {
    pub(super) node_count: usize,
    pub(super) commands: Vec<Command<'layout>>,
}

pub(super) fn compile_layout(layout: &DockLayout) -> Result<CompiledLayout<'_>, DockLayoutError> {
    let mut commands = Vec::new();
    let mut window_ids = HashMap::new();
    let mut node_count = 1usize;
    let mut pending = vec![(layout, NodeIndex(0))];

    while let Some((layout, node)) = pending.pop() {
        match layout {
            DockLayout::Tabs(windows) => {
                for title in windows {
                    if title.is_empty() {
                        return Err(DockLayoutError::EmptyWindowTitle);
                    }
                    if title.as_bytes().contains(&0) {
                        return Err(DockLayoutError::WindowTitleContainsNul {
                            title: title.clone(),
                        });
                    }
                    let stable_id = stable_window_id(title);
                    if stable_id.is_empty() {
                        return Err(DockLayoutError::EmptyWindowId {
                            title: title.clone(),
                        });
                    }
                    if let Some(first_title) = window_ids.insert(stable_id, title.as_str()) {
                        return Err(DockLayoutError::DuplicateWindowId {
                            first_title: first_title.to_owned(),
                            second_title: title.clone(),
                        });
                    }
                    commands.push(Command::DockWindow { node, title });
                }
            }
            DockLayout::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                if !ratio.is_finite() || *ratio <= 0.0 || *ratio >= 1.0 {
                    return Err(DockLayoutError::InvalidSplitRatio { ratio: *ratio });
                }

                let second_index = node_count
                    .checked_add(1)
                    .ok_or(DockLayoutError::LayoutTooLarge)?;
                let next_count = node_count
                    .checked_add(2)
                    .ok_or(DockLayoutError::LayoutTooLarge)?;
                let first_node = NodeIndex(node_count);
                let second_node = NodeIndex(second_index);
                node_count = next_count;

                commands.push(Command::Split {
                    parent: node,
                    direction: *direction,
                    ratio: *ratio,
                    first: first_node,
                    second: second_node,
                });
                pending.push((second, second_node));
                pending.push((first, first_node));
            }
        }
    }

    Ok(CompiledLayout {
        node_count,
        commands,
    })
}

fn stable_window_id(title: &str) -> &str {
    let bytes = title.as_bytes();
    let mut index = 0;
    let mut stable_id_start = 0;
    while index + 2 < bytes.len() {
        if bytes[index..index + 3] == *b"###" {
            stable_id_start = index + 3;
            index += 3;
        } else {
            index += 1;
        }
    }
    &title[stable_id_start..]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DockspaceSubmission {
    CurrentWindow,
    MainViewport,
}

pub(crate) fn submit_and_apply(
    ui: &Ui,
    target: &DockspaceTarget,
    layout: &DockLayout,
    apply: DockLayoutApply,
    submission: DockspaceSubmission,
) -> Result<Id, DockLayoutError> {
    target.validate()?;
    let compiled = compile_layout(layout)?;
    if !ui.io().config_flags().contains(ConfigFlags::DOCKING_ENABLE) {
        return Err(DockLayoutError::DockingDisabled);
    }

    let window_class = target
        .window_class_ref()
        .map(|class| class.to_imgui("Ui dock layout submission"));
    let window_class_ptr = window_class
        .as_ref()
        .map_or(ptr::null(), |class| class as *const _);
    let root_id = target.root_id().raw();
    let initial_position = match submission {
        DockspaceSubmission::CurrentWindow => ui.cursor_screen_pos(),
        DockspaceSubmission::MainViewport => target.initial_position(),
    };
    let initial_size = target.initial_size();

    ui.run_with_bound_context(|| {
        let claim = crate::dock_space::claim_dockspace_submission(
            ui,
            "Ui dock layout submission",
            target.root_id(),
            target.dock_flags(),
            submission == DockspaceSubmission::CurrentWindow,
        )
        .map_err(|_| DockLayoutError::DuplicateDockspaceSubmission {
            root_id: target.root_id(),
        })?;

        let result = (|| {
            // SAFETY: `run_with_bound_context` keeps this Ui's context current, and `root_id` is an
            // opaque value that Dear ImGui accepts for lookup without dereferencing Rust memory.
            let existed_before_submission =
                unsafe { !sys::igDockBuilderGetNode(root_id).is_null() };
            if apply == DockLayoutApply::IfMissing && existed_before_submission {
                // Persisted layouts are authoritative. Re-submit the host without mutating its
                // existing builder tree.
                return submit_dockspace(
                    submission,
                    root_id,
                    initial_size,
                    target.dock_flags().bits(),
                    window_class_ptr,
                    target.root_id(),
                );
            }

            // Dear ImGui requires all DockBuilder mutations to happen before the visible DockSpace
            // submission. AddNode(DockSpace) creates a keep-alive root without claiming a visible
            // host, so splits and window assignments are applied against the final tree.
            let builder_flags =
                target.dock_flags().bits() | sys::ImGuiDockNodeFlags_DockSpace as i32;
            // SAFETY: the bound context is current and the validated flags contain only supported
            // DockNode bits plus the internal DockSpace creation marker.
            let added_root = unsafe { sys::igDockBuilderAddNode(root_id, builder_flags) };
            if added_root != root_id {
                return Err(DockLayoutError::DockspaceSubmissionFailed {
                    root_id: target.root_id(),
                });
            }

            // SAFETY: `root_id` is a live node in the bound context and the validated target owns the
            // finite position and size values copied into these calls.
            unsafe {
                sys::igDockBuilderSetNodePos(
                    root_id,
                    sys::ImVec2 {
                        x: initial_position[0],
                        y: initial_position[1],
                    },
                );
                sys::igDockBuilderSetNodeSize(
                    root_id,
                    sys::ImVec2 {
                        x: initial_size[0],
                        y: initial_size[1],
                    },
                );
            }

            let mut executor = ImGuiCommandExecutor;
            execute_transaction(target.root_id(), &compiled, &mut executor)?;
            submit_dockspace(
                submission,
                root_id,
                initial_size,
                target.dock_flags().bits(),
                window_class_ptr,
                target.root_id(),
            )
        })();
        let main_viewport_host_skipped = result.is_ok()
            && submission == DockspaceSubmission::MainViewport
            && crate::dock_space::window_skips_items(
                &crate::dock_space::main_viewport_dockspace_host_name("Ui dock layout submission"),
            );
        if result.is_ok() && !main_viewport_host_skipped {
            if let Some(claim) = claim {
                claim.commit();
            }
        }
        result
    })
}

fn submit_dockspace(
    submission: DockspaceSubmission,
    root_id: sys::ImGuiID,
    initial_size: [f32; 2],
    flags: i32,
    window_class: *const sys::ImGuiWindowClass,
    public_id: Id,
) -> Result<Id, DockLayoutError> {
    // SAFETY: the bound context remains current, the optional window class remains borrowed for
    // this call, and the validated target supplied finite size/flag values.
    let submitted_id = unsafe {
        match submission {
            DockspaceSubmission::CurrentWindow => sys::igDockSpace(
                root_id,
                sys::ImVec2 {
                    x: initial_size[0],
                    y: initial_size[1],
                },
                flags,
                window_class,
            ),
            DockspaceSubmission::MainViewport => {
                sys::igDockSpaceOverViewport(root_id, sys::igGetMainViewport(), flags, window_class)
            }
        }
    };

    // SAFETY: the bound context remains current and `root_id` is the submitted dockspace ID.
    let submitted_root = unsafe { sys::igDockBuilderGetNode(root_id) };
    if submitted_id != root_id || submitted_root.is_null() {
        return Err(DockLayoutError::DockspaceSubmissionFailed { root_id: public_id });
    }
    Ok(public_id)
}

pub(super) trait CommandExecutor {
    fn split_node(
        &mut self,
        parent: sys::ImGuiID,
        direction: DockSplit,
        ratio: f32,
    ) -> Option<(sys::ImGuiID, sys::ImGuiID)>;
    fn dock_window(&mut self, title: &CStr, node: sys::ImGuiID);
    fn finish(&mut self, root: sys::ImGuiID);
    fn rollback(&mut self, root: sys::ImGuiID);
}

struct ImGuiCommandExecutor;

impl CommandExecutor for ImGuiCommandExecutor {
    fn split_node(
        &mut self,
        parent: sys::ImGuiID,
        direction: DockSplit,
        ratio: f32,
    ) -> Option<(sys::ImGuiID, sys::ImGuiID)> {
        let mut first = 0;
        let mut second = 0;
        // SAFETY: execution runs under the Ui's bound context, `parent` resolves from a live node
        // produced by this transaction, and both output pointers reference initialized locals.
        unsafe {
            sys::igDockBuilderSplitNode(
                parent,
                split_direction_raw(direction),
                ratio,
                &mut first,
                &mut second,
            );
        }
        (first != 0 && second != 0).then_some((first, second))
    }

    fn dock_window(&mut self, title: &CStr, node: sys::ImGuiID) {
        // SAFETY: `title` is NUL-terminated for the duration of the call and `node` was produced by
        // the active DockBuilder transaction in the current context.
        unsafe {
            sys::igDockBuilderDockWindow(title.as_ptr(), node);
        }
    }

    fn finish(&mut self, root: sys::ImGuiID) {
        // SAFETY: `root` is the live root of the active transaction in the current context.
        unsafe {
            sys::igDockBuilderFinish(root);
        }
    }

    fn rollback(&mut self, root: sys::ImGuiID) {
        // SAFETY: rollback executes before leaving the bound context and only removes the root
        // owned by the failed transaction.
        unsafe {
            sys::igDockBuilderRemoveNodeDockedWindows(root, true);
            sys::igDockBuilderRemoveNode(root);
        }
    }
}

pub(super) fn execute_transaction<E: CommandExecutor>(
    root_id: Id,
    compiled: &CompiledLayout<'_>,
    executor: &mut E,
) -> Result<(), DockLayoutError> {
    match execute_commands(root_id, compiled, executor) {
        Ok(()) => {
            executor.finish(root_id.raw());
            Ok(())
        }
        Err(error) => {
            executor.rollback(root_id.raw());
            Err(error)
        }
    }
}

fn execute_commands<E: CommandExecutor>(
    root_id: Id,
    compiled: &CompiledLayout<'_>,
    executor: &mut E,
) -> Result<(), DockLayoutError> {
    let mut nodes = vec![0; compiled.node_count];
    nodes[0] = root_id.raw();

    for command in &compiled.commands {
        match command {
            Command::Split {
                parent,
                direction,
                ratio,
                first,
                second,
            } => {
                let parent_id = resolve_node(&nodes, *parent)?;
                let (first_id, second_id) = executor
                    .split_node(parent_id, *direction, *ratio)
                    .ok_or(DockLayoutError::SplitFailed {
                        direction: *direction,
                        ratio: *ratio,
                    })?;
                nodes[first.0] = first_id;
                nodes[second.0] = second_id;
            }
            Command::DockWindow { node, title } => {
                let node_id = resolve_node(&nodes, *node)?;
                let title =
                    CString::new(*title).map_err(|_| DockLayoutError::WindowTitleContainsNul {
                        title: (*title).to_owned(),
                    })?;
                executor.dock_window(&title, node_id);
            }
        }
    }

    Ok(())
}

fn resolve_node(nodes: &[sys::ImGuiID], index: NodeIndex) -> Result<sys::ImGuiID, DockLayoutError> {
    nodes
        .get(index.0)
        .copied()
        .filter(|id| *id != 0)
        .ok_or(DockLayoutError::CompiledNodeUnavailable { index: index.0 })
}

fn split_direction_raw(direction: DockSplit) -> sys::ImGuiDir {
    match direction {
        DockSplit::Left => sys::ImGuiDir_Left,
        DockSplit::Right => sys::ImGuiDir_Right,
        DockSplit::Up => sys::ImGuiDir_Up,
        DockSplit::Down => sys::ImGuiDir_Down,
    }
}
