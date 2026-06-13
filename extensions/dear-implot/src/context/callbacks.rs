use super::ui::PlotUi;
use crate::{XAxis, YAxis, sys};
use std::{cell::RefCell, rc::Rc};

impl<'ui> PlotUi<'ui> {
    // -------- Formatter (closure) --------
    /// Setup tick label formatter using a Rust closure.
    ///
    /// The closure is kept alive until the current plot ends.
    pub fn setup_x_axis_format_closure<F>(&self, axis: XAxis, f: F) -> AxisFormatterToken
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        let _guard = self.bind();
        AxisFormatterToken::new(axis as sys::ImAxis, f)
    }

    /// Setup tick label formatter using a Rust closure.
    ///
    /// The closure is kept alive until the current plot ends.
    pub fn setup_y_axis_format_closure<F>(&self, axis: YAxis, f: F) -> AxisFormatterToken
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        let _guard = self.bind();
        AxisFormatterToken::new(axis as sys::ImAxis, f)
    }

    // -------- Transform (closure) --------
    /// Setup custom axis transform using Rust closures (forward/inverse).
    ///
    /// The closures are kept alive until the current plot ends.
    pub fn setup_x_axis_transform_closure<FW, INV>(
        &self,
        axis: XAxis,
        forward: FW,
        inverse: INV,
    ) -> AxisTransformToken
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        let _guard = self.bind();
        AxisTransformToken::new(axis as sys::ImAxis, forward, inverse)
    }

    /// Setup custom axis transform for Y axis using closures
    pub fn setup_y_axis_transform_closure<FW, INV>(
        &self,
        axis: YAxis,
        forward: FW,
        inverse: INV,
    ) -> AxisTransformToken
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        let _guard = self.bind();
        AxisTransformToken::new(axis as sys::ImAxis, forward, inverse)
    }
}

// Plot-scope callback storage -------------------------------------------------
//
// ImPlot's axis formatter/transform APIs take function pointers + `user_data`
// pointers, and may call them at any point until the current plot ends.
//
// Returning a standalone token that owns the closure is unsound: safe Rust code
// could drop the token early, leaving ImPlot with a dangling `user_data` pointer.
//
// To keep the safe API sound without forcing users to manually retain tokens,
// we store callback holders in thread-local, plot-scoped storage that is
// created when a plot begins and destroyed when the plot ends.

#[derive(Default)]
struct PlotScopeStorage {
    formatters: Vec<Box<FormatterHolder>>,
    transforms: Vec<Box<TransformHolder>>,
}

thread_local! {
    static PLOT_SCOPE_STACK: RefCell<Vec<PlotScopeStorage>> = const { RefCell::new(Vec::new()) };
}

fn with_plot_scope_storage<T>(f: impl FnOnce(&mut PlotScopeStorage) -> T) -> Option<T> {
    PLOT_SCOPE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        stack.last_mut().map(f)
    })
}

pub(crate) struct PlotScopeGuard {
    _not_send_or_sync: std::marker::PhantomData<Rc<()>>,
}

