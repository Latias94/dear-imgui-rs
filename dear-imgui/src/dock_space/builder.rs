use crate::dock_layout::{DockspaceConfig, DockspaceHost, submit_and_apply, submit_without_layout};
use crate::{DockLayout, DockLayoutApply, DockNodeFlags, DockspaceError, Id, Ui, WindowClass};

use super::validation::main_viewport_dockspace_id;

/// Canonical builder for a dockspace submission.
///
/// The builder defaults to the main viewport and lets Dear ImGui derive its stable root ID. Use
/// [`Self::current_window`] plus [`Self::root_id`] for an explicit host window. Adding a layout
/// makes validation, staging, replacement, and rollback part of the same submission transaction.
#[must_use = "a dockspace builder does nothing until build() is called"]
pub struct DockspaceBuilder<'ui, 'layout> {
    ui: &'ui Ui,
    root_id: Option<Id>,
    host: DockspaceHost,
    flags: DockNodeFlags,
    window_class: Option<WindowClass>,
    layout: Option<(&'layout DockLayout, DockLayoutApply)>,
}

impl<'ui> DockspaceBuilder<'ui, 'static> {
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            ui,
            root_id: None,
            host: DockspaceHost::MainViewport,
            flags: DockNodeFlags::NONE,
            window_class: None,
            layout: None,
        }
    }
}

impl<'ui, 'layout> DockspaceBuilder<'ui, 'layout> {
    /// Use an explicit stable root ID.
    pub fn root_id(mut self, root_id: Id) -> Self {
        self.root_id = Some(root_id);
        self
    }

    /// Host the dockspace over the main viewport.
    pub fn main_viewport(mut self) -> Self {
        self.host = DockspaceHost::MainViewport;
        self
    }

    /// Host the dockspace at the current window cursor with a positive size.
    pub fn current_window(mut self, size: [f32; 2]) -> Self {
        self.host = DockspaceHost::CurrentWindow { size };
        self
    }

    /// Set the public dock node flags used for submission.
    pub fn flags(mut self, flags: DockNodeFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the optional window class used by the dockspace.
    pub fn window_class(mut self, window_class: WindowClass) -> Self {
        self.window_class = Some(window_class);
        self
    }

    /// Apply a complete declarative layout with the selected persistence policy.
    pub fn layout<'next>(
        self,
        layout: &'next DockLayout,
        apply: DockLayoutApply,
    ) -> DockspaceBuilder<'ui, 'next> {
        DockspaceBuilder {
            ui: self.ui,
            root_id: self.root_id,
            host: self.host,
            flags: self.flags,
            window_class: self.window_class,
            layout: Some((layout, apply)),
        }
    }

    /// Validate and submit the dockspace.
    ///
    /// Call this before any window hosted by the target dock tree. A failed declarative layout
    /// application preserves an existing root and its persisted settings.
    pub fn build(self) -> Result<Id, DockspaceError> {
        let submission = self.host;
        let root_id = match self.host {
            DockspaceHost::MainViewport => match self.root_id {
                Some(root_id) if root_id.raw() == 0 => return Err(DockspaceError::ZeroRootId),
                Some(root_id) => root_id,
                None => resolve_main_viewport_dockspace_id(self.ui),
            },
            DockspaceHost::CurrentWindow { .. } => {
                let root_id = self.root_id.ok_or(DockspaceError::MissingRootId)?;
                if root_id.raw() == 0 {
                    return Err(DockspaceError::ZeroRootId);
                }
                root_id
            }
        };
        let config = DockspaceConfig::new(root_id, self.flags, self.window_class);
        match self.layout {
            Some((layout, apply)) => submit_and_apply(self.ui, &config, layout, apply, submission),
            None => submit_without_layout(self.ui, &config, submission),
        }
    }
}

fn resolve_main_viewport_dockspace_id(ui: &Ui) -> Id {
    ui.run_with_bound_context(|| main_viewport_dockspace_id("DockspaceBuilder::build()", None))
}
