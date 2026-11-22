use crate::{EditorConfig, EditorConfigSnapshot, NodeEditorStyle, StyleColor, sys};
use dear_imgui_rs::{Context as ImGuiContext, ContextBinding, ContextId, ContextThreadLease};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr,
};

/// Errors returned by the node-editor safe layer.
#[derive(Debug, thiserror::Error)]
pub enum NodeEditorError {
    #[error("imgui-node-editor CreateEditor returned null")]
    CreateEditorFailed,
    #[error(transparent)]
    ThreadLease(#[from] dear_imgui_rs::ImGuiError),
}

/// Owned imgui-node-editor context.
pub struct EditorContext {
    raw: *mut sys::DneEditorContext,
    imgui_binding: ContextBinding,
    config: EditorConfigSnapshot,
    _settings_file: Option<std::ffi::CString>,
    _callbacks: Option<Box<crate::config::CallbackState>>,
    pub(crate) frame_active: Cell<bool>,
    _thread_lease: ContextThreadLease,
}

impl EditorContext {
    pub fn create(imgui: &ImGuiContext) -> Self {
        Self::try_create_with_config(imgui, EditorConfig::default())
            .expect("failed to create imgui-node-editor context")
    }

    pub fn create_with_config(imgui: &ImGuiContext, config: EditorConfig) -> Self {
        Self::try_create_with_config(imgui, config)
            .expect("failed to create imgui-node-editor context")
    }

    pub fn try_create_with_config(
        imgui: &ImGuiContext,
        mut config: EditorConfig,
    ) -> Result<Self, NodeEditorError> {
        let thread_lease = ContextThreadLease::acquire()?;
        let imgui_binding = imgui.binding();
        let config_snapshot = config.snapshot();
        let raw_config = config.as_sys();
        let raw =
            imgui_binding.with_bound_context(|| unsafe { sys::dne_create_editor(&raw_config) });
        if raw.is_null() {
            return Err(NodeEditorError::CreateEditorFailed);
        }

        Ok(Self {
            raw,
            imgui_binding,
            config: config_snapshot,
            _settings_file: config.settings_file.take(),
            _callbacks: config.callbacks.take(),
            frame_active: Cell::new(false),
            _thread_lease: thread_lease,
        })
    }

    pub fn as_raw(&self) -> *mut sys::DneEditorContext {
        self.raw
    }

    pub fn as_raw_native(&self) -> *mut c_void {
        self.with_current("EditorContext::as_raw_native", || unsafe {
            sys::dne_editor_context_raw(self.raw)
        })
    }

    /// Returns the stable identity of the owning Dear ImGui context.
    pub fn imgui_context_id(&self) -> ContextId {
        self.imgui_binding.id()
    }

    #[doc(alias = "GetConfig")]
    pub fn config(&self) -> &EditorConfigSnapshot {
        &self.config
    }

    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> NodeEditorStyle {
        self.with_current("EditorContext::style", NodeEditorStyle::current)
    }

    pub fn set_style(&self, style: &NodeEditorStyle) {
        self.with_current("EditorContext::set_style", || style.apply());
    }

    pub fn style_color(&self, color: StyleColor) -> [f32; 4] {
        self.with_current("EditorContext::style_color", || {
            crate::style::current_style_color(color)
        })
    }

    pub fn set_style_color(&self, color: StyleColor, value: [f32; 4]) {
        self.with_current("EditorContext::set_style_color", || {
            crate::style::apply_style_color(color, value)
        });
    }

    pub(crate) fn assert_usable(&self, caller: &str) {
        assert!(
            self.imgui_binding.is_alive(),
            "{caller} requires the owning Dear ImGui context to be alive"
        );
        assert!(
            !self.raw.is_null(),
            "{caller} requires a valid node-editor context"
        );
    }

    #[inline]
    pub(crate) fn with_current<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.assert_usable(caller);
        self.imgui_binding.with_bound_context(|| {
            let _current = CurrentEditorGuard::bind(self.raw);
            f()
        })
    }
}

