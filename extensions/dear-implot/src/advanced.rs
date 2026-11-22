//! Advanced plotting features for complex visualizations
//!
//! This module provides high-level functionality for creating complex plots
//! with multiple subplots, legends, and advanced layout management.

use crate::context::with_cleanup;
use crate::{AxisFlags, YAxis, plots::PlotError, sys};
use crate::{PlotCond, PlotScope, PlotSetup, PlotUi};
use std::ffi::CString;

fn validate_size(caller: &str, size: [f32; 2]) -> Result<(), PlotError> {
    if size[0].is_finite() && size[1].is_finite() {
        Ok(())
    } else {
        Err(PlotError::InvalidData(format!(
            "{caller} size must be finite"
        )))
    }
}

fn count_to_i32(caller: &str, name: &str, value: usize) -> Result<i32, PlotError> {
    if value == 0 {
        return Err(PlotError::InvalidData(format!(
            "{caller} {name} must be positive"
        )));
    }

    i32::try_from(value)
        .map_err(|_| PlotError::InvalidData(format!("{caller} {name} exceeded ImPlot's i32 range")))
}

fn validate_ratios(caller: &str, name: &str, ratios: &[f32]) -> Result<(), PlotError> {
    if ratios.iter().all(|value| value.is_finite() && *value > 0.0) {
        Ok(())
    } else {
        Err(PlotError::InvalidData(format!(
            "{caller} {name} must contain only positive finite values"
        )))
    }
}

fn validate_range(caller: &str, min: f64, max: f64) -> Result<(), PlotError> {
    if min.is_finite() && max.is_finite() && min != max {
        Ok(())
    } else {
        Err(PlotError::InvalidData(format!(
            "{caller} range values must be finite and distinct"
        )))
    }
}

/// Multi-plot layout manager for creating subplot grids
pub struct SubplotGrid<'a> {
    title: &'a str,
    rows: usize,
    cols: usize,
    size: Option<[f32; 2]>,
    flags: SubplotFlags,
    row_ratios: Option<Vec<f32>>,
    col_ratios: Option<Vec<f32>>,
}

bitflags::bitflags! {
    /// Flags for subplot configuration
    pub struct SubplotFlags: u32 {
        const NONE = sys::ImPlotSubplotFlags_None as u32;
        const NO_TITLE = sys::ImPlotSubplotFlags_NoTitle as u32;
        const NO_LEGEND = sys::ImPlotSubplotFlags_NoLegend as u32;
        const NO_MENUS = sys::ImPlotSubplotFlags_NoMenus as u32;
        const NO_RESIZE = sys::ImPlotSubplotFlags_NoResize as u32;
        const NO_ALIGN = sys::ImPlotSubplotFlags_NoAlign as u32;
        const SHARE_ITEMS = sys::ImPlotSubplotFlags_ShareItems as u32;
        const LINK_ROWS = sys::ImPlotSubplotFlags_LinkRows as u32;
        const LINK_COLS = sys::ImPlotSubplotFlags_LinkCols as u32;
        const LINK_ALL_X = sys::ImPlotSubplotFlags_LinkAllX as u32;
        const LINK_ALL_Y = sys::ImPlotSubplotFlags_LinkAllY as u32;
        const COLUMN_MAJOR = sys::ImPlotSubplotFlags_ColMajor as u32;
    }
}

impl<'a> SubplotGrid<'a> {
    /// Create a new subplot grid
    pub fn new(title: &'a str, rows: usize, cols: usize) -> Self {
        Self {
            title,
            rows,
            cols,
            size: None,
            flags: SubplotFlags::NONE,
            row_ratios: None,
            col_ratios: None,
        }
    }

    /// Set the size of the subplot grid
    pub fn with_size(mut self, size: [f32; 2]) -> Self {
        self.size = Some(size);
        self
    }

