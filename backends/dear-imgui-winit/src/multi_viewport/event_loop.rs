use super::*;

/// Store an `ActiveEventLoop` reference for viewport creation.
///
/// winit's `ActiveEventLoop` is only valid for the duration of the callback it is
/// passed into. Do **not** store it long-term. Call this right before invoking
/// `Context::update_platform_windows()` / `Context::render_platform_windows_default()`
/// (or any other ImGui call that may create viewports), and preferably clear it
/// afterwards via `set_event_loop_for_frame` or `clear_event_loop`.
pub fn set_event_loop(event_loop: &ActiveEventLoop) {
    EVENT_LOOP.with(|el| {
        *el.borrow_mut() = Some(event_loop as *const ActiveEventLoop);
    });
}

/// Clear any previously stored event loop pointer.
pub fn clear_event_loop() {
    EVENT_LOOP.with(|el| {
        *el.borrow_mut() = None;
    });
}

/// A guard that keeps the event loop pointer valid for the current callback.
///
/// Dropping the guard clears the stored pointer.
pub struct EventLoopFrameGuard;

impl Drop for EventLoopFrameGuard {
    fn drop(&mut self) {
        clear_event_loop();
    }
}

/// Set the event loop pointer for the duration of the current callback.
///
/// Recommended usage:
///
/// ```rust,no_run
/// # use dear_imgui_winit::multi_viewport;
/// # use winit::event_loop::ActiveEventLoop;
/// # fn on_redraw(event_loop: &ActiveEventLoop) {
/// let _guard = multi_viewport::set_event_loop_for_frame(event_loop);
/// // imgui.update_platform_windows();
/// // imgui.render_platform_windows_default();
/// # }
/// ```
pub fn set_event_loop_for_frame(event_loop: &ActiveEventLoop) -> EventLoopFrameGuard {
    set_event_loop(event_loop);
    EventLoopFrameGuard
}
