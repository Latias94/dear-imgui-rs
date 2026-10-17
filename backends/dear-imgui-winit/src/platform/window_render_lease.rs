use std::{cell::Cell, rc::Rc, sync::Arc};

use dear_imgui_rs::NativeWindowTarget;
use winit::window::Window;

/// Registration state shared with actual renderer work, independent of the Context lifetime.
#[derive(Debug)]
pub(crate) struct WindowRenderState {
    retired: Cell<bool>,
    pins: Cell<usize>,
}

impl WindowRenderState {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self {
            retired: Cell::new(false),
            pins: Cell::new(0),
        })
    }

    pub(crate) fn retire(&self) {
        self.retired.set(true);
    }

    pub(crate) fn is_pinned(&self) -> bool {
        self.pins.get() != 0
    }
}

#[derive(Debug)]
struct WindowPin {
    window: Arc<Window>,
    state: Rc<WindowRenderState>,
}

impl WindowPin {
    fn acquire(window: Arc<Window>, state: Rc<WindowRenderState>) -> Option<Self> {
        if state.retired.get() {
            return None;
        }
        state.pins.set(state.pins.get().checked_add(1)?);
        Some(Self { window, state })
    }
}

impl Drop for WindowPin {
    fn drop(&mut self) {
        self.state.pins.set(self.state.pins.get() - 1);
    }
}

#[derive(Debug)]
struct PinnedWindowChain {
    window: Option<WindowPin>,
    owners: Vec<WindowPin>,
}

impl Drop for PinnedWindowChain {
    fn drop(&mut self) {
        // Release the child before its nearest owner, then walk toward the root. Do not rely on
        // Vec's unspecified element destruction order for Win32's owner-destroys-owned behavior.
        drop(self.window.take());
        for owner in self.owners.drain(..) {
            drop(owner);
        }
    }
}

/// Keeps one exact Winit allocation and its native owner chain alive for recorded GPU work.
///
/// This UI-thread capability is separate from historical metadata. Retirement prevents new
/// lookups but does not destroy these windows until all copies are dropped. Native owner changes
/// are rejected while the affected window participates in a work lease. Submission must reject
/// retired work; already submitted work may still present or discard its retained acquired frame.
///
/// ```compile_fail
/// fn require_send<T: Send>() {}
/// require_send::<dear_imgui_winit::WinitWindowRenderLease>();
/// ```
///
/// ```compile_fail
/// fn require_sync<T: Sync>() {}
/// require_sync::<dear_imgui_winit::WinitWindowRenderLease>();
/// ```
#[derive(Clone, Debug)]
pub struct WinitWindowRenderLease {
    target: NativeWindowTarget,
    chain: Rc<PinnedWindowChain>,
}

impl WinitWindowRenderLease {
    pub(crate) fn new(
        target: NativeWindowTarget,
        window: Arc<Window>,
        state: Rc<WindowRenderState>,
        owners: Vec<(Arc<Window>, Rc<WindowRenderState>)>,
    ) -> Option<Self> {
        let window = WindowPin::acquire(window, state)?;
        let owners = owners
            .into_iter()
            .map(|(window, state)| WindowPin::acquire(window, state))
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            target,
            chain: Rc::new(PinnedWindowChain {
                window: Some(window),
                owners,
            }),
        })
    }

    /// Frozen native registration association captured when this lease was acquired.
    pub fn target(&self) -> NativeWindowTarget {
        self.target
    }

    /// Whether the target or an owner has retired since recording began.
    pub fn is_retired(&self) -> bool {
        self.chain
            .window
            .as_ref()
            .is_none_or(|window| window.state.retired.get())
            || self
                .chain
                .owners
                .iter()
                .any(|owner| owner.state.retired.get())
    }

    /// Borrows the exact allocation retained by this work capability.
    pub fn window(&self) -> &Window {
        &self
            .chain
            .window
            .as_ref()
            .expect("a live render lease retains its native window")
            .window
    }

    /// Retains the exact allocation for safe WGPU surface creation.
    ///
    /// Keep this render lease alongside that surface: the Arc alone does not freeze or retain
    /// the complete native owner chain.
    #[doc(hidden)]
    pub fn window_arc(&self) -> Arc<Window> {
        Arc::clone(
            &self
                .chain
                .window
                .as_ref()
                .expect("a live render lease retains its native window")
                .window,
        )
    }
}
