use bevy_app::App;
use bevy_ecs::prelude::Entity;
use dear_imgui_bevy::{ImguiContexts, ImguiViewportBridge};
use dear_imgui_rs::{self as imgui, sys};

pub(super) struct CallbackViewport {
    context_id: imgui::ContextId,
    viewport_id: imgui::Id,
    raw: *mut sys::ImGuiViewport,
    window: Entity,
}

impl CallbackViewport {
    pub(super) fn create(
        app: &mut App,
        context_id: imgui::ContextId,
        viewport_id: imgui::Id,
    ) -> Self {
        let raw = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(
            !raw.is_null(),
            "ImGuiViewport_ImGuiViewport() returned null"
        );

        app.world_mut()
            .non_send_mut::<ImguiContexts>()
            .configure(context_id, |context| unsafe {
                let viewport = imgui::Viewport::from_raw_mut(raw);
                (*raw).ID = viewport_id.raw();
                viewport.set_pos([0.0, 0.0]);
                viewport.set_size([640.0, 480.0]);
                viewport.set_dpi_scale(1.0);
                viewport.set_raw_flags_unchecked(imgui::ViewportFlags::IS_PLATFORM_WINDOW.bits());
                (*context.platform_io().as_raw())
                    .Platform_CreateWindow
                    .expect("native viewport bridge should install Platform_CreateWindow")(
                    raw
                );
            })
            .unwrap_or_else(|error| panic!("Context callback fixture setup failed: {error}"));

        app.update();
        let window = app
            .world()
            .non_send::<ImguiViewportBridge>()
            .viewport_window(context_id, viewport_id)
            .expect("the backend callback should create a viewport Window");

        Self {
            context_id,
            viewport_id,
            raw,
            window,
        }
    }

    pub(super) const fn window(&self) -> Entity {
        self.window
    }

    pub(super) fn destroy(self, app: &mut App) {
        app.world_mut()
            .non_send_mut::<ImguiContexts>()
            .configure(self.context_id, |context| unsafe {
                (*context.platform_io().as_raw())
                    .Platform_DestroyWindow
                    .expect("native viewport bridge should install Platform_DestroyWindow")(
                    self.raw,
                );
            })
            .unwrap_or_else(|error| panic!("Context callback fixture teardown failed: {error}"));
        unsafe { sys::ImGuiViewport_destroy(self.raw) };
        app.update();
        assert!(
            app.world()
                .non_send::<ImguiViewportBridge>()
                .viewport_window(self.context_id, self.viewport_id)
                .is_none(),
            "destroying the callback fixture should remove its viewport mapping"
        );
    }
}
