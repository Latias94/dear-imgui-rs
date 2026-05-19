use dear_imgui_rs::{StyleColor, Ui};

mod edit_modal;
mod edit_request;
mod io_modal;
mod pane;

pub(super) use edit_modal::draw_places_edit_modal;
pub(super) use io_modal::draw_places_io_modal;
pub(super) use pane::{draw_minimal_places_popup, draw_places_pane};

fn subtle_separator(ui: &Ui) {
    let style = ui.clone_style();
    let mut col = style.color(StyleColor::Separator);
    col[3] *= 0.35;
    let _col = ui.push_style_color(StyleColor::Separator, col);
    ui.separator();
}
