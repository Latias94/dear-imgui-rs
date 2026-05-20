use super::types::*;
use crate::sys;

#[derive(Clone, Copy, Default)]
pub(in crate::platform_io::trampolines) struct CallbackSet {
    platform_create_window: Option<ViewportCb>,
    platform_destroy_window: Option<ViewportCb>,
    platform_show_window: Option<ViewportCb>,
    platform_set_window_pos: Option<ViewportVec2Cb>,
    platform_get_window_pos_raw: Option<RawViewportVec2OutCb>,
    platform_get_window_pos: Option<ViewportVec2OutCb>,
    platform_set_window_size: Option<ViewportVec2Cb>,
    platform_get_window_size_raw: Option<RawViewportVec2OutCb>,
    platform_get_window_size: Option<ViewportVec2OutCb>,
    platform_get_window_framebuffer_scale_raw: Option<RawViewportVec2OutCb>,
    platform_get_window_framebuffer_scale: Option<ViewportVec2OutCb>,
    platform_set_window_focus: Option<ViewportCb>,
    platform_get_window_dpi_scale: Option<ViewportF32RetCb>,
    platform_get_window_focus: Option<ViewportBoolRetCb>,
    platform_get_window_minimized: Option<ViewportBoolRetCb>,
    platform_on_changed_viewport: Option<ViewportCb>,
    platform_get_window_work_area_insets_raw: Option<RawViewportVec4OutCb>,
    platform_get_window_work_area_insets: Option<ViewportVec4OutCb>,
    platform_set_window_title: Option<ViewportTitleCb>,
    platform_set_window_alpha: Option<ViewportF32Cb>,
    platform_update_window: Option<ViewportCb>,
    platform_render_window: Option<ViewportRenderCb>,
    platform_swap_buffers: Option<ViewportRenderCb>,
    renderer_create_window: Option<ViewportCb>,
    renderer_destroy_window: Option<ViewportCb>,
    renderer_set_window_size: Option<ViewportVec2Cb>,
    renderer_render_window: Option<ViewportRenderCb>,
    renderer_swap_buffers: Option<ViewportRenderCb>,
}

impl CallbackSet {
    pub(in crate::platform_io::trampolines) fn clear_platform(&mut self) {
        self.platform_create_window = None;
        self.platform_destroy_window = None;
        self.platform_show_window = None;
        self.platform_set_window_pos = None;
        self.platform_get_window_pos_raw = None;
        self.platform_get_window_pos = None;
        self.platform_set_window_size = None;
        self.platform_get_window_size_raw = None;
        self.platform_get_window_size = None;
        self.platform_get_window_framebuffer_scale_raw = None;
        self.platform_get_window_framebuffer_scale = None;
        self.platform_set_window_focus = None;
        self.platform_get_window_dpi_scale = None;
        self.platform_get_window_focus = None;
        self.platform_get_window_minimized = None;
        self.platform_on_changed_viewport = None;
        self.platform_get_window_work_area_insets_raw = None;
        self.platform_get_window_work_area_insets = None;
        self.platform_set_window_title = None;
        self.platform_set_window_alpha = None;
        self.platform_update_window = None;
        self.platform_render_window = None;
        self.platform_swap_buffers = None;
    }

    pub(in crate::platform_io::trampolines) fn clear_renderer(&mut self) {
        self.renderer_create_window = None;
        self.renderer_destroy_window = None;
        self.renderer_set_window_size = None;
        self.renderer_render_window = None;
        self.renderer_swap_buffers = None;
    }

