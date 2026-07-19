//! Main menus, window menus, context menus, and modal popups.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::{Condition, Ui, WindowFlags};

struct MenuDemo {
    status_bar: bool,
    show_about: bool,
    preferences_open: bool,
    confirm_before_delete: bool,
    rename_buffer: String,
    sections: Vec<String>,
    selected: Option<usize>,
    confirm_delete_open: bool,
    status: String,
}

impl Default for MenuDemo {
    fn default() -> Self {
        Self {
            status_bar: true,
            show_about: false,
            preferences_open: false,
            confirm_before_delete: true,
            rename_buffer: "Untitled".to_owned(),
            sections: vec!["Introduction".to_owned(), "Details".to_owned()],
            selected: Some(0),
            confirm_delete_open: false,
            status: "Ready".to_owned(),
        }
    }
}

impl MenuDemo {
    fn delete_selected(&mut self) {
        let Some(index) = self.selected.take() else {
            return;
        };
        self.sections.remove(index);
        self.selected = (!self.sections.is_empty()).then(|| index.min(self.sections.len() - 1));
        self.status = "Section deleted".to_owned();
    }

    fn ui(&mut self, ui: &Ui) {
        if let Some(_menu_bar) = ui.begin_main_menu_bar() {
            ui.menu("File", || {
                if ui.menu_item_with_shortcut("New", "Ctrl+N") {
                    self.sections = vec!["Untitled".to_owned()];
                    self.selected = Some(0);
                    self.status = "New document".to_owned();
                }
                if ui.menu_item_with_shortcut("Save", "Ctrl+S") {
                    self.status = "Saved".to_owned();
                }
                ui.separator();
                if ui.menu_item("Preferences...") {
                    self.preferences_open = true;
                    ui.open_popup("Preferences");
                }
            });
            ui.menu("View", || {
                ui.menu_item_toggle_no_shortcut("Status Bar", &mut self.status_bar, true);
            });
            ui.menu("Help", || {
                if ui.menu_item("About Dear ImGui...") {
                    self.show_about = true;
                }
            });
        }

        ui.window("Document")
            .size([700.0, 420.0], Condition::FirstUseEver)
            .flags(WindowFlags::MENU_BAR)
            .build(|| {
                if let Some(_menu_bar) = ui.begin_menu_bar() {
                    ui.menu("Document", || {
                        if ui.menu_item("Rename...") {
                            if let Some(index) = self.selected {
                                self.rename_buffer = self.sections[index].clone();
                            }
                            ui.open_popup("RenameSection");
                        }
                        if ui.menu_item_enabled_selected_no_shortcut(
                            "Delete...",
                            false,
                            self.selected.is_some(),
                        ) {
                            if self.confirm_before_delete {
                                self.confirm_delete_open = true;
                                ui.open_popup("ConfirmDelete");
                            } else {
                                self.delete_selected();
                            }
                        }
                    });
                }

                ui.text_disabled("Right-click the document for more actions");
                ui.separator();
                ui.child_window("sections")
                    .size([0.0, 260.0])
                    .build(ui, || {
                        for (index, title) in self.sections.iter().enumerate() {
                            if ui
                                .selectable_config(format!("{title}##section-{index}"))
                                .selected(self.selected == Some(index))
                                .build()
                            {
                                self.selected = Some(index);
                            }
                        }
                    });

                if let Some(_popup) = ui.begin_popup_context_window() {
                    if ui.menu_item("Add Section") {
                        self.sections
                            .push(format!("New Section {}", self.sections.len() + 1));
                        self.selected = Some(self.sections.len() - 1);
                        self.status = "Section added".to_owned();
                        ui.close_current_popup();
                    }
                    if ui.menu_item_enabled_selected_no_shortcut(
                        "Duplicate",
                        false,
                        self.selected.is_some(),
                    ) {
                        if let Some(index) = self.selected {
                            let copy = format!("{} Copy", self.sections[index]);
                            self.sections.insert(index + 1, copy);
                            self.selected = Some(index + 1);
                            self.status = "Section duplicated".to_owned();
                        }
                        ui.close_current_popup();
                    }
                }

                if let Some(_popup) = ui.begin_popup("RenameSection") {
                    ui.input_text("Name", &mut self.rename_buffer).build();
                    if ui.button("Rename") {
                        if let Some(index) = self.selected {
                            self.sections[index].clone_from(&self.rename_buffer);
                            self.status = "Section renamed".to_owned();
                        }
                        ui.close_current_popup();
                    }
                    ui.same_line();
                    if ui.button("Cancel") {
                        ui.close_current_popup();
                    }
                }

                if self.confirm_delete_open
                    && let Some(_modal) = ui.begin_modal_popup("ConfirmDelete")
                {
                    ui.text("Delete the selected section?");
                    if ui.button("Delete") {
                        self.delete_selected();
                        self.confirm_delete_open = false;
                        ui.close_current_popup();
                    }
                    ui.same_line();
                    if ui.button("Cancel") {
                        self.confirm_delete_open = false;
                        ui.close_current_popup();
                    }
                }

                if self.status_bar {
                    ui.separator();
                    ui.text_disabled(&self.status);
                }
            });

        if self.preferences_open
            && let Some(_modal) = ui.begin_modal_popup("Preferences")
        {
            ui.checkbox("Show status bar", &mut self.status_bar);
            ui.checkbox("Confirm before delete", &mut self.confirm_before_delete);
            if ui.button("Close") {
                self.preferences_open = false;
                ui.close_current_popup();
            }
        }

        if self.show_about {
            ui.show_about_window(&mut self.show_about);
        }
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Menus and Popups".to_owned(),
        window_size: (1000.0, 680.0),
        ..Default::default()
    };
    let mut demo = MenuDemo::default();

    run_ui(config, move |ui| demo.ui(ui))
}
