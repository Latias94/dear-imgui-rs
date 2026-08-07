#![cfg(feature = "glam")]

mod common;

mod glam_tests {
    use dear_imgui_reflect as reflect;
    use dear_imgui_reflect::imgui::Context;
    use reflect::ImGuiReflect;

    use crate::common::test_guard;
    use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

    #[derive(ImGuiReflect, Default)]
    struct GlamSettings {
        v2: Vec2,
        v3: Vec3,
        v4: Vec4,
        q: Quat,
        m: Mat4,
    }

    #[test]
    fn glam_vec_types_can_be_reflected() {
        let _guard = test_guard();
        let session = reflect::ReflectSession::new();
        let mut ctx = Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([800.0, 600.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let mut settings = GlamSettings::default();

        let _changed = inspector.input("GlamSettings", &mut settings);
    }
}
