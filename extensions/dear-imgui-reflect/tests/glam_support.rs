#[cfg(feature = "glam")]
mod glam_tests {
    use dear_imgui_reflect as reflect;
    use dear_imgui_reflect::imgui::Context;
    use reflect::ImGuiReflect;

    use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

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
        let mut ctx = Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([800.0, 600.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let mut settings = GlamSettings::default();

        let _changed = reflect::input(&ui, "GlamSettings", &mut settings);
    }
}
