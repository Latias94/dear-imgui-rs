use crate::{EditorConfig, EditorConfigSnapshot, NodeEditorStyle, StyleColor, sys};
use dear_imgui_rs::{Context as ImGuiContext, ContextBinding, ContextId};
use std::{ffi::c_void, ptr};

/// Errors returned by the node-editor safe layer.
#[derive(Debug, thiserror::Error)]
pub enum NodeEditorError {
    #[error("imgui-node-editor CreateEditor returned null")]
    CreateEditorFailed,
}

/// Owned imgui-node-editor context.
pub struct EditorContext {
    raw: *mut sys::DneEditorContext,
    imgui_binding: ContextBinding,
    config: EditorConfigSnapshot,
    _settings_file: Option<std::ffi::CString>,
    _callbacks: Option<Box<crate::config::CallbackState>>,
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
        if self
            .imgui_binding
            .try_with_bound_context(|| unsafe { sys::dne_destroy_editor(raw) })
            .is_err()
        {
            // The owning ImGui context is no longer enterable. The node-editor shim owns
            // this handle and can release it without switching the dead ImGui context.
            unsafe { sys::dne_destroy_editor(raw) };
        }
    }
}

struct CurrentEditorGuard {
    previous: *mut c_void,
}

impl CurrentEditorGuard {
    fn bind(editor: *mut sys::DneEditorContext) -> Self {
        let previous = unsafe { sys::dne_get_current_editor_raw() };
        unsafe { sys::dne_set_current_editor(editor) };
        Self { previous }
    }
}

impl Drop for CurrentEditorGuard {
    fn drop(&mut self) {
        unsafe { sys::dne_set_current_editor_raw(self.previous) };
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
        let _ = imgui_b.font_atlas().build();
        let ui = imgui_b.frame();

        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = ui.node_editor(&editor, "wrong-context", [320.0, 240.0]);
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
        let _ = imgui.font_atlas().build();

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
        let _ = imgui.font_atlas().build();

        let editor_context = EditorContext::create(&imgui);
        let node_a = NodeId::new(1);
        let node_b = NodeId::new(2);
        let output_pin = PinId::new(11);
        let input_pin = PinId::new(21);
        let link = LinkId::new(100);

        let ui = imgui.frame();
        ui.window("node-editor-frame-api").build(|| {
            let editor = ui.node_editor(&editor_context, "frame-api", [320.0, 240.0]);

            assert!(!editor.is_suspended());
            {
                let suspension = editor.suspend();
                assert!(editor.is_suspended());
                suspension.resume();
            }
            assert!(!editor.is_suspended());

            editor.set_shortcuts_enabled(false);
            assert!(!editor.shortcuts_enabled());
            editor.set_shortcuts_enabled(true);

            editor.set_node_position(node_a, [20.0, 30.0]);
            editor.set_node_z_position(node_a, 2.0);
            let _ = editor.node_z_position(node_a);
            editor.restore_node_state(node_a);

            {
                let node = editor.begin_node(node_a);
                let pin = node.begin_pin(output_pin, PinKind::Output);
                ui.text("out");
                let cursor = ui.cursor_screen_pos();
                pin.rect(cursor, [cursor[0] + 8.0, cursor[1] + 8.0]);
                pin.pivot_rect(cursor, [cursor[0] + 8.0, cursor[1] + 8.0]);
                pin.pivot_size([8.0, 8.0]);
                pin.pivot_scale([1.0, 1.0]);
                pin.pivot_alignment([0.5, 0.5]);
                pin.end();
                node.end();
            }
            {
                let node = editor.begin_node(node_b);
                let pin = node.begin_pin(input_pin, PinKind::Input);
                ui.text("in");
                pin.end();
                node.end();
            }

            let _ = editor.begin_group_hint(node_a);
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

            editor.end();
        });
        drop(imgui.render_legacy());
    }

    #[test]
    fn frame_tokens_bind_own_editor_before_drop_and_restore_previous_editor() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        let _ = imgui.font_atlas().build();

        let editor_a = EditorContext::create(&imgui);
        let editor_b = EditorContext::create(&imgui);
        let raw_a = editor_a.as_raw_native();
        let raw_b = editor_b.as_raw_native();

        let ui = imgui.frame();
        ui.window("node-editor-token-context").build(|| {
            let frame = ui.node_editor(&editor_a, "token-context", [320.0, 240.0]);

            let style = frame.push_style_var_float(StyleVar::LinkStrength, 0.75);
            unsafe { sys::dne_set_current_editor_raw(raw_b) };
            drop(style);
            assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);

            let node = frame.begin_node(NodeId::new(1));
            unsafe { sys::dne_set_current_editor_raw(raw_b) };
            drop(node);
            assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);

            unsafe { sys::dne_set_current_editor_raw(raw_b) };
            drop(frame);
            assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);
        });
        drop(imgui.render_legacy());

        unsafe { sys::dne_set_current_editor_raw(ptr::null_mut()) };

        let _ = raw_a;
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
