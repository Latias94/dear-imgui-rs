use std::ffi::CStr;

use dear_imgui_rs::{Condition, Context, FramePrepareOptions, sys};
use dear_implot::{AxisFlags, PlotContext};

#[test]
fn axis_popup_headings_preserve_literal_percent_signs() {
    for (popup, expected) in [("##XContext", "X %% load"), ("##YContext", "Y %% load")] {
        let mut context = Context::create();
        context
            .set_ini_filename::<std::path::PathBuf>(None)
            .unwrap();
        context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let plots = PlotContext::create(&context);
        let ui = context.frame();
        let output = ui
            .window("Axis menu test")
            .size([600.0, 440.0], Condition::Always)
            .build(|| {
                let plot_id = ui.get_id("Timing").raw();
                plots
                    .get_plot_ui(ui)
                    .with_plot_setup(
                        "Timing",
                        [560.0, 380.0],
                        |setup| {
                            setup.setup_axes(
                                Some("X %% load"),
                                Some("Y %% load"),
                                AxisFlags::NONE,
                                AxisFlags::NONE,
                            )
                        },
                        |_| {
                            // Open the primary axis menu on the same ID stack used by EndPlot.
                            unsafe {
                                sys::igPushOverrideID(plot_id);
                                sys::igPushID_Int(0);
                            }
                            ui.open_popup(popup);
                            unsafe {
                                sys::igPopID();
                                sys::igPopID();
                                sys::igLogToBuffer(0);
                            }
                        },
                    )
                    .expect("visible plot");
                // Copy the text emitted by EndPlot before Dear ImGui clears its log buffer.
                unsafe {
                    let buffer = &raw mut (*sys::igGetCurrentContext()).LogBuffer;
                    let text = CStr::from_ptr(sys::ImGuiTextBuffer_begin(buffer))
                        .to_string_lossy()
                        .into_owned();
                    sys::igLogFinish();
                    text
                }
            })
            .expect("visible host window");
        assert!(output.contains(expected), "axis menu text: {output:?}");
        assert!(context.end_frame());
    }
}
