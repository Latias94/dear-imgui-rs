use crate::dialog_core::DirEntry;
use crate::dialog_state::FileDialogState;
use crate::file_style::EntryKind;
use dear_imgui_rs::{ColorStackToken, StyleColor, Ui};

pub(super) struct TextColorToken<'ui> {
    _token: Option<ColorStackToken<'ui>>,
}

pub(super) struct StyleVisual {
    pub(super) text_color: Option<[f32; 4]>,
    pub(super) icon: Option<String>,
    pub(super) tooltip: Option<String>,
    pub(super) font_id: Option<dear_imgui_rs::FontId>,
}

pub(super) fn style_visual_for_entry(state: &mut FileDialogState, e: &DirEntry) -> StyleVisual {
    let kind = if e.is_symlink {
        EntryKind::Link
    } else if e.is_dir {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let style = state.ui.config.file_styles.style_for_owned(&e.name, kind);
    let font_id = style
        .as_ref()
        .and_then(|s| s.font_token.as_deref())
        .and_then(|token| state.ui.config.file_style_fonts.get(token).copied());

    StyleVisual {
        text_color: style.as_ref().and_then(|s| s.text_color),
        icon: style.as_ref().and_then(|s| s.icon.clone()),
        tooltip: style.as_ref().and_then(|s| s.tooltip.clone()),
        font_id,
    }
}

impl<'ui> TextColorToken<'ui> {
    pub(super) fn push(ui: &'ui Ui, color: [f32; 4]) -> Self {
        let token = ui.push_style_color(StyleColor::Text, color);
        Self {
            _token: Some(token),
        }
    }

    pub(super) fn none() -> Self {
        Self { _token: None }
    }
}
