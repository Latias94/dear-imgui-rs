use crate::{EditorConfig, EditorConfigSnapshot, NodeEditorStyle, StyleColor, sys};
use dear_imgui_rs::{Context as ImGuiContext, ContextAliveToken};
use std::{ffi::c_void, marker::PhantomData, ptr, rc::Rc};

/// Errors returned by the node-editor safe layer.
#[derive(Debug, thiserror::Error)]
pub enum NodeEditorError {
    #[error("imgui-node-editor CreateEditor returned null")]
    CreateEditorFailed,
}

/// Owned imgui-node-editor context.
pub struct EditorContext {
    raw: *mut sys::DneEditorContext,
    imgui_ctx_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    imgui_alive: ContextAliveToken,
    config: EditorConfigSnapshot,
    _settings_file: Option<std::ffi::CString>,
    _callbacks: Option<Box<crate::config::CallbackState>>,
    _not_send_sync: PhantomData<Rc<()>>,
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
        let imgui_ctx_raw = imgui.as_raw();
        let _imgui_guard = ImGuiContextGuard::bind(imgui_ctx_raw);
        let config_snapshot = config.snapshot();
        let raw_config = config.to_sys();
        let raw = unsafe { sys::dne_create_editor(&raw_config) };
        if raw.is_null() {
            return Err(NodeEditorError::CreateEditorFailed);
        }

        Ok(Self {
            raw,
            imgui_ctx_raw,
            imgui_alive: imgui.alive_token(),
            config: config_snapshot,
            _settings_file: config.settings_file.take(),
            _callbacks: config.callbacks.take(),
            _not_send_sync: PhantomData,
        })
    }

    pub fn as_raw(&self) -> *mut sys::DneEditorContext {
        self.raw
    }

    pub fn as_raw_native(&self) -> *mut c_void {
        unsafe { sys::dne_editor_context_raw(self.raw) }
    }

    #[doc(alias = "GetConfig")]
    pub fn config(&self) -> &EditorConfigSnapshot {
        &self.config
    }

    #[doc(alias = "GetStyle")]
    pub fn style(&self) -> NodeEditorStyle {
        let _current = self.bind_current("EditorContext::style");
        NodeEditorStyle::current()
    }

    pub fn set_style(&self, style: &NodeEditorStyle) {
        let _current = self.bind_current("EditorContext::set_style");
        style.apply();
    }

    pub fn style_color(&self, color: StyleColor) -> [f32; 4] {
        let _current = self.bind_current("EditorContext::style_color");
        crate::style::current_style_color(color)
    }

    pub fn set_style_color(&self, color: StyleColor, value: [f32; 4]) {
        let _current = self.bind_current("EditorContext::set_style_color");
        crate::style::apply_style_color(color, value);
    }

    pub(crate) fn assert_usable(&self, caller: &str) {
        assert!(
            self.imgui_alive.is_alive(),
            "{caller} requires the owning Dear ImGui context to be alive"
        );
        assert!(
            !self.raw.is_null(),
            "{caller} requires a valid node-editor context"
        );
    }

    pub(crate) fn bind_current(&self, caller: &str) -> CurrentEditorGuard<'_> {
        self.assert_usable(caller);
        let imgui_guard = ImGuiContextGuard::bind(self.imgui_ctx_raw);
        let previous = unsafe { sys::dne_get_current_editor_raw() };
        unsafe { sys::dne_set_current_editor(self.raw) };
        CurrentEditorGuard {
            _editor: self,
            _imgui_guard: imgui_guard,
            previous,
        }
    }
}

impl Drop for EditorContext {
    fn drop(&mut self) {
        if self.raw.is_null() {
            return;
        }

        if !self.imgui_alive.is_alive() {
            debug_assert!(
                false,
                "EditorContext was dropped after its owning Dear ImGui context; \
                 declare the editor field before the Context field or drop it explicitly first"
            );
            self.raw = ptr::null_mut();
            return;
        }

        let _imgui_guard = ImGuiContextGuard::bind(self.imgui_ctx_raw);
        unsafe { sys::dne_destroy_editor(self.raw) };
        self.raw = ptr::null_mut();
    }
}

pub(crate) struct CurrentEditorGuard<'a> {
    _editor: &'a EditorContext,
    _imgui_guard: ImGuiContextGuard,
    previous: *mut c_void,
}

impl Drop for CurrentEditorGuard<'_> {
    fn drop(&mut self) {
        unsafe { sys::dne_set_current_editor_raw(self.previous) };
    }
}

struct ImGuiContextGuard {
    prev: *mut dear_imgui_rs::sys::ImGuiContext,
    restore: bool,
}

impl ImGuiContextGuard {
    fn bind(ctx: *mut dear_imgui_rs::sys::ImGuiContext) -> Self {
        let prev = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        let restore = prev != ctx;
        if restore {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(ctx) };
        }
        Self { prev, restore }
    }
}

impl Drop for ImGuiContextGuard {
    fn drop(&mut self) {
        if self.restore {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(self.prev) };
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
        ptr,
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
            let _current = editor_b.bind_current("test");
            assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_b);
        }
        assert_eq!(unsafe { sys::dne_get_current_editor_raw() }, raw_a);

        unsafe { sys::dne_set_current_editor_raw(ptr::null_mut()) };
    }

    #[test]
    fn creating_editor_does_not_break_imgui_frame() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        let _ = imgui.font_atlas_mut().build();

        let _editor = EditorContext::create(&imgui);

        imgui.frame();
        imgui.render();
    }

    #[test]
    fn frame_safe_api_calls_do_not_break_imgui_frame() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        let _ = imgui.font_atlas_mut().build();

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
        imgui.render();
    }

    #[test]
    fn frame_tokens_bind_own_editor_before_drop_and_restore_previous_editor() {
        let _guard = test_guard();
        let mut imgui = ImGuiContext::create();
        imgui.io_mut().set_display_size([640.0, 480.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        let _ = imgui.font_atlas_mut().build();

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
        imgui.render();

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

        let raw = config.to_sys();
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
