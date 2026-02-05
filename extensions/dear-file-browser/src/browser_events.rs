use std::path::PathBuf;

use crate::core::{ClickAction, SortBy};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Modifiers {
    pub(crate) ctrl: bool,
    pub(crate) shift: bool,
}

/// Domain events for driving the in-UI file browser.
///
/// These events are intended to be emitted by the UI layer (Dear ImGui) and
/// handled by the core reducer. Keeping them free of ImGui types makes the core
/// testable.
#[derive(Clone, Debug)]
pub(crate) enum BrowserEvent {
    NavigateUp,
    NavigateTo(PathBuf),

    StartPathEdit,
    SubmitPathEdit,
    CancelPathEdit,
    RequestSearchFocus,

    SetShowHidden(bool),
    SetActiveFilter(Option<usize>),
    SetSearch(String),
    SetSort {
        by: SortBy,
        ascending: bool,
    },
    SetClickAction(ClickAction),
    SetDoubleClick(bool),

    ClickEntry {
        name: String,
        is_dir: bool,
        modifiers: Modifiers,
    },
    DoubleClickEntry {
        name: String,
        is_dir: bool,
    },

    MoveFocus {
        delta: i32,
        modifiers: Modifiers,
    },
    ActivateFocused,

    SelectAll,
    Confirm,
    Cancel,
}