    /// Set subplot flags
    pub fn with_flags(mut self, flags: SubplotFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set row height ratios
    pub fn with_row_ratios(mut self, ratios: &[f32]) -> Self {
        self.row_ratios = if ratios.is_empty() {
            None
        } else {
            Some(ratios.to_vec())
        };
        self
    }

    /// Set column width ratios
    pub fn with_col_ratios(mut self, ratios: &[f32]) -> Self {
        self.col_ratios = if ratios.is_empty() {
            None
        } else {
            Some(ratios.to_vec())
        };
        self
    }

    /// Draw a grid. Use the supplied plotting UI for its cells and next-plot settings.
    pub fn build<R>(
        self,
        plot_ui: &PlotUi<'_>,
        draw: impl FnOnce(&PlotUi<'_>) -> R,
    ) -> Result<R, PlotError> {
        let rows = count_to_i32("SubplotGrid::build()", "rows", self.rows)?;
        let cols = count_to_i32("SubplotGrid::build()", "cols", self.cols)?;
        rows.checked_mul(cols).ok_or_else(|| {
            PlotError::InvalidData(
                "SubplotGrid::build() rows * cols exceeded ImPlot's i32 range".into(),
            )
        })?;
        let title_cstr =
            CString::new(self.title).map_err(|e| PlotError::StringConversion(e.to_string()))?;

        let size = self.size.unwrap_or([-1.0, -1.0]);
        validate_size("SubplotGrid::build()", size)?;
        let size_vec = sys::ImVec2_c {
            x: size[0],
            y: size[1],
        };

        // BeginSubplots reads and writes the ratios during this call.
        let mut row_ratios = self.row_ratios;
        let mut col_ratios = self.col_ratios;
        if let Some(row_ratios) = &row_ratios {
            if row_ratios.len() != self.rows {
                return Err(PlotError::InvalidData(format!(
                    "SubplotGrid::build() row_ratios length must equal rows ({})",
                    self.rows
                )));
            }
            validate_ratios("SubplotGrid::build()", "row_ratios", row_ratios)?;
        }
        if let Some(col_ratios) = &col_ratios {
            if col_ratios.len() != self.cols {
                return Err(PlotError::InvalidData(format!(
                    "SubplotGrid::build() col_ratios length must equal cols ({})",
                    self.cols
                )));
            }
            validate_ratios("SubplotGrid::build()", "col_ratios", col_ratios)?;
        }
        let row_ratios_ptr = row_ratios
            .as_mut()
            .map(|r| r.as_mut_ptr())
            .unwrap_or(std::ptr::null_mut());
        let col_ratios_ptr = col_ratios
            .as_mut()
            .map(|c| c.as_mut_ptr())
            .unwrap_or(std::ptr::null_mut());

        plot_ui.with_idle_context(|| ());
        plot_ui.ui.with_native_window_scope(|host| {
            let context = plot_ui.context;
            let started = context.with_bound_context("dear-implot: BeginSubplots", || unsafe {
                sys::ImPlot_BeginSubplots(
                    title_cstr.as_ptr(),
                    rows,
                    cols,
                    size_vec,
                    self.flags.bits() as _,
                    row_ratios_ptr,
                    col_ratios_ptr,
                )
            });
            if !started {
                return Err(PlotError::PlotCreationFailed(
                    "Failed to begin subplots".into(),
                ));
            }
            with_cleanup(
                || {
                    context.with_bound_context("dear-implot: EndSubplots", || unsafe {
                        sys::ImPlot_EndSubplots();
                        // An unfinished grid leaves next-cell links pointing into its row/column vectors.
                        for axis in [crate::Axis::X1, crate::Axis::Y1] {
                            sys::ImPlot_SetNextAxisLinks(
                                axis.to_sys(),
                                std::ptr::null_mut(),
                                std::ptr::null_mut(),
                            );
                        }
                    })
                },
                || {
                    Ok(host.run(|| {
                        draw(&PlotUi {
                            ui: plot_ui.ui,
                            context: plot_ui.context,
                            subplot: Some(host),
                        })
                    }))
                },
            )
        })
    }
}

/// Multi-axis plot support
pub struct MultiAxisPlot<'a> {
    title: &'a str,
    size: Option<[f32; 2]>,
    y_axes: Vec<YAxisConfig<'a>>,
}

/// Configuration for a Y-axis
pub struct YAxisConfig<'a> {
    pub label: Option<&'a str>,
    pub flags: AxisFlags,
    pub range: Option<(f64, f64)>,
}

