use super::{
    TextEditor, copy_c_string, validate_cursor, validate_line, validate_position,
    validate_selection,
};
use crate::{
    CteError, CteResult, Position, ScrollAlignment, SearchOptions, Selection, SquiggleKind,
    VisualPosition, error::c_string, sys, validation::validate_finite_vec2,
};

macro_rules! editor_command {
    ($name:ident, $raw:path) => {
        pub fn $name(&mut self) {
            self.with_context(concat!("TextEditor::", stringify!($name)), |raw| unsafe {
                $raw(raw)
            });
        }
    };
}

macro_rules! editor_bool_query {
    ($name:ident, $raw:path) => {
        pub fn $name(&self) -> bool {
            self.with_context(concat!("TextEditor::", stringify!($name)), |raw| unsafe {
                $raw(raw)
            })
        }
    };
}

impl TextEditor {
    editor_command!(focus, sys::TextEditor_SetFocus);
    editor_command!(cut, sys::TextEditor_Cut);
    editor_command!(copy, sys::TextEditor_Copy);
    editor_command!(paste, sys::TextEditor_Paste);
    editor_command!(undo, sys::TextEditor_Undo);
    editor_command!(redo, sys::TextEditor_Redo);
    editor_bool_query!(can_undo, sys::TextEditor_CanUndo);
    editor_bool_query!(can_redo, sys::TextEditor_CanRedo);

    pub fn undo_index(&self) -> usize {
        self.with_context("TextEditor::undo_index", |raw| unsafe {
            sys::TextEditor_GetUndoIndex(raw)
        })
    }

    editor_command!(select_all, sys::TextEditor_SelectAll);

    pub fn select_line(&mut self, line: usize) -> CteResult<()> {
        self.with_context("TextEditor::select_line", |raw| unsafe {
            validate_line(raw, "TextEditor::select_line", line)?;
            sys::TextEditor_SelectLine(raw, line);
            Ok(())
        })
    }

    /// Selects an inclusive range of document lines.
    pub fn select_lines(&mut self, start: usize, end: usize) -> CteResult<()> {
        self.with_context("TextEditor::select_lines", |raw| unsafe {
            validate_line(raw, "TextEditor::select_lines", start)?;
            validate_line(raw, "TextEditor::select_lines", end)?;
            if start > end {
                return Err(CteError::InvalidValue {
                    operation: "TextEditor::select_lines",
                    parameter: "start and end",
                    requirement: "ordered from first line to last line",
                });
            }
            sys::TextEditor_SelectLines(raw, start, end);
            Ok(())
        })
    }

    pub fn select_region(&mut self, selection: Selection) -> CteResult<()> {
        self.with_context("TextEditor::select_region", |raw| unsafe {
            validate_selection(raw, "TextEditor::select_region", selection)?;
            sys::TextEditor_SelectRegion(raw, selection.start.into_raw(), selection.end.into_raw());
            Ok(())
        })
    }

    pub fn select_to_brackets(&mut self, include_brackets: bool) {
        self.with_context("TextEditor::select_to_brackets", |raw| unsafe {
            sys::TextEditor_SelectToBrackets(raw, include_brackets)
        });
    }

    editor_command!(grow_selections, sys::TextEditor_GrowSelections);
    editor_command!(shrink_selections, sys::TextEditor_ShrinkSelections);
    editor_command!(add_next_occurrence, sys::TextEditor_AddNextOccurrence);
    editor_command!(select_all_occurrences, sys::TextEditor_SelectAllOccurrences);
    editor_bool_query!(
        any_cursor_has_selection,
        sys::TextEditor_AnyCursorHasSelection
    );
    editor_bool_query!(
        all_cursors_have_selection,
        sys::TextEditor_AllCursorsHaveSelection
    );
    editor_bool_query!(
        main_cursor_has_selection,
        sys::TextEditor_MainCursorHasSelection
    );
    editor_bool_query!(
        current_cursor_has_selection,
        sys::TextEditor_CurrentCursorHasSelection
    );
    editor_command!(clear_cursors, sys::TextEditor_ClearCursors);

    pub fn cursor_count(&self) -> usize {
        self.with_context("TextEditor::cursor_count", |raw| unsafe {
            sys::TextEditor_GetNumberOfCursors(raw)
        })
    }

