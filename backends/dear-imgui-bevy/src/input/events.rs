//! Bevy message readers shared by primary and routed input translation.

use bevy_ecs::{message::MessageReader, system::SystemParam};
use bevy_input::{
    keyboard::{KeyboardFocusLost, KeyboardInput},
    mouse::{MouseButtonInput, MouseWheel},
    touch::TouchInput,
};
use bevy_window::{
    CursorEntered, CursorLeft, CursorMoved, Ime, WindowBackendScaleFactorChanged, WindowFocused,
    WindowResized, WindowScaleFactorChanged,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::RawWinitWindowEvent;

#[derive(SystemParam)]
pub(crate) struct ImguiInputMessageReaders<'w, 's> {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(super) raw_winit_window: MessageReader<'w, 's, RawWinitWindowEvent>,
    pub(super) window_resized: MessageReader<'w, 's, WindowResized>,
    pub(super) window_scale_factor_changed: MessageReader<'w, 's, WindowScaleFactorChanged>,
    pub(super) window_backend_scale_factor_changed:
        MessageReader<'w, 's, WindowBackendScaleFactorChanged>,
    pub(super) window_focused: MessageReader<'w, 's, WindowFocused>,
    pub(super) cursor_entered: MessageReader<'w, 's, CursorEntered>,
    pub(super) cursor_moved: MessageReader<'w, 's, CursorMoved>,
    pub(super) cursor_left: MessageReader<'w, 's, CursorLeft>,
    pub(super) mouse_button_input: MessageReader<'w, 's, MouseButtonInput>,
    pub(super) mouse_wheel: MessageReader<'w, 's, MouseWheel>,
    pub(super) keyboard_input: MessageReader<'w, 's, KeyboardInput>,
    pub(super) keyboard_focus_lost: MessageReader<'w, 's, KeyboardFocusLost>,
    pub(super) touch_input: MessageReader<'w, 's, TouchInput>,
    pub(super) ime: MessageReader<'w, 's, Ime>,
}

pub(super) fn discard_all_unread_messages(messages: &mut ImguiInputMessageReaders) {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    messages.raw_winit_window.clear();
    messages.window_resized.clear();
    messages.window_scale_factor_changed.clear();
    messages.window_backend_scale_factor_changed.clear();
    messages.window_focused.clear();
    messages.cursor_entered.clear();
    messages.cursor_moved.clear();
    messages.cursor_left.clear();
    messages.mouse_button_input.clear();
    messages.mouse_wheel.clear();
    messages.keyboard_input.clear();
    messages.keyboard_focus_lost.clear();
    messages.touch_input.clear();
    messages.ime.clear();
}
