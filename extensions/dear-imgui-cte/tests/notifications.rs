use dear_imgui_cte::{CteError, CteUiExt, NotificationType, Notifications};
use dear_imgui_rs::{Condition, Context, FramePrepareOptions, sys};
use std::time::Duration;

fn render_context() -> Context {
    let mut context = Context::create();
    context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
    context
}

#[test]
fn empty_and_populated_notification_stacks_render() {
    let mut context = render_context();
    let mut notifications = Notifications::create(&context);

    let ui = context.frame();
    ui.notifications(&mut notifications).build().unwrap();
    let empty_vertices = context.render_legacy().total_vtx_count();

    for (kind, message) in [
        (NotificationType::Success, "saved"),
        (NotificationType::Warning, "check input"),
        (NotificationType::Error, "failed"),
        (NotificationType::Info, "working"),
    ] {
        notifications
            .add(kind, message, Duration::from_secs(2))
            .unwrap();
    }

    for frame in 0..3 {
        let ui = context.frame();
        let renderer = ui.notifications(&mut notifications);
        if frame == 0 {
            renderer.position([620.0, 460.0]).build().unwrap();
        } else {
            renderer.build().unwrap();
        }
        ui.window("Notification test host")
            .size([200.0, 100.0], Condition::Always)
            .build(|| ui.text(format!("frame {frame}")));
        let active_notifications = (1..=32)
            .filter(|id| {
                let name = std::ffi::CString::new(format!("Notification{id}"))
                    .expect("generated notification name cannot contain NUL");
                let window = unsafe { sys::igFindWindowByName(name.as_ptr()) };
                !window.is_null() && unsafe { (*window).Active }
            })
            .count();
        assert_eq!(active_notifications, 4);
        let draw_data = context.render_legacy();
        assert!(draw_data.total_vtx_count() > empty_vertices);
    }
}

#[test]
fn notifications_validate_messages_durations_and_positions() {
    let context = Context::create();
    let mut notifications = Notifications::create(&context);
    assert!(matches!(
        notifications.add(
            NotificationType::Info,
            "bad\0message",
            Duration::from_secs(1)
        ),
        Err(CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        notifications.add(
            NotificationType::Info,
            "too long",
            Duration::from_millis(i32::MAX as u64 + 1)
        ),
        Err(CteError::InvalidValue { .. })
    ));
    drop(notifications);
    drop(context);

    let mut context = render_context();
    let mut notifications = Notifications::create(&context);
    let ui = context.frame();
    assert!(matches!(
        ui.notifications(&mut notifications)
            .position([f32::NAN, 0.0])
            .build(),
        Err(CteError::NonFinite { .. })
    ));
    drop(context.render_legacy());
}
