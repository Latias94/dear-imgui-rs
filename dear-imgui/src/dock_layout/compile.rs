use super::model::{DockLayout, DockLayoutApply, DockLayoutError, DockSplit, DockspaceOptions};
use crate::{ConfigFlags, Id, sys, ui::Ui};
use std::collections::HashMap;
use std::ffi::CString;
use std::ptr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NodeIndex(pub(super) usize);

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SplitCommand {
    pub(super) parent: NodeIndex,
    pub(super) direction: DockSplit,
    pub(super) ratio: f32,
    pub(super) first: NodeIndex,
    pub(super) second: NodeIndex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WindowAssignment {
    pub(super) node: NodeIndex,
    pub(super) title: CString,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CompiledLayout {
    pub(super) node_count: usize,
    pub(super) splits: Vec<SplitCommand>,
    pub(super) assignments: Vec<WindowAssignment>,
}

pub(super) fn compile_layout(layout: &DockLayout) -> Result<CompiledLayout, DockLayoutError> {
    let mut splits = Vec::new();
    let mut assignments = Vec::new();
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
                    let native_title = CString::new(title.as_str()).map_err(|_| {
                        DockLayoutError::WindowTitleContainsNul {
                            title: title.clone(),
                        }
                    })?;
                    // SAFETY: `native_title` is readable and NUL-terminated. ImHashStr is a
                    // context-free helper and receives the same arguments as DockBuilderDockWindow.
                    let window_id = unsafe { sys::igImHashStr(native_title.as_ptr(), 0, 0) };
                    if window_id == 0 {
                        return Err(DockLayoutError::EmptyWindowId {
                            title: title.clone(),
                        });
                    }
                    if let Some(first_title) = window_ids.insert(window_id, title.as_str()) {
                        return Err(DockLayoutError::DuplicateWindowId {
                            first_title: first_title.to_owned(),
                            second_title: title.clone(),
                            id: Id::from(window_id),
                        });
                    }
                    assignments.push(WindowAssignment {
                        node,
                        title: native_title,
                    });
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

                splits.push(SplitCommand {
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
        splits,
        assignments,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DockspaceSubmission {
    CurrentWindow { size: [f32; 2] },
    MainViewport,
}

pub(crate) fn submit_and_apply(
    ui: &Ui,
    options: &DockspaceOptions,
    layout: &DockLayout,
    apply: DockLayoutApply,
    submission: DockspaceSubmission,
) -> Result<Id, DockLayoutError> {
    let preflight = (|| {
        options.validate()?;
        if !ui.io().config_flags().contains(ConfigFlags::DOCKING_ENABLE) {
            return Err(DockLayoutError::DockingDisabled);
        }
        let compiled = compile_layout(layout)?;
        let window_class = options
            .window_class_ref()
            .map(crate::WindowClass::try_to_imgui)
            .transpose()?;
        Ok((compiled, window_class))
    })();

    ui.run_with_bound_context(|| {
        let root_id = options.root_id().raw();
        // SAFETY: the Ui's Context is bound and an ID lookup does not dereference Rust memory.
        let existing_node = if root_id == 0 {
            ptr::null_mut()
        } else {
            unsafe { sys::igDockBuilderGetNode(root_id) }
        };
        let existed_before_submission = !existing_node.is_null();
        if existed_before_submission
            && unsafe {
                !sys::ImGuiDockNode_IsRootNode(existing_node)
                    || !sys::ImGuiDockNode_IsDockSpace(existing_node)
            }
        {
            return Err(DockLayoutError::ExistingNodeIsNotDockspaceRoot {
                id: options.root_id(),
            });
        }
        let (compiled, window_class, position, size) =
            match preflight.and_then(|(compiled, window_class)| {
                resolve_host_geometry(ui, submission)
                    .map(|(position, size)| (compiled, window_class, position, size))
            }) {
                Ok(preflight) => preflight,
                Err(error) => {
                    keep_existing_root_alive(root_id, existed_before_submission);
                    return Err(error);
                }
            };
        let window_class_ptr = window_class
            .as_ref()
            .map_or(ptr::null(), |class| class as *const _);
        let visible_claim = crate::dock_space::claim_dockspace_submission(
            ui,
            "Ui dock layout submission",
            options.root_id(),
            options.dock_flags(),
            matches!(submission, DockspaceSubmission::CurrentWindow { .. }),
        )
        .map_err(|_| DockLayoutError::DuplicateDockspaceSubmission {
            root_id: options.root_id(),
        })?;
        let keep_alive_without_layout_changes = apply == DockLayoutApply::IfMissing
            && existed_before_submission
            && visible_claim.is_none();
        if !keep_alive_without_layout_changes
            && (root_has_active_content_window(root_id)
                || compiled_layout_has_active_window(&compiled))
        {
            keep_existing_root_alive(root_id, existed_before_submission);
            return Err(DockLayoutError::WindowSubmittedBeforeDockspace {
                root_id: options.root_id(),
            });
        }

        if apply == DockLayoutApply::IfMissing && existed_before_submission {
            submit_dockspace(
                submission,
                root_id,
                size,
                options.dock_flags().bits(),
                window_class_ptr,
            );
            commit_visible_claim(visible_claim);
            return Ok(options.root_id());
        }

        let frame = unsafe { sys::igGetFrameCount() };
        let layout_claim = ui
            .binding()
            .claim_dock_layout_application(frame, root_id)
            .ok_or(DockLayoutError::DuplicateDockspaceSubmission {
                root_id: options.root_id(),
            })?;
        let result = apply_compiled_layout(
            options.root_id(),
            options.dock_flags().bits(),
            position,
            size,
            &compiled,
            existed_before_submission,
        );
        if let Err(error) = result {
            keep_existing_root_alive(root_id, existed_before_submission);
            return Err(error);
        }

        layout_claim.commit();
        submit_dockspace(
            submission,
            root_id,
            size,
            options.dock_flags().bits(),
            window_class_ptr,
        );
        commit_visible_claim(visible_claim);
        Ok(options.root_id())
    })
}

fn keep_existing_root_alive(root_id: sys::ImGuiID, existed_before_submission: bool) {
    if !existed_before_submission {
        return;
    }
    let kept_alive = unsafe { sys::dear_imgui_rs_dock_builder_keep_root_alive(root_id) } != 0;
    assert!(
        kept_alive,
        "an existing declarative dockspace ID must identify a root node"
    );
}

fn root_has_active_content_window(root_id: sys::ImGuiID) -> bool {
    root_id != 0
        && unsafe { sys::dear_imgui_rs_dock_builder_root_has_active_content_window(root_id) } != 0
}

fn compiled_layout_has_active_window(compiled: &CompiledLayout) -> bool {
    let frame = unsafe { sys::igGetFrameCount() };
    compiled.assignments.iter().any(|assignment| {
        let id = unsafe { sys::igImHashStr(assignment.title.as_ptr(), 0, 0) };
        let window = unsafe { sys::igFindWindowByID(id) };
        !window.is_null() && unsafe { (*window).LastFrameActive == frame }
    })
}

fn resolve_host_geometry(
    ui: &Ui,
    submission: DockspaceSubmission,
) -> Result<([f32; 2], [f32; 2]), DockLayoutError> {
    let (position, size) = match submission {
        DockspaceSubmission::CurrentWindow { size } => {
            let bytes =
                crate::dock_space::current_dockspace_host_name_len("Ui::dock_space_with_layout()");
            if bytes > crate::dock_space::MAX_DOCKSPACE_HOST_NAME_BYTES {
                return Err(DockLayoutError::HostWindowNameTooLong {
                    bytes,
                    max_bytes: crate::dock_space::MAX_DOCKSPACE_HOST_NAME_BYTES,
                });
            }
            (ui.cursor_screen_pos(), size)
        }
        DockspaceSubmission::MainViewport => unsafe {
            let viewport = sys::igGetMainViewport();
            assert!(
                !viewport.is_null(),
                "main viewport must exist during a frame"
            );
            (
                [(*viewport).WorkPos.x, (*viewport).WorkPos.y],
                [(*viewport).WorkSize.x, (*viewport).WorkSize.y],
            )
        },
    };

    if !position.iter().all(|value| value.is_finite()) {
        return Err(DockLayoutError::InvalidHostPosition { position });
    }
    if !size
        .iter()
        .all(|value| *value > 0.0 && crate::dock_space::is_valid_dockspace_size_component(*value))
    {
        return Err(DockLayoutError::InvalidHostSize { size });
    }
    Ok((position, size))
}

fn apply_compiled_layout(
    root_id: Id,
    flags: i32,
    position: [f32; 2],
    size: [f32; 2],
    compiled: &CompiledLayout,
    replacing_existing: bool,
) -> Result<(), DockLayoutError> {
    let final_nodes = if replacing_existing {
        replace_existing_tree(root_id, flags, position, size, compiled)?
    } else {
        build_new_tree(root_id, flags, position, size, compiled)?
    };

    for assignment in &compiled.assignments {
        let node_id = *final_nodes
            .get(assignment.node.0)
            .expect("compiled window assignment references a missing node");
        assert_ne!(
            node_id, 0,
            "compiled window assignment resolved to node zero"
        );
        unsafe {
            sys::igDockBuilderDockWindow(assignment.title.as_ptr(), node_id);
        }
    }
    unsafe {
        sys::igDockBuilderFinish(root_id.raw());
    }
    Ok(())
}

fn build_new_tree(
    root_id: Id,
    flags: i32,
    position: [f32; 2],
    size: [f32; 2],
    compiled: &CompiledLayout,
) -> Result<Vec<sys::ImGuiID>, DockLayoutError> {
    let mut tree = NativeDockTree::create(root_id.raw(), flags)?;
    tree.set_geometry(position, size);
    let nodes = build_topology(tree.root(), compiled)?;
    tree.preserve();
    Ok(nodes)
}

fn replace_existing_tree(
    root_id: Id,
    flags: i32,
    position: [f32; 2],
    size: [f32; 2],
    compiled: &CompiledLayout,
) -> Result<Vec<sys::ImGuiID>, DockLayoutError> {
    let context = unsafe { sys::igGetCurrentContext() };
    assert!(
        !context.is_null(),
        "dock layout replacement requires a Context"
    );
    let staging_id = unsafe { sys::igDockContextGenNodeID(context) };
    assert_ne!(staging_id, 0, "Dear ImGui generated a zero staging node ID");
    assert_ne!(
        staging_id,
        root_id.raw(),
        "Dear ImGui reused the live target root as a staging node"
    );

    let mut staged = NativeDockTree::create(staging_id, flags)?;
    staged.set_geometry(position, size);
    let staged_nodes = build_topology(staged.root(), compiled)?;

    // Allocate every Rust-owned output before the destructive native commit point.
    let mut final_nodes = vec![0; compiled.node_count];
    let staged_indices = staged_nodes
        .iter()
        .copied()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        staged_indices.len(),
        staged_nodes.len(),
        "staged dock tree contains duplicate node IDs"
    );
    let remap_len = compiled
        .node_count
        .checked_mul(2)
        .ok_or(DockLayoutError::LayoutTooLarge)?;
    let remap_capacity = i32::try_from(remap_len).map_err(|_| DockLayoutError::LayoutTooLarge)?;
    let mut remap = vec![0; remap_len];
    let copied = unsafe {
        sys::dear_imgui_rs_dock_builder_copy_node(
            staged.root(),
            root_id.raw(),
            remap.as_mut_ptr(),
            remap_capacity,
        )
    };
    assert_eq!(
        copied, remap_capacity,
        "native dock tree copy rejected a prevalidated remap buffer"
    );
    decode_node_remap(&remap, &staged_indices, root_id.raw(), &mut final_nodes);
    staged.remove_now();
    Ok(final_nodes)
}

fn build_topology(
    root_id: sys::ImGuiID,
    compiled: &CompiledLayout,
) -> Result<Vec<sys::ImGuiID>, DockLayoutError> {
    let mut nodes = vec![0; compiled.node_count];
    nodes[0] = root_id;

    for split in &compiled.splits {
        let parent = *nodes
            .get(split.parent.0)
            .expect("compiled split references a missing parent node");
        assert_ne!(parent, 0, "compiled split parent has not been created");
        let mut first = 0;
        let mut second = 0;
        unsafe {
            sys::igDockBuilderSplitNode(
                parent,
                split_direction_raw(split.direction),
                split.ratio,
                &mut first,
                &mut second,
            );
        }
        if first == 0 || second == 0 {
            return Err(DockLayoutError::SplitFailed {
                direction: split.direction,
                ratio: split.ratio,
            });
        }
        nodes[split.first.0] = first;
        nodes[split.second.0] = second;
    }

    Ok(nodes)
}

fn decode_node_remap(
    remap: &[sys::ImGuiID],
    staged_indices: &HashMap<sys::ImGuiID, usize>,
    final_root: sys::ImGuiID,
    final_nodes: &mut [sys::ImGuiID],
) {
    assert_eq!(
        remap.len(),
        staged_indices.len() * 2,
        "DockBuilderCopyNode returned an incomplete node remap"
    );
    for pair in remap.chunks_exact(2) {
        let index = *staged_indices
            .get(&pair[0])
            .expect("DockBuilderCopyNode returned an unknown staged node");
        let final_id = pair[1];
        assert_ne!(final_id, 0, "DockBuilderCopyNode returned node zero");
        assert_eq!(
            final_nodes[index], 0,
            "DockBuilderCopyNode returned a duplicate staged node"
        );
        final_nodes[index] = final_id;
    }
    assert!(
        final_nodes.iter().all(|id| *id != 0),
        "DockBuilderCopyNode omitted a staged node"
    );
    assert_eq!(
        final_nodes[0], final_root,
        "root remap changed the target ID"
    );
}

fn submit_dockspace(
    submission: DockspaceSubmission,
    root_id: sys::ImGuiID,
    size: [f32; 2],
    flags: i32,
    window_class: *const sys::ImGuiWindowClass,
) {
    let submitted_id = unsafe {
        match submission {
            DockspaceSubmission::CurrentWindow { .. } => sys::igDockSpace(
                root_id,
                sys::ImVec2 {
                    x: size[0],
                    y: size[1],
                },
                flags,
                window_class,
            ),
            DockspaceSubmission::MainViewport => {
                sys::igDockSpaceOverViewport(root_id, sys::igGetMainViewport(), flags, window_class)
            }
        }
    };
    assert_eq!(
        submitted_id, root_id,
        "Dear ImGui returned a different dockspace root ID"
    );
    assert!(
        !unsafe { sys::igDockBuilderGetNode(root_id) }.is_null(),
        "Dear ImGui did not retain the submitted dockspace root"
    );
}

fn commit_visible_claim(claim: Option<crate::context::binding::DockspaceFrameClaim>) {
    let Some(claim) = claim else {
        return;
    };
    claim.commit();
}

struct NativeDockTree {
    root: sys::ImGuiID,
    remove_on_drop: bool,
}

impl NativeDockTree {
    fn create(root: sys::ImGuiID, flags: i32) -> Result<Self, DockLayoutError> {
        let builder_flags = flags | sys::ImGuiDockNodeFlags_DockSpace;
        let added = unsafe { sys::igDockBuilderAddNode(root, builder_flags) };
        if added == 0 {
            return Err(DockLayoutError::NodeCreationFailed { id: Id::from(root) });
        }
        assert_eq!(
            added, root,
            "DockBuilderAddNode changed an explicit node ID"
        );
        Ok(Self {
            root,
            remove_on_drop: true,
        })
    }

    fn root(&self) -> sys::ImGuiID {
        self.root
    }

    fn set_geometry(&self, position: [f32; 2], size: [f32; 2]) {
        unsafe {
            sys::igDockBuilderSetNodePos(
                self.root,
                sys::ImVec2 {
                    x: position[0],
                    y: position[1],
                },
            );
            sys::igDockBuilderSetNodeSize(
                self.root,
                sys::ImVec2 {
                    x: size[0],
                    y: size[1],
                },
            );
        }
    }

    fn preserve(&mut self) {
        self.remove_on_drop = false;
    }

    fn remove_now(&mut self) {
        if self.remove_on_drop {
            unsafe {
                sys::igDockBuilderRemoveNode(self.root);
            }
            self.remove_on_drop = false;
        }
    }
}

impl Drop for NativeDockTree {
    fn drop(&mut self) {
        self.remove_now();
    }
}

fn split_direction_raw(direction: DockSplit) -> sys::ImGuiDir {
    match direction {
        DockSplit::Left => sys::ImGuiDir_Left,
        DockSplit::Right => sys::ImGuiDir_Right,
        DockSplit::Up => sys::ImGuiDir_Up,
        DockSplit::Down => sys::ImGuiDir_Down,
    }
}
