use super::TextEditor;
use crate::{
    CteResult, MiddleMouseMode, sys,
    validation::{validate_finite_f32, validate_nonzero_usize},
};

macro_rules! bool_property {
    ($setter:ident, $getter:ident, $raw_setter:path, $raw_getter:path) => {
        pub fn $setter(&mut self, value: bool) {
            self.with_context(concat!("TextEditor::", stringify!($setter)), |raw| unsafe {
                $raw_setter(raw, value)
            });
        }

        pub fn $getter(&self) -> bool {
            self.with_context(concat!("TextEditor::", stringify!($getter)), |raw| unsafe {
                $raw_getter(raw)
            })
        }
    };
}

impl TextEditor {
    pub fn set_tab_size(&mut self, value: usize) -> CteResult<()> {
        validate_nonzero_usize("TextEditor::set_tab_size", "value", value)?;
        self.with_context("TextEditor::set_tab_size", |raw| unsafe {
            sys::TextEditor_SetTabSize(raw, value)
        });
        Ok(())
    }

    pub fn tab_size(&self) -> usize {
        self.with_context("TextEditor::tab_size", |raw| unsafe {
            sys::TextEditor_GetTabSize(raw)
        })
    }

    pub fn set_line_spacing(&mut self, value: f32) -> CteResult<()> {
        validate_finite_f32("TextEditor::set_line_spacing", "value", value)?;
        self.with_context("TextEditor::set_line_spacing", |raw| unsafe {
            sys::TextEditor_SetLineSpacing(raw, value)
        });
        Ok(())
    }

    pub fn line_spacing(&self) -> f32 {
        self.with_context("TextEditor::line_spacing", |raw| unsafe {
            sys::TextEditor_GetLineSpacing(raw)
        })
    }

    /// Sets the fixed minimap width in columns, or `0` to size it automatically.
    pub fn set_minimap_columns(&mut self, value: usize) {
        self.with_context("TextEditor::set_minimap_columns", |raw| unsafe {
            sys::TextEditor_SetMiniMapColumns(raw, value)
        });
    }

    pub fn minimap_columns(&self) -> usize {
        self.with_context("TextEditor::minimap_columns", |raw| unsafe {
            sys::TextEditor_GetMiniMapColumns(raw)
        })
    }

    pub fn set_middle_mouse_mode(&mut self, mode: MiddleMouseMode) {
        self.with_context("TextEditor::set_middle_mouse_mode", |raw| unsafe {
            match mode {
                MiddleMouseMode::Pan => sys::TextEditor_SetMiddleMousePanMode(raw),
                MiddleMouseMode::Scroll => sys::TextEditor_SetMiddleMouseScrollMode(raw),
            }
        });
    }

    pub fn middle_mouse_mode(&self) -> MiddleMouseMode {
        self.with_context("TextEditor::middle_mouse_mode", |raw| unsafe {
            if sys::TextEditor_IsMiddleMousePanMode(raw) {
                MiddleMouseMode::Pan
            } else {
                MiddleMouseMode::Scroll
            }
        })
    }

    pub fn set_line_number_left_margin(&mut self, value: usize) {
        self.with_context("TextEditor::set_line_number_left_margin", |raw| unsafe {
            sys::TextEditor_SetLineNumberLeftMargin(raw, value)
        });
    }

    pub fn line_number_left_margin(&self) -> usize {
        self.with_context("TextEditor::line_number_left_margin", |raw| unsafe {
            sys::TextEditor_GetLineNumberLeftMargin(raw)
        })
    }

    pub fn set_decoration_left_margin(&mut self, value: usize) {
        self.with_context("TextEditor::set_decoration_left_margin", |raw| unsafe {
            sys::TextEditor_SetDecorationLeftMargin(raw, value)
        });
    }

    pub fn decoration_left_margin(&self) -> usize {
        self.with_context("TextEditor::decoration_left_margin", |raw| unsafe {
            sys::TextEditor_GetDecorationLeftMargin(raw)
        })
    }

