use dear_app::{
    AddOnsConfig, AppConfig, Application, DockingConfig, FrameContext, RedrawMode, RunError,
    WgpuConfig, WgpuPreset, run,
};
use dear_imgui_rs::*;

struct DockDemoState {
    // Window visibility
    show_main: bool,
    show_command: bool,
    show_command2: bool,
    show_misc: bool,
    show_logs: bool,
    show_imgui_demo: bool,

    pending_layout: Option<DemoLayout>,
}

#[derive(Clone, Copy)]
enum DemoLayout {
    Default,
    Alternative,
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

            pending_layout: None,
        }
    }
}

fn default_layout() -> DockLayout {
    DockLayout::split(
        DockSplit::Down,
        0.25,
        DockLayout::tabs(["Misc", "Logs"]),
        DockLayout::split(
            DockSplit::Left,
            0.25,
            DockLayout::tabs(["Command"]),
            DockLayout::split(
                DockSplit::Down,
                0.5,
                DockLayout::tabs(["Command 2"]),
                DockLayout::tabs(["Main View"]),
            ),
        ),
    )
}

fn alternative_layout() -> DockLayout {
    DockLayout::split(
        DockSplit::Left,
        0.30,
        DockLayout::tabs(["Misc", "Logs"]),
        DockLayout::split(
            DockSplit::Down,
            0.35,
            DockLayout::tabs(["Command"]),
            DockLayout::split(
                DockSplit::Right,
                0.5,
                DockLayout::tabs(["Command 2"]),
                DockLayout::tabs(["Main View"]),
            ),
        ),
    )
}

impl Application for DockDemoState {
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let ui = context.ui();
        let addons = context.addons();
        let state = self;
        // Global main menu bar
        if let Some(_mb) = ui.begin_main_menu_bar() {
            if let Some(_m) = ui.begin_menu("View") {
                if ui.menu_item("ImGui Demo") {
                    state.show_imgui_demo = !state.show_imgui_demo;
                }
                if ui.menu_item("Apply Default Layout") {
                    state.pending_layout = Some(DemoLayout::Default);
                }
                if ui.menu_item("Apply Alternative Layout") {
                    state.pending_layout = Some(DemoLayout::Alternative);
                }
                _m.end();
            }
            if let Some(_m) = ui.begin_menu("Windows") {
                ui.menu_item_toggle_no_shortcut("Main View", &mut state.show_main, true);
                ui.menu_item_toggle_no_shortcut("Command", &mut state.show_command, true);
                ui.menu_item_toggle_no_shortcut("Command 2", &mut state.show_command2, true);
                ui.menu_item_toggle_no_shortcut("Misc", &mut state.show_misc, true);
                ui.menu_item_toggle_no_shortcut("Logs", &mut state.show_logs, true);
                _m.end();
            }
            _mb.end();
        }

        // Fullscreen host window with dockspace
        let viewport = ui.main_viewport();
        ui.set_next_window_viewport(viewport.id());
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

        let dock_flags = addons.docking().flags();
        if dock_flags.contains(DockFlags::PASSTHRU_CENTRAL_NODE) {
            host_flags |= WindowFlags::NO_BACKGROUND;
        }

        // Zero rounding/border and remove padding for a clean host window
        let rounding = ui.push_style_var(StyleVar::WindowRounding(0.0));
        let border = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let requested_layout = state.pending_layout.take();
        let layout = match requested_layout.unwrap_or(DemoLayout::Default) {
            DemoLayout::Default => default_layout(),
            DemoLayout::Alternative => alternative_layout(),
        };
        let apply = if requested_layout.is_some() {
            DockLayoutApply::Replace
        } else {
            DockLayoutApply::IfMissing
        };
        let mut dockspace_result = Ok(Id::default());

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
                dockspace_result =
                    DockspaceTarget::new(dockspace_id, ui.cursor_screen_pos(), avail)
                        .map(|target| target.flags(dock_flags))
                        .and_then(|target| ui.dock_space_with_layout(&target, &layout, apply));

                // Optional: a small toolbar of docking flags toggles (update dear-app runtime flags)
                if let Some(_bar) = ui.begin_menu_bar() {
                    if let Some(_menu) = ui.begin_menu("Docking Flags") {
                        let mut new_flags = addons.docking().flags();
                        // Build flags from simple toggles (demo purpose)
                        let mut no_split = new_flags.contains(DockFlags::NO_DOCKING_SPLIT);
                        let mut no_resize = new_flags.contains(DockFlags::NO_RESIZE);
                        let mut auto_hide = new_flags.contains(DockFlags::AUTO_HIDE_TAB_BAR);
                        let mut no_central =
                            new_flags.contains(DockFlags::NO_DOCKING_OVER_CENTRAL_NODE);

                        if ui.menu_item_toggle_no_shortcut("NoSplit", &mut no_split, true) {
                            if no_split {
                                new_flags |= DockFlags::NO_DOCKING_SPLIT;
                            } else {
                                new_flags.remove(DockFlags::NO_DOCKING_SPLIT);
                            }
                        }
                        if ui.menu_item_toggle_no_shortcut("NoResize", &mut no_resize, true) {
                            if no_resize {
                                new_flags |= DockFlags::NO_RESIZE;
                            } else {
                                new_flags.remove(DockFlags::NO_RESIZE);
                            }
                        }
                        if ui.menu_item_toggle_no_shortcut("AutoHideTabBar", &mut auto_hide, true) {
                            if auto_hide {
                                new_flags |= DockFlags::AUTO_HIDE_TAB_BAR;
                            } else {
                                new_flags.remove(DockFlags::AUTO_HIDE_TAB_BAR);
                            }
                        }
                        if ui.menu_item_toggle_no_shortcut(
                            "NoDockingOverCentral",
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
                        addons.docking().set_flags(new_flags);
                        _menu.end();
                    }
                    _bar.end();
                }
            });
        dockspace_result.map_err(|error| RunError::application("frame", error.to_string()))?;

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
        Ok(())
    }
}

fn main() {
    dear_imgui_examples::init_tracing_with_filter(
        "dear_imgui=info,dear_app_docking=info,wgpu=warn",
    );

    let config = AppConfig {
        window_title: "Dear App Docking Demo".to_string(),
        window_size: (1280.0, 720.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.1, 0.1, 0.12, 1.0],
        wgpu: WgpuConfig::from_preset(WgpuPreset::HighPerformance),
        docking: DockingConfig::application_managed(),
        addons: AddOnsConfig::auto(),
        ini_filename: Some(std::path::PathBuf::from("Docking_Demo/docking_demo.ini")),
        restore_previous_geometry: true,
        redraw: RedrawMode::Poll,
        io_config_flags: None,
        theme: Some(dear_app::Theme::Dark),
    };

    run(config, DockDemoState::default()).unwrap();
}