impl<'a> MultiAxisPlot<'a> {
    /// Create a new multi-axis plot
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            size: None,
            y_axes: Vec::new(),
        }
    }

    /// Set the plot size
    pub fn with_size(mut self, size: [f32; 2]) -> Self {
        self.size = Some(size);
        self
    }

    /// Add a Y-axis
    pub fn add_y_axis(mut self, config: YAxisConfig<'a>) -> Self {
        self.y_axes.push(config);
        self
    }

    /// Set up the configured Y axes and draw a plot.
    pub fn build<R>(
        self,
        plot_ui: &PlotUi<'_>,
        draw: impl FnOnce(&PlotScope<'_>) -> R,
    ) -> Result<R, PlotError> {
        if self.title.contains('\0') {
            return Err(PlotError::StringConversion(
                "Plot title contained an interior NUL byte".into(),
            ));
        }
        for axis in &self.y_axes {
            if let Some(label) = axis.label
                && label.contains('\0')
            {
                return Err(PlotError::StringConversion(
                    "Axis label contained an interior NUL byte".to_string(),
                ));
            }
            if let Some((min, max)) = axis.range {
                validate_range("MultiAxisPlot::build()", min, max)?;
            }
        }
        if self.y_axes.len() > 3 {
            return Err(PlotError::InvalidData(
                "MultiAxisPlot::build() supports at most 3 Y axes".to_string(),
            ));
        }

        let size = self.size.unwrap_or([-1.0, -1.0]);
        validate_size("MultiAxisPlot::build()", size)?;
        plot_ui
            .with_plot_setup(
                self.title,
                size,
                |setup| {
                    for (axis, config) in [YAxis::Y1, YAxis::Y2, YAxis::Y3]
                        .into_iter()
                        .zip(self.y_axes)
                    {
                        setup.setup_y_axis(axis, config.label, config.flags);
                        if let Some((min, max)) = config.range {
                            setup.setup_y_axis_limits(axis, min, max, PlotCond::None);
                        }
                    }
                },
                draw,
            )
            .ok_or_else(|| PlotError::PlotCreationFailed("Failed to begin multi-axis plot".into()))
    }
}

/// Plot legend configuration and popup contents.
pub struct LegendManager;

impl LegendManager {
    /// Configure the legend during plot setup.
    pub fn setup(setup: &PlotSetup<'_>, location: LegendLocation, flags: LegendFlags) {
        setup.plot.with_bound_context(|| unsafe {
            sys::ImPlot_SetupLegend(location as _, flags.bits() as _)
        });
    }

    /// Draw the ImGui contents of an open legend entry popup.
    pub fn with_popup<R>(plot: &PlotScope<'_>, label: &str, draw: impl FnOnce() -> R) -> Option<R> {
        let label = CString::new(label).ok()?;
        plot.host.run(|| {
            let open = plot
                .with_bound_context(|| unsafe { sys::ImPlot_BeginLegendPopup(label.as_ptr(), 1) });
            open.then(|| {
                with_cleanup(
                    || {
                        plot.context
                            .with_bound_context("dear-implot: EndLegendPopup", || unsafe {
                                sys::ImPlot_EndLegendPopup()
                            })
                    },
                    // BeginLegendPopup switched ImGui to the popup window. Protect that host until its contents end.
                    || plot.ui.with_native_window_scope(|popup| popup.run(draw)),
                )
            })
        })
    }
}

impl PlotUi<'_> {
    /// Configure the shared legend between subplot cells.
    pub fn setup_subplot_legend(&self, location: LegendLocation, flags: LegendFlags) {
        assert!(
            self.subplot.is_some(),
            "subplot legend setup requires a grid"
        );
        self.with_bound_context(|| unsafe {
            sys::ImPlot_SetupLegend(location as _, flags.bits() as _)
        });
    }
}

