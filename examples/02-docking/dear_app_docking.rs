use dear_app::{AddOnsConfig, AppBuilder, DockingConfig, RedrawMode, RunnerConfig};
use dear_imgui_rs::*;

struct DockDemoState {
    // Window visibility
    show_main: bool,
    show_command: bool,
    show_command2: bool,
    show_misc: bool,
    show_logs: bool,
    show_imgui_demo: bool,

    layout_needs_apply: bool,
}

impl Default for DockDemoState {
    fn default() -> Self {
        Self {
            show_main: true,
            show_command: true,
            show_command2: true,
            show_misc: true,
            show_logs: true,
            show_imgui_demo: false,

            layout_needs_apply: true,
        }
    }
}

fn apply_default_layout(ui: &Ui, dockspace_id: Id) {
    // Clear previous layout and rebuild
    let vp = ui.main_viewport();
    let size = vp.size();

    DockBuilder::remove_node(dockspace_id);
    let root = DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
    DockBuilder::set_node_size(root, [size[0], size[1]]);

    // Layout like the C++ demo comments:
    // Split Main -> Down: Misc (25%) + Main
    let (misc, main) = DockBuilder::split_node_pair(root, SplitDirection::Down, 0.25);
    // Split Main -> Left: Command (25%) + Main
    let (command, main) = DockBuilder::split_node_pair(main, SplitDirection::Left, 0.25);
    // Split Main -> Down: Command2 (50%) + Main
    let (command2, main) = DockBuilder::split_node_pair(main, SplitDirection::Down, 0.5);

    // Dock windows
    DockBuilder::dock_window("Main View", main);
    DockBuilder::dock_window("Command", command);
    DockBuilder::dock_window("Command 2", command2);
    DockBuilder::dock_window("Misc", misc);
    DockBuilder::dock_window("Logs", misc);

    DockBuilder::finish(root);
}

fn apply_alternative_layout(ui: &Ui, dockspace_id: Id) {
    let vp = ui.main_viewport();
    let size = vp.size();

    DockBuilder::remove_node(dockspace_id);
    let root = DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
    DockBuilder::set_node_size(root, [size[0], size[1]]);

    // Alternative layout: Misc on left (30%), bottom split between Command and Command2
    let (misc, main) = DockBuilder::split_node_pair(root, SplitDirection::Left, 0.30);
    let (command, bottom) = DockBuilder::split_node_pair(main, SplitDirection::Down, 0.35);
    let (command2, main) = DockBuilder::split_node_pair(bottom, SplitDirection::Right, 0.5);

    DockBuilder::dock_window("Main View", main);
    DockBuilder::dock_window("Command", command);
    DockBuilder::dock_window("Command 2", command2);
    DockBuilder::dock_window("Misc", misc);
    DockBuilder::dock_window("Logs", misc);

    DockBuilder::finish(root);
}