    pub fn cursor_has_selection(&self, cursor: usize) -> CteResult<bool> {
        self.with_context("TextEditor::cursor_has_selection", |raw| unsafe {
            validate_cursor(raw, "TextEditor::cursor_has_selection", cursor)?;
            Ok(sys::TextEditor_CursorHasSelection(raw, cursor))
        })
    }

    pub fn cursor_position(&self, cursor: usize) -> CteResult<Position> {
        self.with_context("TextEditor::cursor_position", |raw| unsafe {
            validate_cursor(raw, "TextEditor::cursor_position", cursor)?;
            Ok(Position::from_raw(sys::TextEditor_GetCursorPosition(
                raw, cursor,
            )))
        })
    }

    pub fn main_cursor_position(&self) -> Position {
        self.with_context("TextEditor::main_cursor_position", |raw| unsafe {
            Position::from_raw(sys::TextEditor_GetMainCursorPosition(raw))
        })
    }

    pub fn current_cursor_position(&self) -> Position {
        self.with_context("TextEditor::current_cursor_position", |raw| unsafe {
            Position::from_raw(sys::TextEditor_GetCurrentCursorPosition(raw))
        })
    }

    pub fn cursor_selection(&self, cursor: usize) -> CteResult<Selection> {
        self.with_context("TextEditor::cursor_selection", |raw| unsafe {
            validate_cursor(raw, "TextEditor::cursor_selection", cursor)?;
            Ok(Selection::from_raw(sys::TextEditor_GetCursorSelection(
                raw, cursor,
            )))
        })
    }

    pub fn main_cursor_selection(&self) -> Selection {
        self.with_context("TextEditor::main_cursor_selection", |raw| unsafe {
            Selection::from_raw(sys::TextEditor_GetMainCursorSelection(raw))
        })
    }

    pub fn current_cursor_selection(&self) -> Selection {
        self.with_context("TextEditor::current_cursor_selection", |raw| unsafe {
            Selection::from_raw(sys::TextEditor_GetCurrentCursorSelection(raw))
        })
    }

    pub fn set_cursor(&mut self, position: Position) -> CteResult<()> {
        self.with_context("TextEditor::set_cursor", |raw| unsafe {
            validate_position(raw, "TextEditor::set_cursor", position)?;
            sys::TextEditor_SetCursor(raw, position.into_raw());
            Ok(())
        })
    }

    pub fn is_mouse_over_glyph(&self, mouse_position: [f32; 2]) -> CteResult<bool> {
        validate_finite_vec2(
            "TextEditor::is_mouse_over_glyph",
            "mouse_position",
            mouse_position,
        )?;
        if !self.layout_ready {
            return Ok(false);
        }
        Ok(
            self.with_context("TextEditor::is_mouse_over_glyph", |raw| unsafe {
                sys::TextEditor_IsMousePosOverGlyph(raw, mouse_position.into())
            }),
        )
    }

    pub fn is_mouse_over_text_area(&self, mouse_position: [f32; 2]) -> CteResult<bool> {
        validate_finite_vec2(
            "TextEditor::is_mouse_over_text_area",
            "mouse_position",
            mouse_position,
        )?;
        if !self.layout_ready {
            return Ok(false);
        }
        Ok(
            self.with_context("TextEditor::is_mouse_over_text_area", |raw| unsafe {
                sys::TextEditor_IsMousePosOverTextArea(raw, mouse_position.into())
            }),
        )
    }

    pub fn position_at_mouse(&self, mouse_position: [f32; 2]) -> CteResult<Position> {
        validate_finite_vec2(
            "TextEditor::position_at_mouse",
            "mouse_position",
            mouse_position,
        )?;
        if !self.layout_ready {
            return Ok(Position::default());
        }
        Ok(
            self.with_context("TextEditor::position_at_mouse", |raw| unsafe {
                Position::from_raw(sys::TextEditor_GetDocPosAtMousePos(
                    raw,
                    mouse_position.into(),
                ))
            }),
        )
    }

    pub fn word_at_mouse(&self, mouse_position: [f32; 2]) -> CteResult<String> {
        validate_finite_vec2(
            "TextEditor::word_at_mouse",
            "mouse_position",
            mouse_position,
        )?;
        if !self.layout_ready {
            return Ok(String::new());
        }
        self.with_context("TextEditor::word_at_mouse", |raw| unsafe {
            copy_c_string(
                "TextEditor::word_at_mouse",
                sys::TextEditor_GetWordAtMousePos(raw, mouse_position.into()),
            )
        })
    }

