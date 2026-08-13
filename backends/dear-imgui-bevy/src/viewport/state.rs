use super::*;

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Default)]
pub(crate) struct ImguiViewportBridgeState {
    pub(super) commands: Vec<QueuedImguiViewportCommand>,
    pub(super) viewports: HashMap<ImguiViewportInstanceId, ImguiViewportRecord>,
    pub(super) instances_by_id: HashMap<ImguiViewportId, ImguiViewportInstanceId>,
    pub(super) instances_by_native: HashMap<ImguiViewportIdentity, ImguiViewportInstanceId>,
    pub(super) pending_ecs_despawns: HashSet<Entity>,
    pub(super) next_instance_generation: u64,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct QueuedImguiViewportCommand {
    pub(super) instance_id: ImguiViewportInstanceId,
    pub(super) command: ImguiViewportCommand,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug)]
pub(super) struct PendingClientPlacement {
    pub(super) pos: [f32; 2],
    pub(super) dpi_scale: f32,
    pub(super) show_requested: bool,
    pub(super) focus_requested: bool,
}

/// Identifies one exact Dear ImGui viewport without retaining a dereferenceable native pointer.
///
/// Numeric viewport IDs are deliberately excluded: docking may change them in place. Native code
/// validates the retained integer address against the owning Context's complete live registry
/// before Rust creates a reference from it.
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ImguiViewportIdentity {
    pub(super) context_address: usize,
    pub(super) address: usize,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportIdentity {
    pub(super) fn capture(context: *mut sys::ImGuiContext, viewport: &imgui::Viewport) -> Self {
        Self {
            context_address: context as usize,
            address: viewport.as_raw() as usize,
        }
    }

    pub(super) unsafe fn resolve(self) -> Option<*mut sys::ImGuiViewport> {
        let viewport = unsafe {
            sys::ImGuiContext_FindLiveViewportByAddress(
                self.context_address as *mut sys::ImGuiContext,
                self.address,
            )
        };
        (!viewport.is_null()).then_some(viewport)
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub(super) struct ImguiViewportPlatformHandle {
    pub(super) instance_id: ImguiViewportInstanceId,
    pub(super) identity: ImguiViewportIdentity,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Debug)]
pub(super) enum ImguiViewportPlatformHandleState {
    Active(Box<ImguiViewportPlatformHandle>),
    Retired(Box<ImguiViewportPlatformHandle>),
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) struct ImguiViewportRecord {
    pub(super) identity: ImguiViewportIdentity,
    pub(super) current_id: ImguiViewportId,
    pub(super) window: Option<Entity>,
    pub(super) camera: Option<Entity>,
    pub(super) feedback: Option<ImguiViewportFeedback>,
    pub(super) flags: Option<imgui::ViewportFlags>,
    pub(super) show_requested: bool,
    pub(super) native_policy: NativeViewportPolicyState,
    pub(super) pending_client_placement: Option<PendingClientPlacement>,
    pub(super) geometry: geometry::ViewportGeometryReconciler,
    pub(super) handle: Option<ImguiViewportPlatformHandleState>,
    pub(super) focus_next_frame: bool,
    pub(super) focus_ready: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportRecord {
    pub(super) fn new(identity: ImguiViewportIdentity, current_id: ImguiViewportId) -> Self {
        Self {
            identity,
            current_id,
            window: None,
            camera: None,
            feedback: None,
            flags: None,
            show_requested: false,
            native_policy: NativeViewportPolicyState::default(),
            pending_client_placement: None,
            geometry: geometry::ViewportGeometryReconciler::default(),
            handle: None,
            focus_next_frame: false,
            focus_ready: false,
        }
    }

    pub(super) fn clear_ecs_state(&mut self) {
        self.native_policy.release();
        self.window = None;
        self.camera = None;
        self.feedback = None;
        self.flags = None;
        self.show_requested = false;
        self.pending_client_placement = None;
        self.geometry = geometry::ViewportGeometryReconciler::default();
        self.focus_next_frame = false;
        self.focus_ready = false;
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Copy)]
pub(super) struct ImguiViewportHandleRef {
    pub(super) identity: ImguiViewportIdentity,
    pub(super) pointer: *mut c_void,
    pub(super) recreate_platform_window: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeState {
    pub(super) fn next_instance_id(
        &mut self,
        context_id: imgui::ContextId,
    ) -> Result<ImguiViewportInstanceId, ImguiViewportRuntimeError> {
        let next_generation = self
            .next_instance_generation
            .checked_add(1)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceGenerationExhausted)?;
        let generation = std::num::NonZeroU64::new(next_generation)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceGenerationExhausted)?;
        self.next_instance_generation = next_generation;
        Ok(ImguiViewportInstanceId {
            context_id,
            generation,
        })
    }

    pub(super) fn remove_instance(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<ImguiViewportRecord> {
        let record = self.viewports.remove(&instance_id)?;
        if self.instances_by_id.get(&record.current_id) == Some(&instance_id) {
            self.instances_by_id.remove(&record.current_id);
        }
        if self.instances_by_native.get(&record.identity) == Some(&instance_id) {
            self.instances_by_native.remove(&record.identity);
        }
        Some(record)
    }

    pub(super) fn evict_dead_id_owner(
        &mut self,
        current_id: ImguiViewportId,
        incoming: ImguiViewportInstanceId,
    ) -> Result<(), ImguiViewportRuntimeError> {
        let Some(existing) = self.instances_by_id.get(&current_id).copied() else {
            return Ok(());
        };
        if existing == incoming {
            return Ok(());
        }
        let existing_is_live_or_claimed = self.viewports.get(&existing).is_some_and(|record| {
            matches!(
                record.handle.as_ref(),
                Some(ImguiViewportPlatformHandleState::Active(_))
            ) || unsafe { record.identity.resolve().is_some() }
        });
        if existing_is_live_or_claimed {
            return Err(ImguiViewportRuntimeError::ViewportIdCollision {
                viewport_id: current_id,
            });
        }
        if let Some(record) = self.remove_instance(existing) {
            self.pending_ecs_despawns
                .extend(record.window.into_iter().chain(record.camera));
        }
        Ok(())
    }

    pub(super) fn bind_current_id(
        &mut self,
        instance_id: ImguiViewportInstanceId,
        current_id: ImguiViewportId,
    ) -> Result<(), ImguiViewportRuntimeError> {
        self.evict_dead_id_owner(current_id, instance_id)?;
        let Some(previous_id) = self
            .viewports
            .get(&instance_id)
            .map(|record| record.current_id)
        else {
            return Err(ImguiViewportRuntimeError::ViewportInstanceUnavailable);
        };
        if previous_id == current_id {
            self.instances_by_id.insert(current_id, instance_id);
            return Ok(());
        }
        if self.instances_by_id.get(&previous_id) == Some(&instance_id) {
            self.instances_by_id.remove(&previous_id);
        }
        self.viewports
            .get_mut(&instance_id)
            .expect("the viewport record was checked above")
            .current_id = current_id;
        self.instances_by_id.insert(current_id, instance_id);
        Ok(())
    }

    pub(super) fn register_viewport(
        &mut self,
        context_id: imgui::ContextId,
        identity: ImguiViewportIdentity,
        current_id: ImguiViewportId,
    ) -> Result<ImguiViewportInstanceId, ImguiViewportRuntimeError> {
        if let Some(instance_id) = self.instances_by_native.get(&identity).copied() {
            let retains_sidecar = self
                .record(instance_id)
                .is_some_and(|record| record.handle.is_some());
            if retains_sidecar {
                self.bind_current_id(instance_id, current_id)?;
                return Ok(instance_id);
            }
            if let Some(record) = self.remove_instance(instance_id) {
                self.pending_ecs_despawns
                    .extend(record.window.into_iter().chain(record.camera));
            }
        }
        let instance_id = self.next_instance_id(context_id)?;
        self.evict_dead_id_owner(current_id, instance_id)?;
        self.viewports
            .insert(instance_id, ImguiViewportRecord::new(identity, current_id));
        self.instances_by_native.insert(identity, instance_id);
        self.instances_by_id.insert(current_id, instance_id);
        Ok(instance_id)
    }

    pub(super) fn queue(
        &mut self,
        instance_id: ImguiViewportInstanceId,
        current_id: ImguiViewportId,
        command: ImguiViewportCommand,
    ) -> Result<(), ImguiViewportRuntimeError> {
        self.bind_current_id(instance_id, current_id)?;
        self.commands.push(QueuedImguiViewportCommand {
            instance_id,
            command,
        });
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn queue_for_test(
        &mut self,
        context_id: imgui::ContextId,
        command: ImguiViewportCommand,
    ) {
        let current_id = command.current_id();
        let instance_id = self.instance_for_id(current_id).unwrap_or_else(|| {
            self.register_viewport(
                context_id,
                ImguiViewportIdentity {
                    context_address: 0,
                    address: current_id.raw() as usize + 1,
                },
                current_id,
            )
            .expect("a synthetic test viewport route should be registerable")
        });
        self.queue(instance_id, current_id, command)
            .expect("a synthetic test viewport command should be queueable");
    }

    pub(super) fn instance_for_id(
        &self,
        viewport_id: ImguiViewportId,
    ) -> Option<ImguiViewportInstanceId> {
        self.instances_by_id.get(&viewport_id).copied()
    }

    pub(super) fn record(
        &self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<&ImguiViewportRecord> {
        self.viewports.get(&instance_id)
    }

    pub(super) fn record_mut(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<&mut ImguiViewportRecord> {
        self.viewports.get_mut(&instance_id)
    }

    pub(super) fn platform_handle(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<*mut c_void> {
        let record = self.record_mut(instance_id)?;
        let handle = match record.handle.take() {
            Some(ImguiViewportPlatformHandleState::Active(handle))
            | Some(ImguiViewportPlatformHandleState::Retired(handle)) => handle,
            None => Box::new(ImguiViewportPlatformHandle {
                instance_id,
                identity: record.identity,
            }),
        };
        debug_assert_eq!(handle.instance_id, instance_id);
        debug_assert_eq!(handle.identity, record.identity);
        let pointer = (&*handle as *const ImguiViewportPlatformHandle)
            .cast_mut()
            .cast::<c_void>();
        record.handle = Some(ImguiViewportPlatformHandleState::Active(handle));
        Some(pointer)
    }

    pub(super) fn take_platform_handle(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<Box<ImguiViewportPlatformHandle>> {
        match self.record_mut(instance_id)?.handle.take()? {
            ImguiViewportPlatformHandleState::Active(handle)
            | ImguiViewportPlatformHandleState::Retired(handle) => Some(handle),
        }
    }

    pub(super) fn retire_platform_handle(
        &mut self,
        instance_id: ImguiViewportInstanceId,
    ) -> Option<*mut c_void> {
        let record = self.record_mut(instance_id)?;
        let handle = match record.handle.take()? {
            ImguiViewportPlatformHandleState::Active(handle)
            | ImguiViewportPlatformHandleState::Retired(handle) => handle,
        };
        let pointer = (&*handle as *const ImguiViewportPlatformHandle)
            .cast_mut()
            .cast::<c_void>();
        record.handle = Some(ImguiViewportPlatformHandleState::Retired(handle));
        Some(pointer)
    }

    pub(super) fn validate_callback_handle(
        &self,
        instance_id: ImguiViewportInstanceId,
        viewport: &imgui::Viewport,
    ) -> Result<(), ImguiViewportRuntimeError> {
        let record = self
            .record(instance_id)
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?;
        let state = record
            .handle
            .as_ref()
            .ok_or(ImguiViewportRuntimeError::ViewportInstanceUnavailable)?;
        let (handle, active) = match state {
            ImguiViewportPlatformHandleState::Active(handle) => (handle, true),
            ImguiViewportPlatformHandleState::Retired(handle) => (handle, false),
        };
        if handle.instance_id != instance_id || handle.identity != record.identity {
            return Err(ImguiViewportRuntimeError::ViewportInstanceUnavailable);
        }
        let expected = (&**handle as *const ImguiViewportPlatformHandle)
            .cast_mut()
            .cast::<c_void>();
        let expected_claim = if active {
            expected
        } else {
            std::ptr::null_mut()
        };
        for (actual, field) in [
            (viewport.platform_user_data(), "PlatformUserData"),
            (viewport.platform_handle(), "PlatformHandle"),
        ] {
            if actual != expected_claim {
                return Err(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::ViewportFieldReplaced { field },
                ));
            }
        }
        let raw = viewport.platform_handle_raw();
        if !raw.is_null() && raw != expected_claim {
            return Err(ImguiViewportRuntimeError::CallbackOwnership(
                ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                    field: "PlatformHandleRaw",
                },
            ));
        }
        Ok(())
    }

    pub(super) fn retire_stale_platform_handles(
        &mut self,
        live_viewports: &HashSet<ImguiViewportInstanceId>,
    ) {
        for (instance_id, record) in &mut self.viewports {
            if live_viewports.contains(instance_id) {
                continue;
            }
            if let Some(ImguiViewportPlatformHandleState::Active(handle)) = record.handle.take() {
                record.handle = Some(ImguiViewportPlatformHandleState::Retired(handle));
            }
        }
    }

    pub(super) fn set_viewport_flags(
        &mut self,
        instance_id: ImguiViewportInstanceId,
        flags: imgui::ViewportFlags,
    ) -> Option<imgui::ViewportFlags> {
        self.record_mut(instance_id)?.flags.replace(flags)
    }
}
