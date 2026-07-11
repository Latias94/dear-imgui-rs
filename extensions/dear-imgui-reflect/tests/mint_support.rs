#![cfg(feature = "mint")]

mod common;

mod mint_tests {
    use dear_imgui_reflect as reflect;
    use dear_imgui_reflect::imgui::Context;
    use mint::{Vector2, Vector3, Vector4};
    use reflect::ImGuiReflect;

    use crate::common::test_guard;

    #[derive(ImGuiReflect)]
    struct MintSettings {
        v2: Vector2<f32>,
        v3: Vector3<f32>,
        v4: Vector4<f32>,
    }

    #[test]
    fn mint_vector_types_render_without_mutating_idle_values() {
        let _guard = test_guard();
        let session = reflect::ReflectSession::new();
        let mut ctx = Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([800.0, 600.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let mut inspector = session.inspector(ui);
        let mut settings = MintSettings {
            v2: Vector2 { x: 1.0, y: 2.0 },
            v3: Vector3 {
                x: 3.0,
                y: 4.0,
                z: 5.0,
            },
            v4: Vector4 {
                x: 6.0,
                y: 7.0,
                z: 8.0,
                w: 9.0,
            },
        };

        let changed = inspector.input("MintSettings", &mut settings);

        assert!(!changed);
        assert_eq!([settings.v2.x, settings.v2.y], [1.0, 2.0]);
        assert_eq!(
            [settings.v3.x, settings.v3.y, settings.v3.z],
            [3.0, 4.0, 5.0]
        );
        assert_eq!(
            [settings.v4.x, settings.v4.y, settings.v4.z, settings.v4.w],
            [6.0, 7.0, 8.0, 9.0]
        );
    }
}