    pub(in crate::platform_io::trampolines) fn is_empty(&self) -> bool {
        self.platform_create_window.is_none()
            && self.platform_destroy_window.is_none()
            && self.platform_show_window.is_none()
            && self.platform_set_window_pos.is_none()
            && self.platform_get_window_pos_raw.is_none()
            && self.platform_get_window_pos.is_none()
            && self.platform_set_window_size.is_none()
            && self.platform_get_window_size_raw.is_none()
            && self.platform_get_window_size.is_none()
            && self.platform_get_window_framebuffer_scale_raw.is_none()
            && self.platform_get_window_framebuffer_scale.is_none()
            && self.platform_set_window_focus.is_none()
            && self.platform_get_window_dpi_scale.is_none()
            && self.platform_get_window_focus.is_none()
            && self.platform_get_window_minimized.is_none()
            && self.platform_on_changed_viewport.is_none()
            && self.platform_get_window_work_area_insets_raw.is_none()
            && self.platform_get_window_work_area_insets.is_none()
            && self.platform_set_window_title.is_none()
            && self.platform_set_window_alpha.is_none()
            && self.platform_update_window.is_none()
            && self.platform_render_window.is_none()
            && self.platform_swap_buffers.is_none()
            && self.renderer_create_window.is_none()
            && self.renderer_destroy_window.is_none()
            && self.renderer_set_window_size.is_none()
            && self.renderer_render_window.is_none()
            && self.renderer_swap_buffers.is_none()
    }
}

pub(in crate::platform_io::trampolines) struct ContextCallbacks {
    pub(in crate::platform_io::trampolines) ctx: *mut sys::ImGuiContext,
    pub(in crate::platform_io::trampolines) callbacks: CallbackSet,
}

pub(in crate::platform_io) struct CallbackSlot<T: Copy> {
    pub(in crate::platform_io::trampolines) get: fn(&CallbackSet) -> Option<T>,
    pub(in crate::platform_io::trampolines) set: fn(&mut CallbackSet, Option<T>),
}

macro_rules! callback_slot {
    ($name:ident, $field:ident, $ty:ty, $getter:ident, $setter:ident) => {
        fn $getter(callbacks: &CallbackSet) -> Option<$ty> {
            callbacks.$field
        }

        fn $setter(callbacks: &mut CallbackSet, callback: Option<$ty>) {
            callbacks.$field = callback;
        }

        pub(in crate::platform_io) const $name: CallbackSlot<$ty> = CallbackSlot {
            get: $getter,
            set: $setter,
        };
    };
}

