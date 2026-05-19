// Debug-only: enforce BeginPlot/Setup/Plot call ordering
#[cfg(debug_assertions)]
thread_local! {
    static DEBUG_PLOT_STATE: PlotDebugState = PlotDebugState { in_plot: std::cell::Cell::new(false), setup_locked: std::cell::Cell::new(false) };
}

#[cfg(debug_assertions)]
struct PlotDebugState {
    in_plot: std::cell::Cell<bool>,
    setup_locked: std::cell::Cell<bool>,
}

#[cfg(debug_assertions)]
#[inline]
pub(crate) fn debug_begin_plot() {
    DEBUG_PLOT_STATE.with(|s| {
        s.in_plot.set(true);
        s.setup_locked.set(false);
    });
}

#[cfg(debug_assertions)]
#[inline]
pub(crate) fn debug_end_plot() {
    DEBUG_PLOT_STATE.with(|s| {
        s.in_plot.set(false);
        s.setup_locked.set(false);
    });
}

#[cfg(debug_assertions)]
#[inline]
pub(crate) fn debug_before_setup() {
    DEBUG_PLOT_STATE.with(|s| {
        debug_assert!(
            s.in_plot.get(),
            "Setup* called outside of BeginPlot/EndPlot"
        );
        debug_assert!(
            !s.setup_locked.get(),
            "Setup* must be called before any plotting (PlotX) or locking operations"
        );
    });
}

#[cfg(debug_assertions)]
#[inline]
pub(crate) fn debug_before_plot() {
    DEBUG_PLOT_STATE.with(|s| {
        debug_assert!(s.in_plot.get(), "Plot* called outside of BeginPlot/EndPlot");
        s.setup_locked.set(true);
    });
}

#[cfg(not(debug_assertions))]
#[inline]
pub(crate) fn debug_begin_plot() {}
#[cfg(not(debug_assertions))]
#[inline]
pub(crate) fn debug_end_plot() {}
#[cfg(not(debug_assertions))]
#[inline]
pub(crate) fn debug_before_setup() {}
#[cfg(not(debug_assertions))]
#[inline]
pub(crate) fn debug_before_plot() {}
