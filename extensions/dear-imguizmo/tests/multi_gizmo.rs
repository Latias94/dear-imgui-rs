use dear_imgui_rs::{Condition, Context, MouseButton};
use dear_imguizmo::{DrawListTarget, GuizmoExt, Operation};
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

#[test]
fn a_new_context_starts_idle_while_another_context_has_view_interpolation() {
    let _guard = test_guard();
    let mut first = Context::create();
    prepare_imgui(&mut first);
    let mut view_a = IDENTITY;
    let mut view_b = IDENTITY;
    let position = [144.0, 144.0];

    queue_mouse(&mut first, position, None);
    let _ = draw_view_cubes(&mut first, &mut view_a, &mut view_b);
    let _ = draw_view_cubes(&mut first, &mut view_a, &mut view_b);
    queue_mouse(&mut first, position, Some((MouseButton::Left, true)));
    assert!(draw_view_cubes(&mut first, &mut view_a, &mut view_b));
    queue_mouse(&mut first, position, Some((MouseButton::Left, false)));
    assert!(draw_view_cubes(&mut first, &mut view_a, &mut view_b));
    drop(first);

    let mut second = Context::create();
    prepare_imgui(&mut second);
    queue_mouse(&mut second, [700.0, 500.0], None);
    let mut second_a = IDENTITY;
    let mut second_b = IDENTITY;
    assert!(!draw_view_cubes(&mut second, &mut second_a, &mut second_b));
    assert_eq!(second_a, IDENTITY);
    assert_eq!(second_b, IDENTITY);
}

fn draw_object_handle(ctx: &mut Context, operation: Operation) -> bool {
    let ui = ctx.frame();
    let gizmo = ui.guizmo();
    gizmo.set_orthographic(true);
    gizmo.set_rect(0.0, 0.0, 800.0, 600.0);
    let mut using = false;
    ui.window(GIZMO_WINDOW)
        .position([0.0, 0.0], Condition::Always)
        .size([800.0, 600.0], Condition::Always)
        .build(|| {
            gizmo.set_drawlist_window();
            let _id = gizmo.push_id("object");
            let mut model = IDENTITY;
            model[14] = 0.5;
            let handle = gizmo
                .manipulate_config(&IDENTITY, &IDENTITY, &mut model)
                .operation(operation);
            if operation == Operation::BOUNDS {
                handle.bounds([-0.5; 3], [0.5; 3]).build();
            } else {
                handle.build();
            }
            using = gizmo.is_using();
        });
    assert!(ctx.end_frame());
    using
}

fn assert_object_handle_idle(ctx: &mut Context) {
    {
        let gizmo = ctx.frame().guizmo();
        let _id = gizmo.push_id("object");
        assert!(!gizmo.is_over());
        assert!(!gizmo.is_using());
        assert!(!gizmo.is_using_any());
        assert!(gizmo.active_handle_type().is_none());
        assert!(gizmo.hovered_handle_type().is_none());
    }
    assert!(ctx.end_frame());
}

#[test]
fn handing_off_and_back_cancels_the_previous_hotspot_and_object_or_bounds_drag() {
    let _guard = test_guard();
    for (operation, position) in [
        (Operation::TRANSLATE, [400.0, 300.0]),
        (Operation::BOUNDS, [200.0, 450.0]),
    ] {
        let mut first = Context::create();
        prepare_imgui(&mut first);
        queue_mouse(&mut first, position, None);
        let _ = draw_object_handle(&mut first, operation);
        let _ = draw_object_handle(&mut first, operation);
        queue_mouse(&mut first, position, Some((MouseButton::Left, true)));
        assert!(draw_object_handle(&mut first, operation));
        let suspended = first.suspend_or_panic();

        let mut second = Context::create();
        prepare_imgui(&mut second);
        assert_object_handle_idle(&mut second);
        drop(second);
        assert_object_handle_idle(&mut suspended.activate_or_panic());
    }
}

#[test]
fn a_viewport_layer_only_accepts_hover_from_an_explicit_alternative_window() {
    let _guard = test_guard();
    for (layer, window) in [
        (DrawListTarget::Background, "##Background"),
        (DrawListTarget::Foreground, "##Foreground"),
    ] {
        let mut ctx = Context::create();
        prepare_imgui(&mut ctx);
        queue_mouse(&mut ctx, [144.0, 144.0], None);
        for frame in 0..3 {
            let ui = ctx.frame();
            let gizmo = ui.guizmo();
            ui.window(window)
                .position([0.0, 0.0], Condition::Always)
                .size([800.0, 600.0], Condition::Always)
                .build(|| {
                    gizmo.set_drawlist(layer);
                    gizmo.set_rect(0.0, 0.0, 800.0, 600.0);
                    if frame == 2 {
                        assert!(gizmo.set_alternative_window_by_name(window));
                    }
                    let mut view = IDENTITY;
                    gizmo.view_manipulate(&mut view, 5.0, VIEW_A_POSITION, VIEW_SIZE, 0x1010_1010);
                    if frame > 0 {
                        assert_eq!(gizmo.is_view_manipulate_hovered(), frame == 2);
                    }
                });
            assert!(ctx.end_frame());
        }
    }
}

#[test]
fn a_new_owner_thread_starts_idle_and_keeps_the_configured_style() {
    let _guard = test_guard();
    let mut first = Context::create();
    prepare_imgui(&mut first);
    let original = {
        let gizmo = first.frame().guizmo();
        let mut style = gizmo.style();
        let original = style.rotation_line_thickness();
        style.set_rotation_line_thickness(original + 0.5);
        original
    };
    assert!(first.end_frame());
    let position = [400.0, 300.0];
    queue_mouse(&mut first, position, None);
    let _ = draw_object_handle(&mut first, Operation::TRANSLATE);
    let _ = draw_object_handle(&mut first, Operation::TRANSLATE);
    queue_mouse(&mut first, position, Some((MouseButton::Left, true)));
    assert!(draw_object_handle(&mut first, Operation::TRANSLATE));
    drop(first);

    std::thread::spawn(move || {
        let mut next = Context::create();
        prepare_imgui(&mut next);
        let gizmo = next.frame().guizmo();
        assert!(!gizmo.is_using_any());
        assert!(!gizmo.is_over());
        let mut style = gizmo.style();
        assert_eq!(style.rotation_line_thickness(), original + 0.5);
        style.set_rotation_line_thickness(original);
        assert!(next.end_frame());
    })
    .join()
    .unwrap();
}