callback_slot!(
    PLATFORM_CREATE_WINDOW_CB,
    platform_create_window,
    ViewportCb,
    get_platform_create_window,
    set_platform_create_window
);
callback_slot!(
    PLATFORM_DESTROY_WINDOW_CB,
    platform_destroy_window,
    ViewportCb,
    get_platform_destroy_window,
    set_platform_destroy_window
);
callback_slot!(
    PLATFORM_SHOW_WINDOW_CB,
    platform_show_window,
    ViewportCb,
    get_platform_show_window,
    set_platform_show_window
);
callback_slot!(
    PLATFORM_SET_WINDOW_POS_CB,
    platform_set_window_pos,
    ViewportVec2Cb,
    get_platform_set_window_pos,
    set_platform_set_window_pos
);
callback_slot!(
    PLATFORM_GET_WINDOW_POS_RAW_CB,
    platform_get_window_pos_raw,
    RawViewportVec2OutCb,
    get_platform_get_window_pos_raw,
    set_platform_get_window_pos_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_POS_CB,
    platform_get_window_pos,
    ViewportVec2OutCb,
    get_platform_get_window_pos,
    set_platform_get_window_pos
);
callback_slot!(
    PLATFORM_SET_WINDOW_SIZE_CB,
    platform_set_window_size,
    ViewportVec2Cb,
    get_platform_set_window_size,
    set_platform_set_window_size
);
callback_slot!(
    PLATFORM_GET_WINDOW_SIZE_RAW_CB,
    platform_get_window_size_raw,
    RawViewportVec2OutCb,
    get_platform_get_window_size_raw,
    set_platform_get_window_size_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_SIZE_CB,
    platform_get_window_size,
    ViewportVec2OutCb,
    get_platform_get_window_size,
    set_platform_get_window_size
);
callback_slot!(
    PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_RAW_CB,
    platform_get_window_framebuffer_scale_raw,
    RawViewportVec2OutCb,
    get_platform_get_window_framebuffer_scale_raw,
    set_platform_get_window_framebuffer_scale_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_FRAMEBUFFER_SCALE_CB,
    platform_get_window_framebuffer_scale,
    ViewportVec2OutCb,
    get_platform_get_window_framebuffer_scale,
    set_platform_get_window_framebuffer_scale
);
callback_slot!(
    PLATFORM_SET_WINDOW_FOCUS_CB,
    platform_set_window_focus,
    ViewportCb,
    get_platform_set_window_focus,
    set_platform_set_window_focus
);
callback_slot!(
    PLATFORM_GET_WINDOW_DPI_SCALE_CB,
    platform_get_window_dpi_scale,
    ViewportF32RetCb,
    get_platform_get_window_dpi_scale,
    set_platform_get_window_dpi_scale
);
callback_slot!(
    PLATFORM_GET_WINDOW_FOCUS_CB,
    platform_get_window_focus,
    ViewportBoolRetCb,
    get_platform_get_window_focus,
    set_platform_get_window_focus
);
callback_slot!(
    PLATFORM_GET_WINDOW_MINIMIZED_CB,
    platform_get_window_minimized,
    ViewportBoolRetCb,
    get_platform_get_window_minimized,
    set_platform_get_window_minimized
);
callback_slot!(
    PLATFORM_ON_CHANGED_VIEWPORT_CB,
    platform_on_changed_viewport,
    ViewportCb,
    get_platform_on_changed_viewport,
    set_platform_on_changed_viewport
);
callback_slot!(
    PLATFORM_GET_WINDOW_WORK_AREA_INSETS_RAW_CB,
    platform_get_window_work_area_insets_raw,
    RawViewportVec4OutCb,
    get_platform_get_window_work_area_insets_raw,
    set_platform_get_window_work_area_insets_raw
);
callback_slot!(
    PLATFORM_GET_WINDOW_WORK_AREA_INSETS_CB,
    platform_get_window_work_area_insets,
    ViewportVec4OutCb,
    get_platform_get_window_work_area_insets,
    set_platform_get_window_work_area_insets
);
callback_slot!(
    PLATFORM_SET_WINDOW_TITLE_CB,
    platform_set_window_title,
    ViewportTitleCb,
    get_platform_set_window_title,
    set_platform_set_window_title
);
callback_slot!(
    PLATFORM_SET_WINDOW_ALPHA_CB,
    platform_set_window_alpha,
    ViewportF32Cb,
    get_platform_set_window_alpha,
    set_platform_set_window_alpha
);
callback_slot!(
    PLATFORM_UPDATE_WINDOW_CB,
    platform_update_window,
    ViewportCb,
    get_platform_update_window,
    set_platform_update_window
);
callback_slot!(
    PLATFORM_RENDER_WINDOW_CB,
    platform_render_window,
    ViewportRenderCb,
    get_platform_render_window,
    set_platform_render_window
);
callback_slot!(
    PLATFORM_SWAP_BUFFERS_CB,
    platform_swap_buffers,
    ViewportRenderCb,
    get_platform_swap_buffers,
    set_platform_swap_buffers
);
callback_slot!(
    RENDERER_CREATE_WINDOW_CB,
    renderer_create_window,
    ViewportCb,
    get_renderer_create_window,
    set_renderer_create_window
);
callback_slot!(
    RENDERER_DESTROY_WINDOW_CB,
    renderer_destroy_window,
    ViewportCb,
    get_renderer_destroy_window,
    set_renderer_destroy_window
);
callback_slot!(
    RENDERER_SET_WINDOW_SIZE_CB,
    renderer_set_window_size,
    ViewportVec2Cb,
    get_renderer_set_window_size,
    set_renderer_set_window_size
);
callback_slot!(
    RENDERER_RENDER_WINDOW_CB,
    renderer_render_window,
    ViewportRenderCb,
    get_renderer_render_window,
    set_renderer_render_window
);
callback_slot!(
    RENDERER_SWAP_BUFFERS_CB,
    renderer_swap_buffers,
    ViewportRenderCb,
    get_renderer_swap_buffers,
    set_renderer_swap_buffers
);
