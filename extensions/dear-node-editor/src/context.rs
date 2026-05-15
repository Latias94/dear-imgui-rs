use crate::{EditorConfig, sys};
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
        let raw_config = config.to_sys();
        let raw = unsafe { sys::dne_create_editor(&raw_config) };
        if raw.is_null() {
            return Err(NodeEditorError::CreateEditorFailed);
        }

        Ok(Self {
            raw,
            imgui_ctx_raw,
            imgui_alive: imgui.alive_token(),
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

    pub(crate) fn assert_usable(&self, caller: &str) {
        assert!(
            self.imgui_alive.is_alive(),
            "{caller} requires the owning Dear ImGui context to be alive"
        );
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            self.imgui_ctx_raw,
            "{caller} must be used while the owning Dear ImGui context is current"
        );
        assert!(
            !self.raw.is_null(),
            "{caller} requires a valid node-editor context"
        );
    }

    pub(crate) fn bind_current(&self, caller: &str) -> CurrentEditorGuard<'_> {
        self.assert_usable(caller);
        let previous = unsafe { sys::dne_get_current_editor_raw() };
        unsafe { sys::dne_set_current_editor(self.raw) };
        CurrentEditorGuard {
            _editor: self,
            previous,
        }
    }
}

impl Drop for EditorContext {
    fn drop(&mut self) {
        if self.raw.is_null() {
            return;
        }

        if self.imgui_alive.is_alive() {
            let _imgui_guard = ImGuiContextGuard::bind(self.imgui_ctx_raw);
            unsafe { sys::dne_destroy_editor(self.raw) };
        }
        self.raw = ptr::null_mut();
    }
}

pub(crate) struct CurrentEditorGuard<'a> {
    _editor: &'a EditorContext,
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
    use crate::{LinkId, NodeEditorUiExt, NodeId, PinId, PinKind};
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
}