/// Legend location options (ImPlotLocation)
#[repr(i32)]
pub enum LegendLocation {
    Center = sys::ImPlotLocation_Center as i32,
    North = sys::ImPlotLocation_North as i32,
    South = sys::ImPlotLocation_South as i32,
    West = sys::ImPlotLocation_West as i32,
    East = sys::ImPlotLocation_East as i32,
    NorthWest = sys::ImPlotLocation_NorthWest as i32,
    NorthEast = sys::ImPlotLocation_NorthEast as i32,
    SouthWest = sys::ImPlotLocation_SouthWest as i32,
    SouthEast = sys::ImPlotLocation_SouthEast as i32,
}

bitflags::bitflags! {
    /// Flags for legend configuration (ImPlotLegendFlags)
    pub struct LegendFlags: u32 {
        const NONE              = sys::ImPlotLegendFlags_None as u32;
        const NO_BUTTONS        = sys::ImPlotLegendFlags_NoButtons as u32;
        const NO_HIGHLIGHT_ITEM = sys::ImPlotLegendFlags_NoHighlightItem as u32;
        const NO_HIGHLIGHT_AXIS = sys::ImPlotLegendFlags_NoHighlightAxis as u32;
        const NO_MENUS          = sys::ImPlotLegendFlags_NoMenus as u32;
        const OUTSIDE           = sys::ImPlotLegendFlags_Outside as u32;
        const HORIZONTAL        = sys::ImPlotLegendFlags_Horizontal as u32;
        const SORT              = sys::ImPlotLegendFlags_Sort as u32;
        const REVERSE           = sys::ImPlotLegendFlags_Reverse as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::{PlotError, SubplotFlags, SubplotGrid};
    use crate::{PlotContext, sys};
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn setup_context() -> (dear_imgui_rs::Context, PlotContext) {
        let mut imgui = dear_imgui_rs::Context::create();
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
        imgui.io_mut().set_display_size([256.0, 256.0]);
        imgui.io_mut().set_delta_time(1.0 / 60.0);
        let plot = PlotContext::create(&imgui);
        (imgui, plot)
    }

    fn invalid_data_message(err: PlotError) -> String {
        match err {
            PlotError::InvalidData(message) => message,
            other => panic!("expected invalid data error, got {other:?}"),
        }
    }

    fn expect_invalid_data(result: Result<(), PlotError>) -> String {
        match result {
            Err(err) => invalid_data_message(err),
            Ok(_) => panic!("expected SubplotGrid::build() to reject invalid input"),
        }
    }

    #[test]
    fn subplot_flags_match_the_upstream_contract() {
        let cases = [
            (SubplotFlags::NONE, sys::ImPlotSubplotFlags_None),
            (SubplotFlags::NO_TITLE, sys::ImPlotSubplotFlags_NoTitle),
            (SubplotFlags::NO_LEGEND, sys::ImPlotSubplotFlags_NoLegend),
            (SubplotFlags::NO_MENUS, sys::ImPlotSubplotFlags_NoMenus),
            (SubplotFlags::NO_RESIZE, sys::ImPlotSubplotFlags_NoResize),
            (SubplotFlags::NO_ALIGN, sys::ImPlotSubplotFlags_NoAlign),
            (
                SubplotFlags::SHARE_ITEMS,
                sys::ImPlotSubplotFlags_ShareItems,
            ),
            (SubplotFlags::LINK_ROWS, sys::ImPlotSubplotFlags_LinkRows),
            (SubplotFlags::LINK_COLS, sys::ImPlotSubplotFlags_LinkCols),
            (SubplotFlags::LINK_ALL_X, sys::ImPlotSubplotFlags_LinkAllX),
            (SubplotFlags::LINK_ALL_Y, sys::ImPlotSubplotFlags_LinkAllY),
            (SubplotFlags::COLUMN_MAJOR, sys::ImPlotSubplotFlags_ColMajor),
        ];
        for (flags, expected) in cases {
            assert_eq!(flags.bits(), expected as u32);
        }
    }

    #[test]
    fn subplot_grid_forwards_resize_link_and_layout_flags() {
        let _guard = test_guard();
        let (mut imgui, plot) = setup_context();
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        let flags = SubplotFlags::NO_RESIZE | SubplotFlags::LINK_ROWS | SubplotFlags::COLUMN_MAJOR;
        SubplotGrid::new("flag forwarding", 1, 1)
            .with_size([200.0, 180.0])
            .with_flags(flags)
            .build(&plot_ui, |grid| {
                grid.with_bound_context(|| unsafe {
                    let subplot = (*sys::ImPlot_GetCurrentContext()).CurrentSubplot;
                    assert!(!subplot.is_null());
                    assert_eq!(
                        (*subplot).Flags,
                        (sys::ImPlotSubplotFlags_NoResize
                            | sys::ImPlotSubplotFlags_LinkRows
                            | sys::ImPlotSubplotFlags_ColMajor)
                            as sys::ImPlotSubplotFlags
                    );
                })
            })
            .expect("subplot should be visible");
    }

    #[test]
    fn subplot_grid_rejects_invalid_counts_before_ffi() {
        let _guard = test_guard();
        let (mut imgui, _plot) = setup_context();
        let frame = imgui.begin_frame();
        let plot_ui = _plot.get_plot_ui(frame.ui());

        let rows = expect_invalid_data(SubplotGrid::new("bad_rows", 0, 1).build(&plot_ui, |_| ()));
        assert!(rows.contains("rows must be positive"));

        let cols = expect_invalid_data(SubplotGrid::new("bad_cols", 1, 0).build(&plot_ui, |_| ()));
        assert!(cols.contains("cols must be positive"));

        let overflow = expect_invalid_data(
            SubplotGrid::new("too_many_rows", i32::MAX as usize + 1, 1).build(&plot_ui, |_| ()),
        );
        assert!(overflow.contains("rows exceeded"));

        let cells = expect_invalid_data(
            SubplotGrid::new("too_many_cells", 65_536, 32_768).build(&plot_ui, |_| ()),
        );
        assert!(cells.contains("rows * cols exceeded"));
    }

    #[test]
    fn subplot_grid_ratio_lengths_follow_usize_counts() {
        let _guard = test_guard();
        let (mut imgui, _plot) = setup_context();
        let frame = imgui.begin_frame();
        let plot_ui = _plot.get_plot_ui(frame.ui());

        let err = expect_invalid_data(
            SubplotGrid::new("bad_ratios", 2usize, 1usize)
                .with_row_ratios(&[1.0])
                .build(&plot_ui, |_| ()),
        );

        assert!(err.contains("row_ratios length must equal rows (2)"));
    }

    #[test]
    fn unfinished_grid_clears_cell_links_and_balances_subplot_styles() {
        let _guard = test_guard();
        let (mut imgui, owner) = setup_context();
        let frame = imgui.begin_frame();
        let plots = owner.get_plot_ui(frame.ui());
        plots.with_style_var_f32(crate::StyleVar::MinorAlpha, 0.25, || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                SubplotGrid::new("linked grid", 1, 2)
                    .with_flags(SubplotFlags::LINK_ALL_X | SubplotFlags::LINK_ALL_Y)
                    .with_size([200.0, 140.0])
                    .build(&plots, |grid| {
                        grid.with_plot("cell", |plot| {
                            plot.plot_line("samples", &[0.0, 1.0], &[1.0, 2.0])
                        })
                        .unwrap();
                        grid.with_bound_context(|| unsafe {
                            let pending = &(*owner.raw()).NextPlotData;
                            assert!(!pending.LinkedMin[crate::Axis::X1 as usize].is_null());
                        });
                        panic!("leave before opening the second cell");
                    })
            }));
            assert!(result.is_err());
            plots.with_bound_context(|| unsafe {
                let ctx = &*owner.raw();
                assert_eq!(ctx.StyleModifiers.Size, 1);
                assert_eq!(ctx.ColorModifiers.Size, 0);
                assert!(ctx.CurrentSubplot.is_null());
                for axis in [crate::Axis::X1, crate::Axis::Y1] {
                    assert!(ctx.NextPlotData.LinkedMin[axis as usize].is_null());
                    assert!(ctx.NextPlotData.LinkedMax[axis as usize].is_null());
                }
            });
        });
    }
}