fn main() {
    dear_imgui_rs::logging::init_tracing_with_filter(
        "dear_imgui=info,dear_app_docking=info,wgpu=warn",
    );

    let runner = RunnerConfig {
        window_title: "Dear App Docking Demo".to_string(),
        window_size: (1280.0, 720.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.1, 0.1, 0.12, 1.0],
        docking: DockingConfig {
            enable: true,
            auto_dockspace: false, // we'll create our own with dynamic flags
            ..Default::default()
        },
        ini_filename: Some(std::path::PathBuf::from("Docking_Demo/docking_demo.ini")),
        restore_previous_geometry: true,
        redraw: RedrawMode::Poll,
        io_config_flags: None,
        theme: Some(dear_app::Theme::Dark),
    };

    // Enable add-ons compiled into dear-app via features
    let addons = AddOnsConfig::auto();

    let mut state = DockDemoState::default();

    AppBuilder::new()
        .with_config(runner)
        .with_addons(addons)
        .on_frame(move |ui, addons| {
            // Global main menu bar
            if let Some(_mb) = ui.begin_main_menu_bar() {
                if let Some(_m) = ui.begin_menu("View") {
                    if ui.menu_item("ImGui Demo") {
                        state.show_imgui_demo = !state.show_imgui_demo;
                    }
                    if ui.menu_item("Apply Default Layout") {
                        state.layout_needs_apply = true;
                    }
                    if ui.menu_item("Apply Alternative Layout") {
                        // mark and store alt in a flag; for simplicity, apply immediately
                        let dockspace_id = ui.get_id("MainDockSpace");
                        apply_alternative_layout(ui, dockspace_id);
                    }
                    _m.end();
                }
                if let Some(_m) = ui.begin_menu("Windows") {
                    ui.menu_item_toggle("Main View", None::<&str>, &mut state.show_main, true);
                    ui.menu_item_toggle("Command", None::<&str>, &mut state.show_command, true);
                    ui.menu_item_toggle("Command 2", None::<&str>, &mut state.show_command2, true);
                    ui.menu_item_toggle("Misc", None::<&str>, &mut state.show_misc, true);
                    ui.menu_item_toggle("Logs", None::<&str>, &mut state.show_logs, true);
                    _m.end();
                }
                _mb.end();
            }

            // Fullscreen host window with dockspace
            let viewport = ui.main_viewport();
            ui.set_next_window_viewport(Id::from(viewport.id()));
            let pos = viewport.pos();
            let size = viewport.size();

            let mut host_flags = WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                | WindowFlags::NO_NAV_FOCUS
                | WindowFlags::NO_DOCKING
                | WindowFlags::MENU_BAR;

            let dock_flags = addons.docking.flags();
            if dock_flags.contains(DockFlags::PASSTHRU_CENTRAL_NODE) {
                host_flags |= WindowFlags::NO_BACKGROUND;
            }

            // Zero rounding/border and remove padding for a clean host window
            let rounding = ui.push_style_var(StyleVar::WindowRounding(0.0));
            let border = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
            let padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));

            ui.window("DockSpaceHost")
                .flags(host_flags)
                .position([pos[0], pos[1]], Condition::Always)
                .size([size[0], size[1]], Condition::Always)
                .build(|| {
                    // Pop padding/border/rounding to restore defaults inside
                    padding.pop();
                    border.pop();
                    rounding.pop();
                    let dockspace_id = ui.get_id("MainDockSpace");
                    let avail = ui.content_region_avail();
                    let _ = ui.dock_space_with_class(dockspace_id, avail, dock_flags, None);

                    // Apply layout on first run or on request
                    if state.layout_needs_apply {
                        apply_default_layout(ui, dockspace_id);
                        state.layout_needs_apply = false;
                    }

                    // Optional: a small toolbar of docking flags toggles (update dear-app runtime flags)
                    if let Some(_bar) = ui.begin_menu_bar() {
                        if let Some(_menu) = ui.begin_menu("Docking Flags") {
                            let mut new_flags = addons.docking.flags();
                            // Build flags from simple toggles (demo purpose)
                            let mut no_split = new_flags.contains(DockFlags::NO_DOCKING_SPLIT);
                            let mut no_resize = new_flags.contains(DockFlags::NO_RESIZE);
                            let mut auto_hide = new_flags.contains(DockFlags::AUTO_HIDE_TAB_BAR);
                            let mut no_central =
                                new_flags.contains(DockFlags::NO_DOCKING_OVER_CENTRAL_NODE);

                            if ui.menu_item_toggle("NoSplit", None::<&str>, &mut no_split, true) {
                                if no_split {
                                    new_flags |= DockFlags::NO_DOCKING_SPLIT;
                                } else {
                                    new_flags.remove(DockFlags::NO_DOCKING_SPLIT);
                                }
                            }
                            if ui.menu_item_toggle("NoResize", None::<&str>, &mut no_resize, true) {
                                if no_resize {
                                    new_flags |= DockFlags::NO_RESIZE;
                                } else {
                                    new_flags.remove(DockFlags::NO_RESIZE);
                                }
                            }
                            if ui.menu_item_toggle(
                                "AutoHideTabBar",
                                None::<&str>,
                                &mut auto_hide,
                                true,
                            ) {
                                if auto_hide {
                                    new_flags |= DockFlags::AUTO_HIDE_TAB_BAR;
                                } else {
                                    new_flags.remove(DockFlags::AUTO_HIDE_TAB_BAR);
                                }
                            }
                            if ui.menu_item_toggle(
                                "NoDockingOverCentral",
                                None::<&str>,
                                &mut no_central,
                                true,
                            ) {
                                if no_central {
                                    new_flags |= DockFlags::NO_DOCKING_OVER_CENTRAL_NODE;
                                } else {
                                    new_flags.remove(DockFlags::NO_DOCKING_OVER_CENTRAL_NODE);
                                }
                            }
                            // Apply runtime flags
                            addons.docking.set_flags(new_flags);
                            _menu.end();
                        }
                        _bar.end();
                    }
                });

            // Windows content
            if state.show_main {
                ui.window("Main View").build(|| {
                    ui.text("Main workspace");
                    ui.separator();
                    ui.text("Drag other windows and try layouts from the menu.");
                });
            }
            if state.show_command {
                ui.window("Command").build(|| {
                    ui.text("Commands and parameters");
                    ui.separator();
                    ui.text("- Option A\n- Option B\n- Option C");
                });
            }
            if state.show_command2 {
                ui.window("Command 2").build(|| {
                    ui.text("More commands");
                    ui.separator();
                    ui.text("- Action 1\n- Action 2\n- Action 3");
                });
            }
            if state.show_misc {
                ui.window("Misc").build(|| {
                    ui.text("Miscellaneous tools");
                    ui.separator();
                    ui.text("Use View menu to toggle windows.");
                });
            }
            if state.show_logs {
                ui.window("Logs").build(|| {
                    ui.text("Logs window (console output)");
                    ui.separator();
                    ui.text_wrapped("Check your terminal for tracing output.");
                });
            }

            if state.show_imgui_demo {
                ui.show_demo_window(&mut state.show_imgui_demo);
            }
        })
        .run()
        .unwrap();
}
