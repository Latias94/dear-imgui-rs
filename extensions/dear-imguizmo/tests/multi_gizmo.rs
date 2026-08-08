use dear_imgui_rs::{Condition, Context, MouseButton};
use dear_imguizmo::GuizmoExt;
use std::sync::{Mutex, OnceLock};

const GIZMO_WINDOW: &str = "dear-imguizmo multi-view test";
const VIEW_A_POSITION: [f32; 2] = [80.0, 80.0];
const VIEW_B_POSITION: [f32; 2] = [280.0, 80.0];
const VIEW_SIZE: [f32; 2] = [128.0, 128.0];
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn prepare_imgui(imgui: &mut Context) {
    let io = imgui.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);
}

fn queue_mouse(imgui: &mut Context, position: [f32; 2], button: Option<(MouseButton, bool)>) {
    let io = imgui.io_mut();
    io.add_mouse_pos_event(position);
    if let Some((button, down)) = button {
        io.add_mouse_button_event(button, down);
    }
}

fn draw_view_cubes(imgui: &mut Context, view_a: &mut [f32; 16], view_b: &mut [f32; 16]) -> bool {
    let mut using_view_manipulate = false;
    {
        let ui = imgui.frame();
        let gizmo = ui.guizmo();
        ui.window(GIZMO_WINDOW)
            .position([0.0, 0.0], Condition::Always)
            .size([800.0, 600.0], Condition::Always)
            .build(|| {
                let [window_x, window_y] = ui.window_pos();
                let [window_width, window_height] = ui.window_size();
                gizmo.set_drawlist_window();
                gizmo.set_rect(window_x, window_y, window_width, window_height);
                gizmo.draw_grid(&IDENTITY, &IDENTITY, &IDENTITY, 10.0);

                {
                    let _id = gizmo.push_id("view-a");
                    using_view_manipulate |=
                        gizmo.view_manipulate(view_a, 5.0, VIEW_A_POSITION, VIEW_SIZE, 0x1010_1010);
                }
                {
                    let _id = gizmo.push_id("view-b");
                    using_view_manipulate |=
                        gizmo.view_manipulate(view_b, 5.0, VIEW_B_POSITION, VIEW_SIZE, 0x1010_1010);
                }
            });
    }
    assert!(imgui.end_frame());
    using_view_manipulate
}

#[test]
fn view_manipulators_keep_drag_state_isolated_by_gizmo_id() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let mut view_a = IDENTITY;
    let mut view_b = IDENTITY;

    queue_mouse(&mut imgui, [700.0, 500.0], None);
    assert!(!draw_view_cubes(&mut imgui, &mut view_a, &mut view_b));

    queue_mouse(&mut imgui, [144.0, 144.0], Some((MouseButton::Left, true)));
    assert!(draw_view_cubes(&mut imgui, &mut view_a, &mut view_b));

    queue_mouse(&mut imgui, [174.0, 164.0], None);
    assert!(draw_view_cubes(&mut imgui, &mut view_a, &mut view_b));
    assert_ne!(view_a, IDENTITY, "dragging view A must update only view A");
    assert_eq!(
        view_b, IDENTITY,
        "view B must not inherit view A's drag state"
    );

    queue_mouse(&mut imgui, [174.0, 164.0], Some((MouseButton::Left, false)));
    let _ = draw_view_cubes(&mut imgui, &mut view_a, &mut view_b);
}
