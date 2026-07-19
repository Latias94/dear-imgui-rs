//! Virtualized log viewer with filtering and per-row context menus.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::{Condition, ListClipper, StyleColor, Ui};

#[derive(Clone, Copy)]
enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    fn color(self) -> [f32; 4] {
        match self {
            Self::Debug => [0.6, 0.6, 0.6, 1.0],
            Self::Info => [0.6, 0.9, 1.0, 1.0],
            Self::Warn => [1.0, 0.8, 0.2, 1.0],
            Self::Error => [1.0, 0.3, 0.3, 1.0],
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warn => 2,
            Self::Error => 3,
        }
    }
}

#[derive(Clone)]
struct LogEntry {
    level: Level,
    message: String,
}

struct LogViewer {
    entries: Vec<LogEntry>,
    filtered: Vec<usize>,
    filter: String,
    levels: [bool; 4],
    auto_scroll: bool,
    remembered: Option<String>,
    sequence: u64,
}

impl LogViewer {
    fn new() -> Self {
        let mut viewer = Self {
            entries: Vec::new(),
            filtered: Vec::new(),
            filter: String::new(),
            levels: [true; 4],
            auto_scroll: true,
            remembered: None,
            sequence: 0,
        };
        viewer.generate(2_000);
        viewer.rebuild_filter();
        viewer
    }

    fn generate(&mut self, count: usize) {
        for _ in 0..count {
            self.sequence += 1;
            let level = match self.sequence % 17 {
                0 | 1 => Level::Warn,
                2 => Level::Error,
                3 | 4 => Level::Debug,
                _ => Level::Info,
            };
            self.entries.push(LogEntry {
                level,
                message: format!(
                    "Event #{:06} - simulated payload value={}",
                    self.sequence,
                    self.sequence % 97
                ),
            });
        }
    }

    fn rebuild_filter(&mut self) {
        let filter = self.filter.to_lowercase();
        self.filtered.clear();
        self.filtered.extend(
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| self.levels[entry.level.index()])
                .filter(|(_, entry)| {
                    filter.is_empty() || entry.message.to_lowercase().contains(&filter)
                })
                .map(|(index, _)| index),
        );
    }

    fn ui(&mut self, ui: &Ui) {
        let mut add_count = 0;
        let mut clear = false;
        let mut filter_changed = false;
        let mut duplicate = None;
        let mut delete = None;
        let mut remember = None;

        ui.window("Virtualized Log")
            .size([1000.0, 640.0], Condition::FirstUseEver)
            .build(|| {
                let _width = ui.push_item_width(280.0);
                filter_changed |= ui.input_text("Filter", &mut self.filter).build();
                drop(_width);

                ui.same_line();
                for (index, label) in ["Debug", "Info", "Warn", "Error"].iter().enumerate() {
                    filter_changed |= ui.checkbox(label, &mut self.levels[index]);
                    ui.same_line();
                }
                ui.checkbox("Auto-scroll", &mut self.auto_scroll);

                if ui.button("Add 1K") {
                    add_count = 1_000;
                }
                ui.same_line();
                if ui.button("Add 10K") {
                    add_count = 10_000;
                }
                ui.same_line();
                clear = ui.button("Clear");
                if let Some(message) = &self.remembered {
                    ui.same_line();
                    ui.text_disabled(format!("Remembered: {message}"));
                }
                ui.separator();

                ui.child_window("log-view").size([0.0, 0.0]).build(ui, || {
                    let was_at_bottom = ui.scroll_y() >= ui.scroll_max_y();
                    for visible_index in ListClipper::new(self.filtered.len()).begin(ui).iter() {
                        let entry_index = self.filtered[visible_index];
                        let entry = &self.entries[entry_index];
                        let _color = ui.push_style_color(StyleColor::Text, entry.level.color());
                        ui.selectable_config(format!(
                            "[{}] {}##log-{entry_index}",
                            entry.level.label(),
                            entry.message
                        ))
                        .build();

                        if let Some(_popup) = ui.begin_popup_context_item() {
                            if ui.menu_item("Remember line") {
                                remember = Some(entry.message.clone());
                            }
                            if ui.menu_item("Duplicate") {
                                duplicate = Some(entry_index);
                            }
                            if ui.menu_item("Delete") {
                                delete = Some(entry_index);
                            }
                        }
                    }
                    if self.auto_scroll && was_at_bottom {
                        ui.set_scroll_here_y(1.0);
                    }
                });
            });

        if clear {
            self.entries.clear();
        } else {
            if add_count > 0 {
                self.generate(add_count);
            }
            if let Some(index) = duplicate {
                if let Some(entry) = self.entries.get(index).cloned() {
                    self.entries.insert(index + 1, entry);
                }
            }
            if let Some(index) = delete {
                if index < self.entries.len() {
                    self.entries.remove(index);
                }
            }
        }
        if let Some(message) = remember {
            self.remembered = Some(message);
        }
        if clear || add_count > 0 || duplicate.is_some() || delete.is_some() || filter_changed {
            self.rebuild_filter();
        }
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - List Clipper".to_owned(),
        window_size: (1200.0, 720.0),
        ..Default::default()
    };
    let mut viewer = LogViewer::new();

    run_ui(config, move |ui| viewer.ui(ui))
}
