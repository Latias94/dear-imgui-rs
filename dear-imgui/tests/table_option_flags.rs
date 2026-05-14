use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn table_options_reject_non_independent_bits_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();
    let sizing_in_flags =
        imgui::TableFlags::from_bits_retain(imgui::sys::ImGuiTableFlags_SizingFixedFit);

    let _ = ui.window("Table option flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_table_with_flags("sizing in flags", 1, sizing_in_flags);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_table_with_sizing(
                    "negative inner width",
                    1,
                    imgui::TableFlags::SCROLL_X,
                    [100.0, 100.0],
                    -1.0,
                );
            }))
            .is_err()
        );

        let table = ui.begin_table_with_flags(
            "typed sizing",
            1,
            imgui::TableOptions::new().sizing_policy(imgui::TableSizingPolicy::FixedFit),
        );
        if let Some(table) = table {
            table.end();
        }
    });
}

#[test]
fn table_column_options_reject_non_independent_bits_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();
    let width_in_flags =
        imgui::TableColumnFlags::from_bits_retain(imgui::sys::ImGuiTableColumnFlags_WidthFixed);
    let indent_in_flags =
        imgui::TableColumnFlags::from_bits_retain(imgui::sys::ImGuiTableColumnFlags_IndentDisable);
    let status_in_flags =
        imgui::TableColumnFlags::from_bits_retain(imgui::sys::ImGuiTableColumnFlags_IsEnabled);
    let private_in_flags = imgui::TableColumnFlags::from_bits_retain(
        imgui::sys::ImGuiTableColumnFlags_NoDirectResize_,
    );

    let _ = ui.window("Table column option flags").build(|| {
        let Some(_table) = ui.begin_table_with_flags(
            "columns",
            4,
            imgui::TableOptions::new().sizing_policy(imgui::TableSizingPolicy::FixedFit),
        ) else {
            return;
        };

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column("width in flags", width_in_flags, None, 0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column_with_indent(
                    "indent in flags",
                    indent_in_flags,
                    None,
                    None,
                    0,
                );
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column("status in flags", status_in_flags, None, 0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column("private in flags", private_in_flags, None, 0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column_fixed_width(
                    "nan width",
                    imgui::TableColumnFlags::NONE,
                    f32::NAN,
                    0,
                );
            }))
            .is_err()
        );

        ui.table_setup_column_with_indent(
            "typed",
            imgui::TableColumnFlags::DISABLED
                | imgui::TableColumnFlags::DEFAULT_HIDE
                | imgui::TableColumnFlags::DEFAULT_SORT,
            Some(imgui::TableColumnWidth::Fixed(48.0)),
            Some(imgui::TableColumnIndent::Disable),
            0,
        );
    });
}