    pub fn set_text_left_margin(&mut self, value: usize) {
        self.with_context("TextEditor::set_text_left_margin", |raw| unsafe {
            sys::TextEditor_SetTextLeftMargin(raw, value)
        });
    }

    pub fn text_left_margin(&self) -> usize {
        self.with_context("TextEditor::text_left_margin", |raw| unsafe {
            sys::TextEditor_GetTextLeftMargin(raw)
        })
    }

    bool_property!(
        set_insert_spaces_on_tabs,
        inserts_spaces_on_tabs,
        sys::TextEditor_SetInsertSpacesOnTabs,
        sys::TextEditor_IsInsertSpacesOnTabs
    );
    bool_property!(
        set_word_wrap_enabled,
        is_word_wrap_enabled,
        sys::TextEditor_SetWordWrapEnabled,
        sys::TextEditor_IsWordWrapEnabled
    );
    bool_property!(
        set_read_only,
        is_read_only,
        sys::TextEditor_SetReadOnlyEnabled,
        sys::TextEditor_IsReadOnlyEnabled
    );
    bool_property!(
        set_carets_visible,
        are_carets_visible,
        sys::TextEditor_SetCaretsVisible,
        sys::TextEditor_IsCaretsVisible
    );
    bool_property!(
        set_auto_indent_enabled,
        is_auto_indent_enabled,
        sys::TextEditor_SetAutoIndentEnabled,
        sys::TextEditor_IsAutoIndentEnabled
    );
    bool_property!(
        set_show_whitespaces,
        shows_whitespaces,
        sys::TextEditor_SetShowWhitespacesEnabled,
        sys::TextEditor_IsShowWhitespacesEnabled
    );
    bool_property!(
        set_show_spaces,
        shows_spaces,
        sys::TextEditor_SetShowSpacesEnabled,
        sys::TextEditor_IsShowSpacesEnabled
    );
    bool_property!(
        set_show_tabs,
        shows_tabs,
        sys::TextEditor_SetShowTabsEnabled,
        sys::TextEditor_IsShowTabsEnabled
    );
    bool_property!(
        set_show_line_numbers,
        shows_line_numbers,
        sys::TextEditor_SetShowLineNumbersEnabled,
        sys::TextEditor_IsShowLineNumbersEnabled
    );
    bool_property!(
        set_show_minimap,
        shows_minimap,
        sys::TextEditor_SetShowMiniMapEnabled,
        sys::TextEditor_IsShowMiniMapEnabled
    );
    bool_property!(
        set_show_scrollbar_minimap,
        shows_scrollbar_minimap,
        sys::TextEditor_SetShowScrollbarMiniMapEnabled,
        sys::TextEditor_IsShowScrollbarMiniMapEnabled
    );
    bool_property!(
        set_show_pan_scroll_indicator,
        shows_pan_scroll_indicator,
        sys::TextEditor_SetShowPanScrollIndicatorEnabled,
        sys::TextEditor_IsShowPanScrollIndicatorEnabled
    );
    bool_property!(
        set_show_matching_brackets,
        shows_matching_brackets,
        sys::TextEditor_SetShowMatchingBrackets,
        sys::TextEditor_IsShowingMatchingBrackets
    );
    bool_property!(
        set_complete_paired_glyphs,
        completes_paired_glyphs,
        sys::TextEditor_SetCompletePairedGlyphs,
        sys::TextEditor_IsCompletingPairedGlyphs
    );
    bool_property!(
        set_line_folding_enabled,
        is_line_folding_enabled,
        sys::TextEditor_SetLineFoldingEnabled,
        sys::TextEditor_IsLineFoldingEnabled
    );
    bool_property!(
        set_overwrite_enabled,
        is_overwrite_enabled,
        sys::TextEditor_SetOverwriteEnabled,
        sys::TextEditor_IsOverwriteEnabled
    );
}