impl Drop for EditorContext {
    fn drop(&mut self) {
        if self.raw.is_null() {
            return;
        }

        let raw = std::mem::replace(&mut self.raw, ptr::null_mut());
        let destroy = || {
            // DestroyEditor runs settings callbacks. Bind its target first so Rust
            // owns any restoration, including if a callback drops the previous editor.
            let _current = CurrentEditorGuard::bind(raw);
            let native = unsafe { sys::dne_editor_context_raw(raw) };
            SAVED_EDITORS.with_borrow_mut(|stack| {
                for previous in stack.iter_mut().filter(|previous| **previous == native) {
                    *previous = ptr::null_mut();
                }
            });
            unsafe { sys::dne_destroy_editor(raw) };
        };
        if self.imgui_binding.try_with_bound_context(destroy).is_err() {
            // The shim owns the handle even after Dear ImGui teardown.
            destroy();
        }
    }
}

thread_local! {
    static SAVED_EDITORS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

struct CurrentEditorGuard {
    restore: bool,
}

impl CurrentEditorGuard {
    fn bind(editor: *mut sys::DneEditorContext) -> Self {
        let previous = unsafe { sys::dne_get_current_editor_raw() };
        let target = unsafe { sys::dne_editor_context_raw(editor) };
        let restore = previous != target;
        if restore {
            SAVED_EDITORS.with_borrow_mut(|stack| stack.push(previous));
            unsafe { sys::dne_set_current_editor_raw(target) };
        }
        Self { restore }
    }
}

impl Drop for CurrentEditorGuard {
    fn drop(&mut self) {
        if self.restore {
            let previous = SAVED_EDITORS.with_borrow_mut(|stack| stack.pop().unwrap());
            unsafe { sys::dne_set_current_editor_raw(previous) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorConfig, LinkId, NodeEditorUiExt, NodeId, PinId, PinKind, StyleColor, StyleVar,
    };
    use dear_imgui_rs::MouseButton;
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        ptr,
        rc::Rc,
        sync::{Mutex, OnceLock},
    };

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn drop_restores_previous_imgui_context() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let raw_imgui = imgui.as_raw();
        let editor = EditorContext::create(&imgui);

        unsafe { dear_imgui_rs::sys::igSetCurrentContext(ptr::null_mut()) };
        drop(editor);

        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            ptr::null_mut()
        );
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_imgui) };
    }

    #[test]
    fn current_editor_guard_restores_previous_editor() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let editor_a = EditorContext::create(&imgui);
        let editor_b = EditorContext::create(&imgui);
        let raw_a = editor_a.as_raw_native();
        let raw_b = editor_b.as_raw_native();

        unsafe { sys::dne_set_current_editor_raw(raw_a) };
        {
            editor_b.with_current("test", || {
                assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);
            });
        }
        assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_a);

        unsafe { sys::dne_set_current_editor_raw(ptr::null_mut()) };
    }

    #[test]
    fn nested_binding_does_not_restore_a_released_editor() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let previous = EditorContext::create(&imgui);
        let current = EditorContext::create(&imgui);
        let target = current.as_raw_native();
        unsafe { sys::dne_set_current_editor_raw(previous.as_raw_native()) };
        current.with_current("settings callback binding", || {
            drop(previous);
            assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, target);
        });
        assert!(unsafe { sys::dne_get_current_editor_raw() }.is_null());
    }

    #[test]
    fn binding_identity_matches_owning_imgui_context() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let editor = EditorContext::create(&imgui);

        assert_eq!(editor.imgui_context_id(), imgui.id());
    }

    #[test]
    fn panic_restores_previous_imgui_and_editor_contexts() {
        let _guard = test_guard();
        let imgui_a = ImGuiContext::create();
        let raw_imgui_a = imgui_a.as_raw();
        let editor_a = EditorContext::create(&imgui_a);
        let raw_editor_a = editor_a.as_raw_native();
        let suspended_a = imgui_a.suspend_or_panic();

        let imgui_b = ImGuiContext::create();
        let raw_imgui_b = imgui_b.as_raw();
        let editor_b = EditorContext::create(&imgui_b);
        let raw_editor_b = editor_b.as_raw_native();
        unsafe { sys::dne_set_current_editor_raw(raw_editor_b) };

        let result = catch_unwind(AssertUnwindSafe(|| {
            editor_a.with_current("panic restoration test", || {
                assert_eq!(
                    unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                    raw_imgui_a
                );
                assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_editor_a);
                panic!("intentional binding panic");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            raw_imgui_b
        );
        assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_editor_b);

        unsafe { sys::dne_set_current_editor_raw(ptr::null_mut()) };
        drop(editor_b);
        drop(imgui_b);
        let imgui_a = suspended_a.activate().expect("context A should reactivate");
        drop(editor_a);
        drop(imgui_a);
    }

    #[test]
    fn dead_imgui_context_rejects_calls_but_editor_still_drops() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let editor = EditorContext::create(&imgui);
        drop(imgui);

        let result = catch_unwind(AssertUnwindSafe(|| editor.style()));
        assert!(result.is_err());
        assert!(
            std::thread::spawn(|| ContextThreadLease::acquire().is_err())
                .join()
                .unwrap()
        );
        drop(editor);
    }

    #[test]
    fn dead_editor_drop_preserves_another_current_context() {
        let _guard = test_guard();
        let imgui_a = ImGuiContext::create();
        let editor_a = EditorContext::create(&imgui_a);
        let suspended_a = imgui_a.suspend_or_panic();

        let imgui_b = ImGuiContext::create();
        let raw_imgui_b = imgui_b.as_raw();
        let editor_b = EditorContext::create(&imgui_b);
        let raw_editor_b = editor_b.as_raw_native();
        unsafe { sys::dne_set_current_editor_raw(raw_editor_b) };

        drop(suspended_a);
        drop(editor_a);

        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            raw_imgui_b
        );
        assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_editor_b);

        unsafe { sys::dne_set_current_editor_raw(ptr::null_mut()) };
        drop(editor_b);
        drop(imgui_b);
    }

    #[test]
    fn editor_drops_during_imgui_teardown_without_rebinding() {
        struct Marker;
        struct DropEditorOnQuiesce {
            editor: RefCell<Option<EditorContext>>,
            quiesced: Cell<bool>,
        }

        impl dear_imgui_rs::ContextAttachment for DropEditorOnQuiesce {
            fn quiesce(
                &self,
                _context: &dear_imgui_rs::ContextTeardown<'_>,
            ) -> Result<(), dear_imgui_rs::ContextAttachmentTeardownError> {
                drop(self.editor.borrow_mut().take());
                self.quiesced.set(true);
                Ok(())
            }
        }

        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        let attachment = Rc::new(DropEditorOnQuiesce {
            editor: RefCell::new(Some(EditorContext::create(&imgui))),
            quiesced: Cell::new(false),
        });
        let _lease = imgui
            .register_attachment::<Marker>(
                dear_imgui_rs::ContextAttachmentRole::Extension,
                attachment.clone(),
            )
            .expect("attachment should register");

        drop(imgui);

        assert!(attachment.quiesced.get());
        assert!(attachment.editor.borrow().is_none());
    }

    #[test]
    fn ui_from_another_imgui_context_is_rejected_by_identity() {
        let _guard = test_guard();
        let imgui_a = ImGuiContext::create();
        let editor = EditorContext::create(&imgui_a);
        let suspended_a = imgui_a.suspend_or_panic();

        let mut imgui_b = ImGuiContext::create();
        imgui_b.io_mut().set_display_size([640.0, 480.0]);
        imgui_b.io_mut().set_delta_time(1.0 / 60.0);
        imgui_b
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
        let ui = imgui_b.frame();

        let result = catch_unwind(AssertUnwindSafe(|| {
            ui.with_node_editor(&editor, "wrong-context", [320.0, 240.0], |_| ());
        }));
        assert!(result.is_err());
        drop(imgui_b.render_legacy());

        drop(imgui_b);
        let imgui_a = suspended_a.activate().expect("context A should reactivate");
        drop(editor);
        drop(imgui_a);
    }

    #[test]
    fn creating_editor_does_not_break_imgui_frame() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();

        let _editor = EditorContext::create(&imgui);

        imgui.frame();
        drop(imgui.render_legacy());
    }

    #[test]
    fn frame_safe_api_calls_do_not_break_imgui_frame() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();

        let editor_context = EditorContext::create(&imgui);
        let node_a = NodeId::new(1);
        let node_b = NodeId::new(2);
        let output_pin = PinId::new(11);
        let input_pin = PinId::new(21);
        let link = LinkId::new(100);

        let ui = imgui.frame();
        ui.window("node-editor-frame-api").build(|| {
            ui.with_node_editor(&editor_context, "frame-api", [320.0, 240.0], |editor| {
                assert!(!editor.is_suspended());
                editor.with_suspended(|| assert!(editor.is_suspended()));
                assert!(!editor.is_suspended());

                editor.set_shortcuts_enabled(false);
                assert!(!editor.shortcuts_enabled());
                editor.set_shortcuts_enabled(true);

                editor.set_node_position(node_a, [20.0, 30.0]);
                editor.set_node_z_position(node_a, 2.0);
                let _ = editor.node_z_position(node_a);
                editor.restore_node_state(node_a);

                editor.node(node_a, |node| {
                    node.pin(output_pin, PinKind::Output, |pin| {
                        ui.text("out");
                        let cursor = ui.cursor_screen_pos();
                        pin.rect(cursor, [cursor[0] + 8.0, cursor[1] + 8.0]);
                        pin.pivot_rect(cursor, [cursor[0] + 8.0, cursor[1] + 8.0]);
                        pin.pivot_size([8.0, 8.0]);
                        pin.pivot_scale([1.0, 1.0]);
                        pin.pivot_alignment([0.5, 0.5]);
                        editor.with_suspended(|| {
                            editor.with_suspended(|| assert!(editor.is_suspended()));
                            assert!(editor.is_suspended());
                        });
                        assert!(!editor.is_suspended());
                    });
                });
                editor.node(node_b, |node| {
                    node.pin(input_pin, PinKind::Input, |_| ui.text("in"));
                });

                editor.group_hint(node_a, |_| ());
                let _ = editor.node_background_draw_list(node_a);
                let _ = editor.link(link, output_pin, input_pin);
                let _ = editor.link_pins(link);
                let _ = editor.node_has_any_links(node_a);
                let _ = editor.pin_has_any_links(output_pin);
                let _ = editor.pin_had_any_links(output_pin);

                editor.select_node(node_a);
                editor.add_node_to_selection(node_b);
                let _ = editor.is_node_selected(node_a);
                editor.deselect_node(node_a);
                editor.select_link(link);
                editor.add_link_to_selection(link);
                let _ = editor.is_link_selected(link);
                editor.deselect_link(link);
                editor.clear_selection();

                let _ = editor.has_selection_changed();
                let _ = editor.selected_object_count();
                let _ = editor.is_active();
                let _ = editor.is_background_clicked();
                let _ = editor.is_background_double_clicked();
                let _ = editor.background_click_button();
                let _ = editor.background_double_click_button();
                let _ = editor.screen_size();
                let _ = editor.screen_to_canvas([10.0, 10.0]);
                let _ = editor.canvas_to_screen([10.0, 10.0]);
                let _ = editor.node_count();
                let _ = editor.ordered_node_ids();
            });
        });
        drop(imgui.render_legacy());
    }

    #[test]
    fn node_background_requires_an_ended_node_submitted_in_the_current_frame() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
        let context = EditorContext::create(&imgui);
        let node_id = NodeId::new(1);

        for frame_index in 0..2 {
            let ui = imgui.frame();
            ui.window("node-background-contract")
                .size([500.0, 350.0], dear_imgui_rs::Condition::Always)
                .build(|| {
                    ui.with_node_editor(&context, "background", [400.0, 260.0], |frame| {
                        assert!(frame.node_background_draw_list(node_id).is_none());
                        if frame_index == 0 {
                            frame.node(node_id, |_| {
                                ui.text("Node");
                                assert!(
                                    catch_unwind(AssertUnwindSafe(|| {
                                        let _ = frame.node_background_draw_list(node_id);
                                    }))
                                    .is_err()
                                );
                                assert!(
                                    catch_unwind(AssertUnwindSafe(|| {
                                        frame.group_hint(node_id, |_| ());
                                    }))
                                    .is_err()
                                );
                            });
                            assert!(frame.node_background_draw_list(node_id).is_some());
                        }
                    });
                });
            drop(imgui.render_legacy());
        }
    }

    #[test]
    fn group_hint_scopes_reject_reentry_without_ending_the_active_hint() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();

        struct ZoomedOutSettings;
        impl crate::SettingsHandler for ZoomedOutSettings {
            fn load_settings(&mut self) -> Option<Vec<u8>> {
                Some(br#"{"view":{"zoom":0.5}}"#.to_vec())
            }
        }
        let context = EditorContext::create_with_config(
            &imgui,
            EditorConfig::new().settings_handler(ZoomedOutSettings),
        );
        let node_id = NodeId::new(1);
        let ui = imgui.frame();
        ui.window("group-hint-contract")
            .size([500.0, 350.0], dear_imgui_rs::Condition::Always)
            .build(|| {
                ui.with_node_editor(&context, "hints", [400.0, 260.0], |frame| {
                    // Upstream reports the inverse of the persisted canvas scale.
                    assert!((frame.current_zoom() - 2.0).abs() < 0.001);
                    assert!(frame.group_hint(NodeId::new(999), |_| ()).is_none());
                    frame.node(node_id, |_| frame.group([100.0, 80.0]));

                    frame
                        .group_hint(node_id, |hint| {
                            let min = hint.min();
                            let max = hint.max();
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| {
                                    frame.group_hint(node_id, |_| ());
                                }))
                                .is_err()
                            );
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| {
                                    frame.node(NodeId::new(2), |_| ());
                                }))
                                .is_err()
                            );
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| {
                                    let _ = frame.node_background_draw_list(node_id);
                                }))
                                .is_err()
                            );
                            assert_eq!(hint.min(), min);
                            assert_eq!(hint.max(), max);
                        })
                        .expect("zoomed-out group has a hint");

                    frame.group_hint(node_id, |_| ()).unwrap();
                    frame.node(NodeId::new(2), |_| ());
                });
            });
        drop(imgui.render_legacy());
    }

    #[test]
    fn deletion_candidates_must_be_queried_and_consumed_once() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let context = EditorContext::create(&imgui);
        let first = NodeId::new(1);
        let second = NodeId::new(2);
        for frame_index in 0..2 {
            let ui = imgui.frame();
            ui.window("delete-candidate-contract")
                .size([500.0, 350.0], dear_imgui_rs::Condition::Always)
                .build(|| {
                    ui.with_node_editor(&context, "delete", [400.0, 260.0], |frame| {
                        frame.node(first, |_| ui.text("First"));
                        frame.node(second, |_| ui.text("Second"));
                        if frame_index == 0 {
                            assert!(frame.delete_node(first));
                            assert!(frame.delete_node(second));
                        } else {
                            let delete = frame
                                .begin_delete()
                                .expect("queued nodes should be offered");
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| delete.accept_deleted_item(true)))
                                    .is_err()
                            );
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| delete.reject_deleted_item()))
                                    .is_err()
                            );
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| frame.begin_delete())).is_err()
                            );
                            assert_eq!(delete.query_deleted_node(), Some(first));
                            assert!(delete.accept_deleted_item(true));
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| delete.accept_deleted_item(true)))
                                    .is_err()
                            );
                            assert_eq!(delete.query_deleted_node(), Some(second));
                            delete.reject_deleted_item();
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| delete.reject_deleted_item()))
                                    .is_err()
                            );
                            assert_eq!(delete.query_deleted_node(), None);
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| delete.accept_deleted_item(true)))
                                    .is_err()
                            );
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| delete.reject_deleted_item()))
                                    .is_err()
                            );
                            delete.end();
                        }
                        assert!(frame.with_create([1.0; 4], 1.0, |_, _| ()).is_none());
                        assert!(frame.with_create([1.0; 4], 1.0, |_, _| ()).is_none());
                    });
                });
            drop(imgui.render_legacy());
        }
    }

    #[test]
    fn pin_ids_are_unique_per_editor_begin_and_reusable_after_it_ends() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let context = EditorContext::create(&imgui);
        let pin_id = PinId::new(11);
        for _ in 0..2 {
            let ui = imgui.frame();
            ui.window("unique-pin-contract")
                .size([500.0, 350.0], dear_imgui_rs::Condition::Always)
                .build(|| {
                    for _ in 0..2 {
                        ui.with_node_editor(&context, "pins", [200.0, 100.0], |frame| {
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| {
                                    ui.with_node_editor(&context, "nested", [20.0, 20.0], |_| ());
                                }))
                                .is_err()
                            );
                            frame.node(NodeId::new(1), |node| {
                                node.pin(pin_id, PinKind::Output, |_| {
                                    ui.text("Output");
                                    assert!(
                                        catch_unwind(AssertUnwindSafe(|| node.pin(
                                            PinId::new(12),
                                            PinKind::Output,
                                            |_| ()
                                        )))
                                        .is_err()
                                    );
                                    assert!(
                                        catch_unwind(
                                            AssertUnwindSafe(|| frame.group([20.0, 20.0]))
                                        )
                                        .is_err()
                                    );
                                });
                                assert!(
                                    catch_unwind(AssertUnwindSafe(|| node.pin(
                                        pin_id,
                                        PinKind::Output,
                                        |_| ()
                                    )))
                                    .is_err()
                                );
                            });
                            frame.node(NodeId::new(2), |second| {
                                assert!(
                                    catch_unwind(AssertUnwindSafe(|| second.pin(
                                        pin_id,
                                        PinKind::Input,
                                        |_| ()
                                    )))
                                    .is_err()
                                );
                            });
                        });
                    }
                });
            drop(imgui.render_legacy());
        }
    }

    #[test]
    fn frame_and_phase_guard_panics_unwind_tokens_before_reusing_the_editor() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let context = EditorContext::create(&imgui);
        let ui = imgui.frame();
        ui.window("guard-unwind-contract")
            .size([500.0, 350.0], dear_imgui_rs::Condition::Always)
            .build(|| {
                for reject_frame_reentry in [true, false] {
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| {
                            ui.with_node_editor(&context, "unwind", [200.0, 80.0], |frame| {
                                frame.node(NodeId::new(1), |node| {
                                    node.pin(PinId::new(11), PinKind::Output, |_| {
                                        ui.text("Output");
                                        if reject_frame_reentry {
                                            ui.with_node_editor(
                                                &context,
                                                "nested",
                                                [20.0, 20.0],
                                                |_| (),
                                            );
                                        } else {
                                            let _ = frame.begin_delete();
                                        }
                                    });
                                });
                            });
                        }))
                        .is_err()
                    );
                    assert!(!context.frame_active.get());
                }
                ui.with_node_editor(&context, "unwind", [200.0, 80.0], |frame| {
                    frame.node(NodeId::new(1), |node| {
                        node.pin(PinId::new(11), PinKind::Output, |_| ());
                    });
                });
                assert!(!context.frame_active.get());
            });
        drop(imgui.render_legacy());
        drop(context);
    }

    #[test]
    fn frame_tokens_bind_own_editor_before_drop_and_restore_previous_editor() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();

        let editor_a = EditorContext::create(&imgui);
        let editor_b = EditorContext::create(&imgui);
        let raw_b = editor_b.as_raw_native();

        let ui = imgui.frame();
        ui.window("node-editor-token-context").build(|| {
            ui.with_node_editor(&editor_a, "token-context", [320.0, 240.0], |frame| {
                let original = frame.style().link_strength;
                let outer = frame.push_style_var_float(StyleVar::LinkStrength, 0.75);
                let inner = frame.push_style_var_float(StyleVar::LinkStrength, 0.5);
                unsafe { sys::dne_set_current_editor_raw(raw_b) };
                outer.pop();
                assert_eq!(frame.style().link_strength, 0.5);
                drop(inner);
                assert_eq!(frame.style().link_strength, original);
                assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);

                frame.with_suspended(|| {
                    let local = frame.push_style_var_float(StyleVar::LinkStrength, 0.5);
                    ui.window("nested editor host").build(|| {
                        // A query with no drawing side effects exercises host validation.
                        assert!(catch_unwind(AssertUnwindSafe(|| frame.is_suspended())).is_err());
                        drop(local);
                    });
                    assert_eq!(frame.style().link_strength, original);
                });
                assert!(!frame.is_suspended());
                frame.node(NodeId::new(1), |_| unsafe {
                    sys::dne_set_current_editor_raw(raw_b)
                });
                assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);

                unsafe { sys::dne_set_current_editor_raw(raw_b) };
            });
            assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);
        });
        drop(imgui.render_legacy());

        unsafe { sys::dne_set_current_editor_raw(ptr::null_mut()) };
    }

    #[test]
    fn config_accepts_typed_buttons_and_custom_zoom_levels() {
        let mut config = EditorConfig::new()
            .drag_button(MouseButton::Left)
            .select_button(MouseButton::Right)
            .navigate_button(MouseButton::Middle)
            .context_menu_button(MouseButton::Extra1)
            .smooth_zoom(true, 1.25)
            .custom_zoom_levels(vec![0.5, 1.0, 2.0]);

        let snapshot = config.snapshot();
        assert_eq!(snapshot.custom_zoom_levels, vec![0.5, 1.0, 2.0]);
        assert_eq!(snapshot.drag_button, MouseButton::Left);
        assert_eq!(snapshot.select_button, MouseButton::Right);
        assert_eq!(snapshot.navigate_button, MouseButton::Middle);
        assert_eq!(snapshot.context_menu_button, MouseButton::Extra1);
        assert!(snapshot.enable_smooth_zoom);
        assert_eq!(snapshot.smooth_zoom_power, 1.25);

        let raw = config.as_sys();
        assert_eq!(raw.drag_button_index, MouseButton::Left as i32);
        assert_eq!(raw.select_button_index, MouseButton::Right as i32);
        assert_eq!(raw.navigate_button_index, MouseButton::Middle as i32);
        assert_eq!(raw.context_menu_button_index, MouseButton::Extra1 as i32);
        assert_eq!(raw.custom_zoom_level_count, 3);
        assert!(!raw.custom_zoom_levels.is_null());
    }

    #[test]
    fn editor_exposes_creation_config_snapshot() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let editor = EditorContext::create_with_config(
            &imgui,
            EditorConfig::new()
                .no_settings_file()
                .canvas_size_mode(crate::CanvasSizeMode::CenterOnly)
                .custom_zoom_levels(vec![0.75, 1.0, 1.5])
                .smooth_zoom(true, 1.4),
        );

        let snapshot = editor.config();
        assert_eq!(snapshot.settings_file, None);
        assert_eq!(snapshot.canvas_size_mode, crate::CanvasSizeMode::CenterOnly);
        assert_eq!(snapshot.custom_zoom_levels, vec![0.75, 1.0, 1.5]);
        assert!(snapshot.enable_smooth_zoom);
        assert_eq!(snapshot.smooth_zoom_power, 1.4);
    }

    #[test]
    fn style_snapshot_roundtrips_color() {
        let _guard = test_guard();
        let imgui = ImGuiContext::create();
        let _editor = EditorContext::create(&imgui);

        let original = _editor.style_color(StyleColor::Background);
        let updated = [0.11, 0.22, 0.33, 0.44];
        _editor.set_style_color(StyleColor::Background, updated);
        assert_eq!(_editor.style_color(StyleColor::Background), updated);

        let mut style = _editor.style();
        style.set_color(StyleColor::Background, original);
        _editor.set_style(&style);
        assert_eq!(_editor.style_color(StyleColor::Background), original);
    }
}
