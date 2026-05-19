use std::path::{Path, PathBuf};

use crate::places::Place;

pub(super) struct PlacesEditRequest {
    mode: crate::dialog_state::PlacesEditMode,
    group: String,
    group_from: Option<String>,
    place_from_path: Option<PathBuf>,
    place_label: String,
    place_path: String,
    focus: bool,
}

impl PlacesEditRequest {
    pub(super) fn add_place(group: &str, cwd: &Path) -> Self {
        let label = cwd
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| cwd.display().to_string());
        Self {
            mode: crate::dialog_state::PlacesEditMode::AddPlace,
            group: group.to_string(),
            group_from: None,
            place_from_path: None,
            place_label: label,
            place_path: cwd.display().to_string(),
            focus: true,
        }
    }

    pub(super) fn edit_place(group: &str, p: &Place) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::EditPlace,
            group: group.to_string(),
            group_from: None,
            place_from_path: Some(p.path.clone()),
            place_label: p.label.clone(),
            place_path: p.path.display().to_string(),
            focus: true,
        }
    }

    pub(super) fn rename_group(group: &str) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::RenameGroup,
            group: group.to_string(),
            group_from: Some(group.to_string()),
            place_from_path: None,
            place_label: String::new(),
            place_path: String::new(),
            focus: true,
        }
    }

    pub(super) fn remove_group_confirm(group: &str) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::RemoveGroupConfirm,
            group: group.to_string(),
            group_from: Some(group.to_string()),
            place_from_path: None,
            place_label: String::new(),
            place_path: String::new(),
            focus: false,
        }
    }

    pub(super) fn apply_to_state(self, ui: &mut crate::FileDialogUiState) {
        ui.operations.places.edit.mode = self.mode;
        ui.operations.places.edit.group = self.group;
        ui.operations.places.edit.group_from = self.group_from;
        ui.operations.places.edit.place_from_path = self.place_from_path;
        ui.operations.places.edit.place_label = self.place_label;
        ui.operations.places.edit.place_path = self.place_path;
        ui.operations.places.edit.error = None;
        ui.operations.places.edit.open_next = true;
        ui.operations.places.edit.focus_next = self.focus;
    }
}