    pub fn scroll_to_line(&mut self, line: usize, alignment: ScrollAlignment) -> CteResult<()> {
        self.with_context("TextEditor::scroll_to_line", |raw| unsafe {
            validate_line(raw, "TextEditor::scroll_to_line", line)?;
            sys::TextEditor_ScrollToLine(raw, line, alignment.into_raw());
            Ok(())
        })
    }

    pub fn first_visible_row(&self) -> usize {
        self.with_context("TextEditor::first_visible_row", |raw| unsafe {
            sys::TextEditor_GetFirstVisibleRow(raw)
        })
    }

    pub fn last_visible_row(&self) -> usize {
        self.with_context("TextEditor::last_visible_row", |raw| unsafe {
            sys::TextEditor_GetLastVisibleRow(raw)
        })
    }

    pub fn first_visible_column(&self) -> usize {
        self.with_context("TextEditor::first_visible_column", |raw| unsafe {
            sys::TextEditor_GetFirstVisibleColumn(raw)
        })
    }

    pub fn last_visible_column(&self) -> usize {
        self.with_context("TextEditor::last_visible_column", |raw| unsafe {
            sys::TextEditor_GetLastVisibleColumn(raw)
        })
    }

    /// Returns the line height after the editor has produced a visible layout.
    pub fn line_height(&self) -> Option<f32> {
        self.layout_ready.then(|| {
            self.with_context("TextEditor::line_height", |raw| unsafe {
                sys::TextEditor_GetLineHeight(raw)
            })
        })
    }

    /// Returns the glyph width after the editor has produced a visible layout.
    pub fn glyph_width(&self) -> Option<f32> {
        self.layout_ready.then(|| {
            self.with_context("TextEditor::glyph_width", |raw| unsafe {
                sys::TextEditor_GetGlyphWidth(raw)
            })
        })
    }

    pub fn document_to_visual(&self, position: Position) -> CteResult<VisualPosition> {
        self.with_context("TextEditor::document_to_visual", |raw| unsafe {
            validate_position(raw, "TextEditor::document_to_visual", position)?;
            Ok(VisualPosition::from_raw(sys::TextEditor_DocPos2VisPos(
                raw,
                position.into_raw(),
            )))
        })
    }

    pub fn visual_to_document(&self, position: VisualPosition) -> Position {
        self.with_context("TextEditor::visual_to_document", |raw| unsafe {
            Position::from_raw(sys::TextEditor_VisPos2DocPos(raw, position.into_raw()))
        })
    }

    pub fn is_position_visible(&self, position: Position) -> CteResult<bool> {
        self.with_context("TextEditor::is_position_visible", |raw| unsafe {
            validate_position(raw, "TextEditor::is_position_visible", position)?;
            Ok(sys::TextEditor_IsDocPosVisible(raw, position.into_raw()))
        })
    }

    pub fn is_visual_position_over_glyph(&self, position: VisualPosition) -> bool {
        self.with_context("TextEditor::is_visual_position_over_glyph", |raw| unsafe {
            sys::TextEditor_IsVisPosOverGlyph(raw, position.into_raw())
        })
    }

    pub fn word_start(&self, position: Position) -> CteResult<Position> {
        self.with_context("TextEditor::word_start", |raw| unsafe {
            validate_position(raw, "TextEditor::word_start", position)?;
            Ok(Position::from_raw(sys::TextEditor_FindWordStart(
                raw,
                position.into_raw(),
            )))
        })
    }

    pub fn word_end(&self, position: Position) -> CteResult<Position> {
        self.with_context("TextEditor::word_end", |raw| unsafe {
            validate_position(raw, "TextEditor::word_end", position)?;
            Ok(Position::from_raw(sys::TextEditor_FindWordEnd(
                raw,
                position.into_raw(),
            )))
        })
    }

    pub fn select_first_occurrence(&mut self, text: &str, options: SearchOptions) -> CteResult<()> {
        let text = c_string("TextEditor::select_first_occurrence", text)?;
        self.with_context("TextEditor::select_first_occurrence", |raw| unsafe {
            sys::TextEditor_SelectFirstOccurrenceOf(
                raw,
                text.as_ptr(),
                options.case_sensitive,
                options.whole_word,
            )
        });
        Ok(())
    }

