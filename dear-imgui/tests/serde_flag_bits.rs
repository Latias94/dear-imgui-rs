#![cfg(feature = "serde")]

use dear_imgui_rs as imgui;
use serde::Deserialize;
use serde::de::value::{Error, I32Deserializer};
use serde_test::{Token, assert_tokens};

fn first_unknown_bit(known_bits: i32) -> i32 {
    (0..31)
        .map(|shift| 1_i32 << shift)
        .find(|candidate| known_bits & candidate == 0)
        .expect("test requires at least one spare positive flag bit")
}

macro_rules! assert_strict_numeric_flags {
    ($($flag:ty),+ $(,)?) => {$(
        let flags = <$flag>::all();
        let bits = flags.bits();
        assert_tokens(&flags, &[Token::I32(bits)]);
        let unknown = bits | first_unknown_bit(bits);
        assert!(
            <$flag>::deserialize(I32Deserializer::<Error>::new(unknown)).is_err(),
            "{} accepted unknown bits",
            stringify!($flag),
        );
    )+};
}

#[test]
fn serde_keeps_numeric_format_and_rejects_unknown_bits() {
    assert_strict_numeric_flags!(
        imgui::ConfigFlags,
        imgui::BackendFlags,
        imgui::ViewportFlags,
        imgui::WindowClassViewportFlags,
        imgui::WindowFlags,
        imgui::TableFlags,
        imgui::TableColumnFlags,
        imgui::TableColumnStateFlags,
        imgui::TableRowFlags,
        imgui::InputTextFlags,
        imgui::InputTextMultilineFlags,
        imgui::InputScalarFlags,
    );
}

#[test]
fn native_table_column_flags_round_trip_without_internal_resize_flags() {
    let mut context = imgui::Context::create();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    context.prepare_frame(imgui::FramePrepareOptions::new([800.0, 600.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();

    let ui = context.frame();
    ui.window("Table serialization")
        .size([320.0, 240.0], imgui::Condition::Always)
        .build(|| {
            let _table = ui
                .begin_table_with_flags(
                    "columns",
                    2,
                    imgui::TableOptions::new()
                        .flags(imgui::TableFlags::RESIZABLE)
                        .sizing_policy(imgui::TableSizingPolicy::StretchSame),
                )
                .expect("visible table");
            ui.table_setup_column("left", imgui::TableColumnFlags::empty(), None);
            ui.table_setup_column("right", imgui::TableColumnFlags::empty(), None);
            ui.table_next_row();

            let raw = unsafe { imgui::sys::igTableGetColumnFlags(1) };
            assert_ne!(raw & imgui::sys::ImGuiTableColumnFlags_NoDirectResize_, 0);
            let right = imgui::TableColumnIndex::from(1usize);
            let public = ui.table_get_column_flags(right);
            assert_tokens(
                &public,
                &[Token::I32(raw & imgui::TableColumnStateFlags::all().bits())],
            );
        })
        .expect("visible window");
}
