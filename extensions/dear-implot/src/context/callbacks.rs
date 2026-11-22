use super::PlotSetup;
use crate::{XAxis, YAxis, sys};
use std::any::Any;
use std::ffi::{c_char, c_int, c_void};

#[derive(Default)]
pub(super) struct AxisCallbacks {
    formatters: [Option<Box<dyn Any>>; YAxis::Y3 as usize + 1],
    transforms: [Option<Box<dyn Any>>; YAxis::Y3 as usize + 1],
}

impl AxisCallbacks {
    // EndPlot has returned. The plot is still in the owner context's pool and no other plot
    // can have begun between EndPlot and this call. Drop the Rust owners only after detaching.
    pub(super) unsafe fn detach(&self, raw: *mut sys::ImPlotPlot) {
        for (i, axis) in unsafe { &mut (*raw).Axes }.iter_mut().enumerate() {
            if self.formatters[i].is_some() {
                axis.Formatter = None;
                axis.FormatterData = std::ptr::null_mut();
            }
            if self.transforms[i].is_some() && !axis.TransformData.is_null() {
                axis.TransformForward = None;
                axis.TransformInverse = None;
                axis.TransformData = std::ptr::null_mut();
            }
        }
    }
}

impl PlotSetup<'_> {
    /// Format X-axis tick and mouse labels. The plot owns the callback through `EndPlot`.
    ///
    /// # Safety
    /// The callback runs inside native layout and drawing. It must not reenter Dear ImGui
    /// or ImPlot, including through thread-local state. A panic in a callback aborts.
    pub unsafe fn setup_x_axis_format_closure<F: Fn(f64) -> String + 'static>(
        &mut self,
        axis: XAxis,
        f: F,
    ) {
        self.formatter(axis as _, f);
    }

    /// Format Y-axis labels. See [`Self::setup_x_axis_format_closure`].
    ///
    /// # Safety
    /// The callback must not reenter Dear ImGui or ImPlot.
    pub unsafe fn setup_y_axis_format_closure<F: Fn(f64) -> String + 'static>(
        &mut self,
        axis: YAxis,
        f: F,
    ) {
        self.formatter(axis as _, f);
    }

    fn formatter<F: Fn(f64) -> String + 'static>(&mut self, axis: sys::ImAxis, f: F) {
        let holder = Box::new(f);
        let data = (&*holder as *const F).cast_mut().cast();
        self.plot.with_axes(&[axis], || unsafe {
            sys::ImPlot_SetupAxisFormat_PlotFormatter(axis, Some(formatter_thunk::<F>), data);
        });
        drop(self.callbacks.formatters[axis as usize].replace(holder));
    }

    /// Register forward and inverse transforms for an X axis until `EndPlot` returns.
    ///
    /// # Safety
    /// Both callbacks must not reenter Dear ImGui or ImPlot. Supply finite, mutually inverse
    /// transforms over the plotted domain. Native layout converts their results to pixels.
    /// A panic in a callback aborts.
    pub unsafe fn setup_x_axis_transform_closure<F, I>(
        &mut self,
        axis: XAxis,
        forward: F,
        inverse: I,
    ) where
        F: Fn(f64) -> f64 + 'static,
        I: Fn(f64) -> f64 + 'static,
    {
        self.transform(axis as _, forward, inverse);
    }

    /// Register transforms for a Y axis. See [`Self::setup_x_axis_transform_closure`].
    ///
    /// # Safety
    /// Both callbacks must not reenter Dear ImGui or ImPlot and must provide finite,
    /// mutually inverse transforms over the plotted domain.
    pub unsafe fn setup_y_axis_transform_closure<F, I>(
        &mut self,
        axis: YAxis,
        forward: F,
        inverse: I,
    ) where
        F: Fn(f64) -> f64 + 'static,
        I: Fn(f64) -> f64 + 'static,
    {
        self.transform(axis as _, forward, inverse);
    }

    fn transform<F, I>(&mut self, axis: sys::ImAxis, forward: F, inverse: I)
    where
        F: Fn(f64) -> f64 + 'static,
        I: Fn(f64) -> f64 + 'static,
    {
        let holder = Box::new((forward, inverse));
        let data = (&*holder as *const (F, I)).cast_mut().cast();
        self.plot.with_axes(&[axis], || unsafe {
            sys::ImPlot_SetupAxisScale_PlotTransform(
                axis,
                Some(forward_thunk::<F, I>),
                Some(inverse_thunk::<F, I>),
                data,
            );
        });
        drop(self.callbacks.transforms[axis as usize].replace(holder));
    }
}

// The registered C ABI thunks are specialized for their boxed owner. Rust aborts on panic at
// an extern "C" boundary, so an unwind cannot cross the native stack.
unsafe extern "C" fn formatter_thunk<F: Fn(f64) -> String>(
    value: f64,
    buffer: *mut c_char,
    size: c_int,
    data: *mut c_void,
) -> c_int {
    if size <= 0 || buffer.is_null() {
        return 0;
    }
    let text = unsafe { &*data.cast::<F>() }(value);
    let mut end = text.find('\0').unwrap_or(text.len()).min(size as usize - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast(), end);
        *buffer.add(end) = 0;
    }
    end as c_int
}

unsafe extern "C" fn forward_thunk<F: Fn(f64) -> f64, I>(value: f64, data: *mut c_void) -> f64 {
    let (forward, _) = unsafe { &*data.cast::<(F, I)>() };
    forward(value)
}

unsafe extern "C" fn inverse_thunk<F, I: Fn(f64) -> f64>(value: f64, data: *mut c_void) -> f64 {
    let (_, inverse) = unsafe { &*data.cast::<(F, I)>() };
    inverse(value)
}