    pub fn select_next_occurrence(&mut self, text: &str, options: SearchOptions) -> CteResult<()> {
        let text = c_string("TextEditor::select_next_occurrence", text)?;
        self.with_context("TextEditor::select_next_occurrence", |raw| unsafe {
            sys::TextEditor_SelectNextOccurrenceOf(
                raw,
                text.as_ptr(),
                options.case_sensitive,
                options.whole_word,
            )
        });
        Ok(())
    }

    pub fn select_every_occurrence(&mut self, text: &str, options: SearchOptions) -> CteResult<()> {
        let text = c_string("TextEditor::select_every_occurrence", text)?;
        self.with_context("TextEditor::select_every_occurrence", |raw| unsafe {
            sys::TextEditor_SelectAllOccurrencesOf(
                raw,
                text.as_ptr(),
                options.case_sensitive,
                options.whole_word,
            )
        });
        Ok(())
    }

    pub fn replace_current_selection(&mut self, text: &str) -> CteResult<()> {
        let text = c_string("TextEditor::replace_current_selection", text)?;
        self.with_context("TextEditor::replace_current_selection", |raw| unsafe {
            sys::TextEditor_ReplaceTextInCurrentCursor(raw, text.as_ptr())
        });
        Ok(())
    }

    pub fn replace_all_selections(&mut self, text: &str) -> CteResult<()> {
        let text = c_string("TextEditor::replace_all_selections", text)?;
        self.with_context("TextEditor::replace_all_selections", |raw| unsafe {
            sys::TextEditor_ReplaceTextInAllCursors(raw, text.as_ptr())
        });
        Ok(())
    }

    editor_command!(open_find_replace, sys::TextEditor_OpenFindReplaceWindow);
    editor_command!(close_find_replace, sys::TextEditor_CloseFindReplaceWindow);
    editor_bool_query!(has_find_string, sys::TextEditor_HasFindString);
    editor_command!(find_next, sys::TextEditor_FindNext);
    editor_command!(find_all, sys::TextEditor_FindAll);

    pub fn set_find_button_label(&mut self, label: &str) -> CteResult<()> {
        self.set_find_label(
            "TextEditor::set_find_button_label",
            label,
            sys::TextEditor_SetFindButtonLabel,
        )
    }

    pub fn set_find_all_button_label(&mut self, label: &str) -> CteResult<()> {
        self.set_find_label(
            "TextEditor::set_find_all_button_label",
            label,
            sys::TextEditor_SetFindAllButtonLabel,
        )
    }

    pub fn set_replace_button_label(&mut self, label: &str) -> CteResult<()> {
        self.set_find_label(
            "TextEditor::set_replace_button_label",
            label,
            sys::TextEditor_SetReplaceButtonLabel,
        )
    }

    pub fn set_replace_all_button_label(&mut self, label: &str) -> CteResult<()> {
        self.set_find_label(
            "TextEditor::set_replace_all_button_label",
            label,
            sys::TextEditor_SetReplaceAllButtonLabel,
        )
    }

    pub fn add_marker(
        &mut self,
        line: usize,
        line_number_color: u32,
        text_color: u32,
        line_number_tooltip: &str,
        text_tooltip: &str,
    ) -> CteResult<()> {
        let line_number_tooltip = c_string("TextEditor::add_marker", line_number_tooltip)?;
        let text_tooltip = c_string("TextEditor::add_marker", text_tooltip)?;
        self.with_context("TextEditor::add_marker", |raw| unsafe {
            validate_line(raw, "TextEditor::add_marker", line)?;
            sys::TextEditor_AddMarker(
                raw,
                line,
                line_number_color,
                text_color,
                line_number_tooltip.as_ptr(),
                text_tooltip.as_ptr(),
            );
            Ok(())
        })
    }

    editor_command!(clear_markers, sys::TextEditor_ClearMarkers);
    editor_bool_query!(has_markers, sys::TextEditor_HasMarkers);

    pub fn add_squiggle(
        &mut self,
        selection: Selection,
        kind: SquiggleKind,
        color: u32,
        tooltip: &str,
    ) -> CteResult<()> {
        let tooltip = c_string("TextEditor::add_squiggle", tooltip)?;
        self.with_context("TextEditor::add_squiggle", |raw| unsafe {
            validate_selection(raw, "TextEditor::add_squiggle", selection)?;
            sys::TextEditor_AddSquiggle(
                raw,
                selection.start.into_raw(),
                selection.end.into_raw(),
                kind.get(),
                color,
                tooltip.as_ptr(),
            );
            Ok(())
        })
    }