impl PlotScopeGuard {
    pub(crate) fn new() -> Self {
        PLOT_SCOPE_STACK.with(|stack| stack.borrow_mut().push(PlotScopeStorage::default()));
        Self {
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for PlotScopeGuard {
    fn drop(&mut self) {
        PLOT_SCOPE_STACK.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert!(popped.is_some(), "dear-implot: plot scope stack underflow");
        });
    }
}

// =================== Formatter bridge ===================

struct FormatterHolder {
    func: Box<dyn Fn(f64) -> String + Send + Sync + 'static>,
}

#[must_use]
pub struct AxisFormatterToken {
    _private: (),
    _not_send_or_sync: std::marker::PhantomData<Rc<()>>,
}

impl AxisFormatterToken {
    fn new<F>(axis: sys::ImAxis, f: F) -> Self
    where
        F: Fn(f64) -> String + Send + Sync + 'static,
    {
        let configured = with_plot_scope_storage(|storage| {
            let holder = Box::new(FormatterHolder { func: Box::new(f) });
            let user = &*holder as *const FormatterHolder as *mut std::os::raw::c_void;
            storage.formatters.push(holder);
            unsafe {
                sys::ImPlot_SetupAxisFormat_PlotFormatter(
                    axis as sys::ImAxis,
                    Some(formatter_thunk),
                    user,
                )
            }
        })
        .is_some();

        debug_assert!(
            configured,
            "dear-implot: axis formatter closure must be set within an active plot"
        );

        Self {
            _private: (),
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for AxisFormatterToken {
    fn drop(&mut self) {
        // The actual callback lifetime is managed by PlotScopeGuard.
    }
}

unsafe extern "C" fn formatter_thunk(
    value: f64,
    buff: *mut std::os::raw::c_char,
    size: std::os::raw::c_int,
    user_data: *mut std::os::raw::c_void,
) -> std::os::raw::c_int {
    if user_data.is_null() || buff.is_null() || size <= 0 {
        return 0;
    }
    // Safety: ImPlot passes back the same pointer we provided in `AxisFormatterToken::new`.
    let holder = unsafe { &*(user_data as *const FormatterHolder) };
    let s = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (holder.func)(value))) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-implot: panic in axis formatter callback");
            std::process::abort();
        }
    };
    let bytes = s.as_bytes();
    let max = (size - 1).max(0) as usize;
    let n = bytes.len().min(max);

    // Safety: `buff` is assumed to point to a valid buffer of at least `size`
    // bytes, with space for a terminating null. This matches ImPlot's
    // formatter contract.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buff as *mut u8, n);
        *buff.add(n) = 0;
    }
    n as std::os::raw::c_int
}

// =================== Transform bridge ===================

struct TransformHolder {
    forward: Box<dyn Fn(f64) -> f64 + Send + Sync + 'static>,
    inverse: Box<dyn Fn(f64) -> f64 + Send + Sync + 'static>,
}

#[must_use]
pub struct AxisTransformToken {
    _private: (),
    _not_send_or_sync: std::marker::PhantomData<Rc<()>>,
}

impl AxisTransformToken {
    fn new<FW, INV>(axis: sys::ImAxis, forward: FW, inverse: INV) -> Self
    where
        FW: Fn(f64) -> f64 + Send + Sync + 'static,
        INV: Fn(f64) -> f64 + Send + Sync + 'static,
    {
        let configured = with_plot_scope_storage(|storage| {
            let holder = Box::new(TransformHolder {
                forward: Box::new(forward),
                inverse: Box::new(inverse),
            });
            let user = &*holder as *const TransformHolder as *mut std::os::raw::c_void;
            storage.transforms.push(holder);
            unsafe {
                sys::ImPlot_SetupAxisScale_PlotTransform(
                    axis as sys::ImAxis,
                    Some(transform_forward_thunk),
                    Some(transform_inverse_thunk),
                    user,
                )
            }
        })
        .is_some();

        debug_assert!(
            configured,
            "dear-implot: axis transform closure must be set within an active plot"
        );

        Self {
            _private: (),
            _not_send_or_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for AxisTransformToken {
    fn drop(&mut self) {
        // The actual callback lifetime is managed by PlotScopeGuard.
    }
}

unsafe extern "C" fn transform_forward_thunk(
    value: f64,
    user_data: *mut std::os::raw::c_void,
) -> f64 {
    if user_data.is_null() {
        return value;
    }
    let holder = unsafe { &*(user_data as *const TransformHolder) };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (holder.forward)(value))) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-implot: panic in axis transform (forward) callback");
            std::process::abort();
        }
    }
}

unsafe extern "C" fn transform_inverse_thunk(
    value: f64,
    user_data: *mut std::os::raw::c_void,
) -> f64 {
    if user_data.is_null() {
        return value;
    }
    let holder = unsafe { &*(user_data as *const TransformHolder) };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (holder.inverse)(value))) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("dear-implot: panic in axis transform (inverse) callback");
            std::process::abort();
        }
    }
}
