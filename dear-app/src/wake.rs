use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use winit::event_loop::EventLoopProxy;

use crate::runtime::RuntimeEvent;

/// Thread-safe, coalescing invalidation for a running application.
///
/// Obtain a handle from [`crate::InitializedContext::wake_handle`]. Asset loaders can request a
/// redraw after publishing their results, and animation or simulation code can request a deadline.
/// Repeated requests retain at most one queued wake event and the earliest pending deadline.
/// The handle contains no Dear ImGui or GPU objects and remains valid across GPU recovery.
#[derive(Clone, Debug)]
pub struct AppWakeHandle {
    proxy: EventLoopProxy<RuntimeEvent>,
    state: Arc<Mutex<WakeState>>,
}

/// A redraw request cannot reach an application whose event loop has closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("the application event loop has closed")]
pub struct AppWakeClosed;

#[derive(Debug, Default)]
struct WakeState {
    queued: bool,
    immediate: bool,
    deadline: Option<Instant>,
    closed: bool,
}

impl AppWakeHandle {
    pub(crate) fn new(proxy: EventLoopProxy<RuntimeEvent>) -> Self {
        Self {
            proxy,
            state: Arc::new(Mutex::new(WakeState::default())),
        }
    }

    /// Requests a frame as soon as the event loop can draw it.
    pub fn request_redraw(&self) -> Result<(), AppWakeClosed> {
        self.publish(|state| state.immediate = true)
    }

    /// Requests one frame at the event loop's first opportunity at this deadline.
    ///
    /// Multiple outstanding deadlines coalesce to the earliest. This does not create a recurring
    /// timer: request the next deadline after consuming the application's scheduled work.
    pub fn request_redraw_at(&self, deadline: Instant) -> Result<(), AppWakeClosed> {
        self.publish(|state| {
            state.deadline = Some(state.deadline.map_or(deadline, |old| old.min(deadline)));
        })
    }

    fn publish(&self, update: impl FnOnce(&mut WakeState)) -> Result<(), AppWakeClosed> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed {
            return Err(AppWakeClosed);
        }
        update(&mut state);
        if !state.queued {
            state.queued = true;
            if self.proxy.send_event(RuntimeEvent::Wake).is_err() {
                state.closed = true;
                return Err(AppWakeClosed);
            }
        }
        Ok(())
    }

    pub(crate) fn acknowledge(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .queued = false;
    }

    pub(crate) fn take_due(&self, now: Instant) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.take_due(now)
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .deadline
    }

    pub(crate) fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        state.immediate = false;
        state.deadline = None;
    }
}

impl WakeState {
    fn take_due(&mut self, now: Instant) -> bool {
        let immediate = std::mem::take(&mut self.immediate);
        let deadline_due = self.deadline.is_some_and(|deadline| deadline <= now);
        if deadline_due {
            self.deadline = None;
        }
        !self.closed && (immediate || deadline_due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn immediate_redraw_does_not_consume_a_future_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        let mut state = WakeState {
            immediate: true,
            deadline: Some(deadline),
            ..Default::default()
        };
        assert!(state.take_due(now));
        assert!(!state.take_due(now));
        assert_eq!(state.deadline, Some(deadline));
        assert!(state.take_due(deadline));
        assert!(!state.take_due(deadline));
        assert_eq!(state.deadline, None);
    }

    #[test]
    fn closed_schedules_never_request_a_frame() {
        let mut state = WakeState {
            closed: true,
            immediate: true,
            deadline: Some(Instant::now()),
            ..Default::default()
        };
        assert!(!state.take_due(Instant::now()));
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    #[ignore = "requires a native desktop event loop"]
    fn native_background_requests_coalesce_and_rearm() {
        use winit::{
            application::ApplicationHandler,
            event::WindowEvent,
            event_loop::{ActiveEventLoop, EventLoop},
            platform::pump_events::EventLoopExtPumpEvents,
            window::WindowId,
        };

        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AppWakeHandle>();
        let mut builder = EventLoop::<RuntimeEvent>::with_user_event();
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }
        #[cfg(target_os = "linux")]
        {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11().with_any_thread(true);
        }
        let mut event_loop = builder.build().unwrap();
        let handle = AppWakeHandle::new(event_loop.create_proxy());
        let now = Instant::now();
        let earliest = now + Duration::from_secs(1);
        let threads: Vec<_> = (0..8)
            .map(|offset| {
                let handle = handle.clone();
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        handle.request_redraw().unwrap();
                        handle
                            .request_redraw_at(earliest + Duration::from_secs(offset))
                            .unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(handle.deadline(), Some(earliest));
        assert!(handle.take_due(now));
        assert!(!handle.take_due(now));

        struct Probe {
            handle: AppWakeHandle,
            received: usize,
        }
        impl ApplicationHandler<RuntimeEvent> for Probe {
            fn resumed(&mut self, _: &ActiveEventLoop) {}
            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
            fn user_event(&mut self, _: &ActiveEventLoop, event: RuntimeEvent) {
                assert!(matches!(event, RuntimeEvent::Wake));
                self.received += 1;
                self.handle.acknowledge();
            }
        }
        let mut probe = Probe {
            handle: handle.clone(),
            received: 0,
        };
        for _ in 0..4 {
            event_loop.pump_app_events(Some(Duration::from_millis(20)), &mut probe);
        }
        assert_eq!(
            probe.received, 1,
            "the worker burst must queue only one wake"
        );
        handle.request_redraw().unwrap();
        event_loop.pump_app_events(Some(Duration::from_millis(20)), &mut probe);
        assert_eq!(
            probe.received, 2,
            "acknowledging a wake must allow a later request"
        );
        assert!(handle.take_due(now));
        assert!(handle.take_due(earliest));
        assert!(!handle.take_due(earliest));
        handle.close();
        assert_eq!(handle.request_redraw(), Err(AppWakeClosed));
        assert_eq!(handle.request_redraw_at(now), Err(AppWakeClosed));
    }
}