    pub fn clear_squiggles_in(&mut self, selection: Selection) -> CteResult<()> {
        self.with_context("TextEditor::clear_squiggles_in", |raw| unsafe {
            validate_selection(raw, "TextEditor::clear_squiggles_in", selection)?;
            sys::TextEditor_ClearSquiggles_DocPos(
                raw,
                selection.start.into_raw(),
                selection.end.into_raw(),
            );
            Ok(())
        })
    }

    pub fn clear_squiggles_of_kind(&mut self, kind: SquiggleKind) {
        self.with_context("TextEditor::clear_squiggles_of_kind", |raw| unsafe {
            sys::TextEditor_ClearSquiggles_size_t(raw, kind.get())
        });
    }

    editor_command!(clear_squiggles, sys::TextEditor_ClearSquiggles_Nil);
    editor_bool_query!(has_squiggles, sys::TextEditor_HasSquiggles);

    pub fn fold_around_line(&mut self, line: usize) -> CteResult<()> {
        self.line_command(
            "TextEditor::fold_around_line",
            line,
            sys::TextEditor_FoldAroundLine,
        )
    }

    pub fn unfold_around_line(&mut self, line: usize) -> CteResult<()> {
        self.line_command(
            "TextEditor::unfold_around_line",
            line,
            sys::TextEditor_UnfoldAroundLine,
        )
    }

    pub fn toggle_fold_at_line(&mut self, line: usize) -> CteResult<()> {
        self.line_command(
            "TextEditor::toggle_fold_at_line",
            line,
            sys::TextEditor_ToggleAtLine,
        )
    }

    editor_command!(unfold_all, sys::TextEditor_UnfoldAll);

    pub fn is_line_foldable(&self, line: usize) -> CteResult<bool> {
        self.line_query(
            "TextEditor::is_line_foldable",
            line,
            sys::TextEditor_IsLineFoldable,
        )
    }

    pub fn is_line_folded(&self, line: usize) -> CteResult<bool> {
        self.line_query(
            "TextEditor::is_line_folded",
            line,
            sys::TextEditor_IsLineFolded,
        )
    }

    pub fn is_line_visible(&self, line: usize) -> CteResult<bool> {
        self.line_query(
            "TextEditor::is_line_visible",
            line,
            sys::TextEditor_IsLineVisible,
        )
    }

    pub fn is_line_hidden(&self, line: usize) -> CteResult<bool> {
        self.line_query(
            "TextEditor::is_line_hidden",
            line,
            sys::TextEditor_IsLineHidden,
        )
    }

    editor_command!(indent_lines, sys::TextEditor_IndentLines);
    editor_command!(deindent_lines, sys::TextEditor_DeindentLines);
    editor_command!(move_lines_up, sys::TextEditor_MoveUpLines);
    editor_command!(move_lines_down, sys::TextEditor_MoveDownLines);
    editor_command!(toggle_comments, sys::TextEditor_ToggleComments);
    editor_command!(selection_to_lowercase, sys::TextEditor_SelectionToLowerCase);
    editor_command!(selection_to_uppercase, sys::TextEditor_SelectionToUpperCase);
    editor_command!(
        strip_trailing_whitespaces,
        sys::TextEditor_StripTrailingWhitespaces
    );
    editor_command!(tabs_to_spaces, sys::TextEditor_TabsToSpaces);
    editor_command!(spaces_to_tabs, sys::TextEditor_SpacesToTabs);

    fn set_find_label(
        &mut self,
        operation: &'static str,
        label: &str,
        setter: unsafe extern "C" fn(*mut sys::TextEditor, *const std::ffi::c_char),
    ) -> CteResult<()> {
        let label = c_string(operation, label)?;
        self.with_context(operation, |raw| unsafe { setter(raw, label.as_ptr()) });
        Ok(())
    }

    fn line_command(
        &mut self,
        operation: &'static str,
        line: usize,
        command: unsafe extern "C" fn(*mut sys::TextEditor, usize),
    ) -> CteResult<()> {
        self.with_context(operation, |raw| unsafe {
            validate_line(raw, operation, line)?;
            command(raw, line);
            Ok(())
        })
    }

    fn line_query(
        &self,
        operation: &'static str,
        line: usize,
        query: unsafe extern "C" fn(*mut sys::TextEditor, usize) -> bool,
    ) -> CteResult<bool> {
        self.with_context(operation, |raw| unsafe {
            validate_line(raw, operation, line)?;
            Ok(query(raw, line))
        })
    }
}
