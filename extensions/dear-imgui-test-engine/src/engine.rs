#[cfg(feature = "capture")]
use std::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;

use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole, ContextBinding,
    ContextId, Ui, with_scratch_txt, with_scratch_txt_two,
};
use dear_imgui_test_engine_sys as sys;

use crate::attachment::{AttachmentControl, TestEngineAttachmentMarker};
use crate::error::ffi_status;
use crate::{
    AttachmentState, CaptureOutput, FrameDriverError, ResultSummary, RunFlags, RunSpeed, RunState,
    Script, ScriptTest, TestEngineError, TestEngineResult, TestFrameDriver, TestGroup,
    VerboseLevel,
};

/// Dear ImGui Test Engine context with one transactional Context attachment.
///
/// The upstream engine is not thread-safe. Create, attach, and use it on the owning ImGui thread.
pub struct TestEngine {
    control: Rc<AttachmentControl>,
    lease: Option<ContextAttachmentLease>,
    _not_send_sync: PhantomData<Rc<()>>,
}

struct ActivePresentation<'engine> {
    engine: &'engine mut TestEngine,
    active: bool,
}

struct PostSwapFailure {
    source: TestEngineError,
    presentation_completed: bool,
}

#[cfg(feature = "capture")]
pub(crate) struct CaptureProviderGuard {
    engine: *mut sys::ImGuiTestEngine,
    user_data: *mut c_void,
    active: bool,
}

#[cfg(feature = "capture")]
impl CaptureProviderGuard {
    pub(crate) fn clear(&mut self) -> TestEngineResult<()> {
        if !self.active {
            return Ok(());
        }
        let status =
            unsafe { sys::imgui_test_engine_clear_capture_provider(self.engine, self.user_data) };
        ffi_status("imgui_test_engine_clear_capture_provider", status)?;
        self.active = false;
        Ok(())
    }
}

#[cfg(feature = "capture")]
impl Drop for CaptureProviderGuard {
    fn drop(&mut self) {
        let _ = self.clear();
    }
}

impl<'engine> ActivePresentation<'engine> {
    fn begin(engine: &'engine mut TestEngine) -> TestEngineResult<Self> {
        engine.pre_swap_internal()?;
        Ok(Self {
            engine,
            active: true,
        })
    }

    fn abort_once(&mut self) -> TestEngineResult<()> {
        if !self.active {
            return Ok(());
        }
        self.engine.abort_presentation_internal()?;
        self.active = false;
        Ok(())
    }

    fn abort(mut self) -> Option<TestEngineError> {
        match self.abort_once() {
            Ok(()) => None,
            Err(_) => match self.abort_once() {
                Ok(()) => None,
                Err(_) => match self.engine.stop() {
                    Ok(()) => {
                        self.active = false;
                        None
                    }
                    Err(source) => Some(source),
                },
            },
        }
    }

    fn complete(mut self) -> Result<(), (TestEngineError, Option<TestEngineError>)> {
        match self.engine.post_swap_hook_internal() {
            Ok(()) => {
                self.active = false;
                self.engine
                    .refresh_run_state()
                    .map_err(|source| (source, None))
            }
            Err(failure) => {
                if failure.presentation_completed {
                    self.active = false;
                    Err((failure.source, None))
                } else {
                    let abort_error = self.abort();
                    Err((failure.source, abort_error))
                }
            }
        }
    }
}

impl Drop for ActivePresentation<'_> {
    fn drop(&mut self) {
        if self.abort_once().is_err() {
            let _ = self.engine.stop();
            self.active = false;
        }
    }
}

impl TestEngine {
    /// Creates a detached native Test Engine.
    pub fn create() -> TestEngineResult<Self> {
        let mut raw = std::ptr::null_mut();
        let status = unsafe { sys::imgui_test_engine_create_context(&mut raw) };
        ffi_status("imgui_test_engine_create_context", status)?;
        if raw.is_null() {
            return Err(TestEngineError::InvalidNativeData {
                operation: "imgui_test_engine_create_context",
                detail: "successful creation returned a null engine",
            });
        }
        Ok(Self {
            control: Rc::new(AttachmentControl::new(raw)),
            lease: None,
            _not_send_sync: PhantomData,
        })
    }

    /// Returns the raw Test Engine pointer for explicit unsafe interop.
    pub fn as_raw(&self) -> *mut sys::ImGuiTestEngine {
        self.control.raw()
    }

    /// Returns the Rust attachment lifecycle without calling native code.
    pub fn attachment_state(&self) -> AttachmentState {
        self.control.attachment_state()
    }

    /// Returns the independently tracked test run lifecycle.
    pub fn run_state(&self) -> RunState {
        self.control.run_state()
    }

    pub(crate) fn attached_context_id(&self) -> Option<ContextId> {
        self.control.context_id()
    }

    /// Transactionally attaches and starts the engine on one Context.
    ///
    /// The unique extension slot is reserved before native start. Any native or binding failure
    /// rolls that reservation back so another engine may attach.
    pub fn start(&mut self, context: &mut Context) -> TestEngineResult<()> {
        const OPERATION: &str = "TestEngine::start";
        if self.control.attachment_state() != AttachmentState::Detached
            || self.control.raw().is_null()
        {
            return Err(self.state_error(OPERATION, "start requires a live detached Test Engine"));
        }

        let attachment: Rc<dyn ContextAttachment> = self.control.clone();
        let mut lease = context
            .register_attachment::<TestEngineAttachmentMarker>(
                ContextAttachmentRole::Extension,
                attachment,
            )
            .map_err(|source| TestEngineError::Attachment {
                operation: OPERATION,
                source,
            })?;

        let binding = context.binding();
        let context_raw = context.as_raw();
        self.control.reserve(binding.clone());
        let start_result = binding
            .try_with_bound_context(|| unsafe {
                sys::imgui_test_engine_start(self.control.raw(), context_raw)
            })
            .map_err(|source| TestEngineError::ContextBinding {
                operation: OPERATION,
                source,
            })
            .and_then(|status| ffi_status("imgui_test_engine_start", status));

        if let Err(error) = start_result {
            match lease.detach() {
                Ok(true) => {}
                Ok(false) => {
                    self.lease = Some(lease);
                    return Err(TestEngineError::AttachmentInvariant {
                        operation: OPERATION,
                        detail: "start rollback found an inactive lease while Context was exclusively borrowed",
                    });
                }
                Err(source) => {
                    self.lease = Some(lease);
                    return Err(TestEngineError::AttachmentDetach {
                        operation: OPERATION,
                        source,
                    });
                }
            }
            self.control.rollback_start();
            return Err(error);
        }

        self.control.commit_start();
        self.lease = Some(lease);
        Ok(())
    }

    /// Stops, unbinds, unregisters, and destroys the native engine.
    ///
    /// This operation is idempotent from every completed state. A failure leaves enough state for
    /// a later call or `Drop` to retry without double-unbinding. Call this explicitly when cleanup
    /// failures must be observed; `Drop` is non-panicking and can only perform best-effort retries.
    pub fn shutdown(&mut self) -> TestEngineResult<()> {
        match self.control.attachment_state() {
            AttachmentState::Destroyed => return Ok(()),
            AttachmentState::Reserved => {
                match self.lease.as_mut().map(ContextAttachmentLease::detach) {
                    Some(Ok(true)) => {}
                    Some(Err(source)) => {
                        return Err(TestEngineError::AttachmentDetach {
                            operation: "TestEngine::shutdown",
                            source,
                        });
                    }
                    None | Some(Ok(false)) => {
                        return Err(TestEngineError::AttachmentInvariant {
                            operation: "TestEngine::shutdown",
                            detail: "reserved attachment lease was not active; Context teardown must synchronize state first",
                        });
                    }
                }
                self.lease = None;
                self.control.rollback_start();
            }
            AttachmentState::Attached => self.detach_from_live_context()?,
            AttachmentState::ContextDropping => {
                return Err(self.state_error(
                    "TestEngine::shutdown",
                    "Context pre-native teardown is currently executing",
                ));
            }
            AttachmentState::Detached | AttachmentState::ContextDestroyed => {}
        }

        self.destroy_detached()?;
        if let Some(error) = self.control.take_teardown_error() {
            return Err(error);
        }
        Ok(())
    }

    /// Returns whether the engine is still attached according to Rust lifecycle state.
    pub fn is_bound(&self) -> bool {
        matches!(
            self.control.attachment_state(),
            AttachmentState::Reserved
                | AttachmentState::Attached
                | AttachmentState::ContextDropping
        )
    }

    /// Queries whether the native engine remains started.
    pub fn is_started(&self) -> TestEngineResult<bool> {
        if self.control.attachment_state() != AttachmentState::Attached {
            return Ok(false);
        }
        self.query_bool("imgui_test_engine_is_started", |raw, output| unsafe {
            sys::imgui_test_engine_is_started(raw, output)
        })
    }

    /// Stops the native coroutine while keeping its upstream Context hooks installed.
    pub fn stop(&mut self) -> TestEngineResult<()> {
        self.require_started("TestEngine::stop")?;
        self.call_attached("imgui_test_engine_stop", |raw| unsafe {
            sys::imgui_test_engine_stop(raw)
        })?;
        self.control.set_run_state(RunState::Inactive);
        Ok(())
    }

    /// Renders and presents one frame through a single ordered backend driver.
    ///
    /// Consuming [`RenderedFrame`] proves Dear ImGui rendering has completed and prevents the same
    /// render lease from being driven twice. The attached Context is checked before the driver sees
    /// the frame. Presentation is then bracketed as `pre-swap -> present -> post-swap`.
    ///
    /// If presentation fails or panics, an abort path releases native capture state without
    /// pretending that the surface was presented.
    pub fn drive_frame<Driver>(
        &mut self,
        frame: RenderedFrame<'_>,
        frame_index: u64,
        driver: &mut Driver,
    ) -> Result<(), FrameDriverError<Driver::RenderError, Driver::PresentError>>
    where
        Driver: TestFrameDriver,
    {
        self.require_started("TestEngine::drive_frame")
            .map_err(FrameDriverError::Context)?;
        let expected = self.attached_context_id().ok_or_else(|| {
            FrameDriverError::Context(self.state_error(
                "TestEngine::drive_frame",
                "frame driving requires a live Test Engine attachment",
            ))
        })?;
        let actual = frame.context_id();
        if actual != expected {
            return Err(FrameDriverError::Context(
                TestEngineError::ContextMismatch {
                    operation: "TestEngine::drive_frame",
                    expected,
                    actual,
                },
            ));
        }

        let reconciled = driver
            .render(frame, frame_index)
            .map_err(FrameDriverError::Render)?;
        if reconciled.context_id() != expected {
            return Err(FrameDriverError::Context(
                TestEngineError::ContextMismatch {
                    operation: "TestEngine::drive_frame render completion",
                    expected,
                    actual: reconciled.context_id(),
                },
            ));
        }
        drop(reconciled);
        let presentation = ActivePresentation::begin(self).map_err(FrameDriverError::PreSwap)?;
        if let Err(source) = driver.present(frame_index) {
            return Err(FrameDriverError::Present {
                source,
                abort_error: presentation.abort(),
            });
        }
        presentation
            .complete()
            .map_err(|(source, abort_error)| FrameDriverError::PostSwap {
                source,
                abort_error,
            })
    }

    fn pre_swap_internal(&mut self) -> TestEngineResult<()> {
        self.require_started("TestEngine::pre_swap")?;
        self.call_attached("imgui_test_engine_pre_swap", |raw| unsafe {
            sys::imgui_test_engine_pre_swap(raw)
        })
    }

    fn post_swap_hook_internal(&mut self) -> Result<(), PostSwapFailure> {
        self.require_started("TestEngine::post_swap")
            .map_err(|source| PostSwapFailure {
                source,
                presentation_completed: false,
            })?;
        let mut presentation_completed = false;
        self.call_attached("imgui_test_engine_post_swap", |raw| unsafe {
            sys::imgui_test_engine_post_swap(raw, &mut presentation_completed)
        })
        .map_err(|source| PostSwapFailure {
            source,
            presentation_completed,
        })
    }

    fn abort_presentation_internal(&mut self) -> TestEngineResult<()> {
        self.require_started("TestEngine::abort_presentation")?;
        self.call_attached("imgui_test_engine_abort_presentation", |raw| unsafe {
            sys::imgui_test_engine_abort_presentation(raw)
        })
    }

    #[cfg(feature = "capture")]
    pub(crate) fn install_capture_provider(
        &mut self,
        callback: sys::ImGuiTestEngineCaptureCallback_c,
        user_data: *mut c_void,
    ) -> TestEngineResult<CaptureProviderGuard> {
        self.require_started("TestEngine::install_capture_provider")?;
        self.call_attached("imgui_test_engine_install_capture_provider", |raw| unsafe {
            sys::imgui_test_engine_install_capture_provider(raw, callback, user_data)
        })?;
        Ok(CaptureProviderGuard {
            engine: self.control.raw(),
            user_data,
            active: true,
        })
    }

    /// Shows Test Engine windows for a `Ui` from the attached Context and current frame.
    pub fn show_windows(&mut self, ui: &Ui, opened: Option<&mut bool>) -> TestEngineResult<()> {
        const OPERATION: &str = "TestEngine::show_windows";
        self.require_started(OPERATION)?;
        let expected = self
            .control
            .context_id()
            .ok_or_else(|| self.state_error(OPERATION, "the engine is not attached"))?;
        let actual = ui.context_id();
        if actual != expected {
            return Err(TestEngineError::ContextMismatch {
                operation: OPERATION,
                expected,
                actual,
            });
        }

        let binding = self.attached_binding(OPERATION)?;
        let raw = self.control.raw();
        let opened = opened.map_or(std::ptr::null_mut(), |value| value as *mut bool);
        let status = binding
            .try_with_bound_context(|| unsafe {
                let context = dear_imgui_rs::sys::igGetCurrentContext();
                if context.is_null() || !(*context).WithinFrameScope {
                    None
                } else {
                    Some(sys::imgui_test_engine_show_windows(raw, opened))
                }
            })
            .map_err(|source| TestEngineError::ContextBinding {
                operation: OPERATION,
                source,
            })?;
        let status = status.ok_or(TestEngineError::FrameNotActive {
            operation: OPERATION,
        })?;
        ffi_status("imgui_test_engine_show_windows", status)
    }

    /// Registers the native built-in integration tests.
    pub fn register_default_tests(&mut self) -> TestEngineResult<()> {
        self.require_ready("TestEngine::register_default_tests")?;
        self.call_attached("imgui_test_engine_register_default_tests", |raw| unsafe {
            sys::imgui_test_engine_register_default_tests(raw)
        })
    }

    /// Registers a script test after all script commands have been validated.
    ///
    /// Rust retains script ownership until native registration returns `Success`.
    pub fn add_script_test<F>(
        &mut self,
        category: &str,
        name: &str,
        build: F,
    ) -> TestEngineResult<()>
    where
        F: FnOnce(&mut ScriptTest<'_>) -> TestEngineResult<()>,
    {
        const OPERATION: &str = "TestEngine::add_script_test";
        self.require_ready(OPERATION)?;
        validate_name(OPERATION, "category", category)?;
        validate_name(OPERATION, "name", name)?;

        let binding = self.attached_binding(OPERATION)?;
        let engine_raw = self.control.raw();
        binding
            .try_with_bound_context(|| {
                let mut script = Script::create()?;
                if let Err(error) = build(&mut ScriptTest {
                    script: &mut script,
                }) {
                    let _ = script.destroy();
                    return Err(error);
                }

                let register_status =
                    with_scratch_txt_two(category, name, |category_ptr, name_ptr| unsafe {
                        sys::imgui_test_engine_register_script_test(
                            engine_raw,
                            category_ptr,
                            name_ptr,
                            script.raw(),
                        )
                    });
                if let Err(error) =
                    ffi_status("imgui_test_engine_register_script_test", register_status)
                {
                    let _ = script.destroy();
                    return Err(error);
                }
                script.disarm();
                Ok(())
            })
            .map_err(|source| TestEngineError::ContextBinding {
                operation: OPERATION,
                source,
            })?
    }

    /// Queues one Test Engine group. A queued/running/terminal run must be consumed first.
    pub fn queue_tests(
        &mut self,
        group: TestGroup,
        filter: Option<&str>,
        run_flags: RunFlags,
    ) -> TestEngineResult<()> {
        const OPERATION: &str = "TestEngine::queue_tests";
        self.require_ready(OPERATION)?;
        if let Some(filter) = filter {
            validate_c_string(OPERATION, "filter", filter)?;
        }
        if !RunFlags::all().contains(run_flags) {
            return Err(TestEngineError::invalid_input(
                OPERATION,
                "run_flags",
                "flags contain bits unknown to this Test Engine version",
            ));
        }

        let status = match filter {
            Some(filter) => with_scratch_txt(filter, |filter_ptr| {
                self.call_attached_raw(OPERATION, |raw| unsafe {
                    sys::imgui_test_engine_queue_tests(
                        raw,
                        group as sys::ImGuiTestEngineGroup,
                        filter_ptr,
                        run_flags.bits() as i32,
                    )
                })
            })?,
            None => self.call_attached_raw(OPERATION, |raw| unsafe {
                sys::imgui_test_engine_queue_tests(
                    raw,
                    group as sys::ImGuiTestEngineGroup,
                    c"".as_ptr(),
                    run_flags.bits() as i32,
                )
            })?,
        };
        ffi_status("imgui_test_engine_queue_tests", status)?;
        self.control.set_run_state(RunState::Queued);
        self.refresh_run_state()
    }

    /// Queues the default test group.
    pub fn queue_all_tests(&mut self) -> TestEngineResult<()> {
        self.queue_tests(TestGroup::Tests, None, RunFlags::NONE)
    }

    /// Returns the current native result counters without consuming terminal state.
    pub fn result_summary(&mut self) -> TestEngineResult<ResultSummary> {
        self.require_attached("TestEngine::result_summary")?;
        self.refresh_run_state()?;
        self.result_summary_unchecked()
    }

    /// Consumes a terminal summary and transitions the run state back to `Ready`.
    pub fn take_terminal_summary(&mut self) -> TestEngineResult<Option<ResultSummary>> {
        self.require_started("TestEngine::take_terminal_summary")?;
        self.refresh_run_state()?;
        if self.control.run_state() != RunState::Terminal {
            return Ok(None);
        }
        let summary = self.result_summary_unchecked()?;
        self.control
            .set_run_state(self.control.run_state().after_terminal_consumed());
        Ok(Some(summary))
    }

    pub fn is_test_queue_empty(&mut self) -> TestEngineResult<bool> {
        self.require_started("TestEngine::is_test_queue_empty")?;
        let empty = self.query_bool(
            "imgui_test_engine_is_test_queue_empty",
            |raw, output| unsafe { sys::imgui_test_engine_is_test_queue_empty(raw, output) },
        )?;
        self.refresh_run_state_from(None, Some(empty));
        Ok(empty)
    }

    pub fn is_running_tests(&mut self) -> TestEngineResult<bool> {
        self.require_started("TestEngine::is_running_tests")?;
        let running = self
            .query_bool("imgui_test_engine_is_running_tests", |raw, output| unsafe {
                sys::imgui_test_engine_is_running_tests(raw, output)
            })?;
        self.refresh_run_state_from(Some(running), None);
        Ok(running)
    }

    pub fn is_requesting_max_app_speed(&self) -> TestEngineResult<bool> {
        self.require_started("TestEngine::is_requesting_max_app_speed")?;
        self.query_bool(
            "imgui_test_engine_is_requesting_max_app_speed",
            |raw, output| unsafe {
                sys::imgui_test_engine_is_requesting_max_app_speed(raw, output)
            },
        )
    }

    pub fn try_abort_engine(&mut self) -> TestEngineResult<bool> {
        self.require_active_run("TestEngine::try_abort_engine")?;
        let aborted = self
            .query_bool("imgui_test_engine_try_abort_engine", |raw, output| unsafe {
                sys::imgui_test_engine_try_abort_engine(raw, output)
            })?;
        if aborted {
            self.refresh_run_state()?;
        }
        Ok(aborted)
    }

    pub fn abort_current_test(&mut self) -> TestEngineResult<()> {
        self.require_active_run("TestEngine::abort_current_test")?;
        self.call_attached("imgui_test_engine_abort_current_test", |raw| unsafe {
            sys::imgui_test_engine_abort_current_test(raw)
        })
    }

    pub fn set_run_speed(&mut self, speed: RunSpeed) -> TestEngineResult<()> {
        self.require_ready("TestEngine::set_run_speed")?;
        self.call_attached("imgui_test_engine_set_run_speed", |raw| unsafe {
            sys::imgui_test_engine_set_run_speed(raw, speed as sys::ImGuiTestEngineRunSpeed)
        })
    }

    pub fn set_verbose_level(&mut self, level: VerboseLevel) -> TestEngineResult<()> {
        self.require_ready("TestEngine::set_verbose_level")?;
        self.call_attached("imgui_test_engine_set_verbose_level", |raw| unsafe {
            sys::imgui_test_engine_set_verbose_level(raw, level as sys::ImGuiTestEngineVerboseLevel)
        })
    }

    /// Sets how much context is retained and printed when a test fails.
    pub fn set_verbose_level_on_error(&mut self, level: VerboseLevel) -> TestEngineResult<()> {
        self.require_ready("TestEngine::set_verbose_level_on_error")?;
        self.call_attached(
            "imgui_test_engine_set_verbose_level_on_error",
            |raw| unsafe {
                sys::imgui_test_engine_set_verbose_level_on_error(
                    raw,
                    level as sys::ImGuiTestEngineVerboseLevel,
                )
            },
        )
    }

    /// Mirrors Test Engine log output to the process terminal.
    pub fn set_log_to_tty(&mut self, enabled: bool) -> TestEngineResult<()> {
        self.require_ready("TestEngine::set_log_to_tty")?;
        self.call_attached("imgui_test_engine_set_log_to_tty", |raw| unsafe {
            sys::imgui_test_engine_set_log_to_tty(raw, enabled)
        })
    }

    /// Selects whether a successful capture is saved or discarded.
    ///
    /// This does not install a framebuffer provider. Use
    /// [`crate::TestRunner::run_graphical_with_capture`] when capture support is required.
    pub fn set_capture_output(&mut self, output: CaptureOutput) -> TestEngineResult<()> {
        self.require_ready("TestEngine::set_capture_output")?;
        let enabled = matches!(output, CaptureOutput::Save);
        self.call_attached(
            "imgui_test_engine_set_capture_output_enabled",
            |raw| unsafe { sys::imgui_test_engine_set_capture_output_enabled(raw, enabled) },
        )
    }

    pub fn install_default_crash_handler(&self) -> TestEngineResult<()> {
        self.require_started("TestEngine::install_default_crash_handler")?;
        self.call_attached(
            "imgui_test_engine_install_default_crash_handler",
            |_| unsafe { sys::imgui_test_engine_install_default_crash_handler() },
        )
    }

    fn detach_from_live_context(&mut self) -> TestEngineResult<()> {
        let binding = self.attached_binding("TestEngine::shutdown")?;
        if self.query_bool_with_binding(
            &binding,
            "imgui_test_engine_is_started",
            |raw, output| unsafe { sys::imgui_test_engine_is_started(raw, output) },
        )? {
            self.call_with_binding(&binding, "imgui_test_engine_stop", |raw| unsafe {
                sys::imgui_test_engine_stop(raw)
            })?;
        }
        if self.query_bool_with_binding(
            &binding,
            "imgui_test_engine_is_bound",
            |raw, output| unsafe { sys::imgui_test_engine_is_bound(raw, output) },
        )? {
            self.call_with_binding(&binding, "imgui_test_engine_unbind", |raw| unsafe {
                sys::imgui_test_engine_unbind(raw)
            })?;
        }

        // A live ContextBinding excludes teardown, so the core lease must still be Active here.
        match self.lease.as_mut().map(ContextAttachmentLease::detach) {
            Some(Ok(true)) => {}
            Some(Err(source)) => {
                return Err(TestEngineError::AttachmentDetach {
                    operation: "TestEngine::shutdown",
                    source,
                });
            }
            None | Some(Ok(false)) => {
                return Err(TestEngineError::AttachmentInvariant {
                    operation: "TestEngine::shutdown",
                    detail: "live attachment lease was not active; Context teardown must synchronize state first",
                });
            }
        }
        self.lease = None;
        self.control.mark_detached();
        Ok(())
    }

    fn destroy_detached(&self) -> TestEngineResult<()> {
        let raw = self.control.raw();
        if raw.is_null() {
            self.control.mark_destroyed();
            return Ok(());
        }

        let status = match self.control.binding() {
            Some(binding) if binding.is_alive() => binding
                .try_with_bound_context(|| unsafe { sys::imgui_test_engine_destroy_context(raw) })
                .map_err(|source| TestEngineError::ContextBinding {
                    operation: "TestEngine::shutdown",
                    source,
                })?,
            _ => unsafe { sys::imgui_test_engine_destroy_context(raw) },
        };
        ffi_status("imgui_test_engine_destroy_context", status)?;
        self.control.mark_destroyed();
        Ok(())
    }

    fn refresh_run_state(&self) -> TestEngineResult<()> {
        if !matches!(
            self.control.run_state(),
            RunState::Queued | RunState::Running
        ) {
            return Ok(());
        }
        let running = self
            .query_bool("imgui_test_engine_is_running_tests", |raw, output| unsafe {
                sys::imgui_test_engine_is_running_tests(raw, output)
            })?;
        let empty = self.query_bool(
            "imgui_test_engine_is_test_queue_empty",
            |raw, output| unsafe { sys::imgui_test_engine_is_test_queue_empty(raw, output) },
        )?;
        self.refresh_run_state_from(Some(running), Some(empty));
        Ok(())
    }

    fn refresh_run_state_from(&self, running: Option<bool>, empty: Option<bool>) {
        if !matches!(
            self.control.run_state(),
            RunState::Queued | RunState::Running
        ) {
            return;
        }
        if running == Some(true) {
            self.control.set_run_state(RunState::Running);
        } else if empty == Some(true) {
            self.control.set_run_state(RunState::Terminal);
        }
    }

    fn result_summary_unchecked(&self) -> TestEngineResult<ResultSummary> {
        let mut raw_summary = sys::ImGuiTestEngineResultSummary_c::default();
        self.call_attached("imgui_test_engine_get_result_summary", |raw| unsafe {
            sys::imgui_test_engine_get_result_summary(raw, &mut raw_summary)
        })?;
        ResultSummary::try_from_raw(
            raw_summary.CountTested,
            raw_summary.CountSuccess,
            raw_summary.CountInQueue,
        )
    }

    fn require_ready(&self, operation: &'static str) -> TestEngineResult<()> {
        if self.control.attachment_state() == AttachmentState::Attached
            && self.control.run_state().accepts_queue()
        {
            Ok(())
        } else {
            Err(self.state_error(
                operation,
                "operation requires an attached, started engine in Ready state",
            ))
        }
    }

    fn require_active_run(&self, operation: &'static str) -> TestEngineResult<()> {
        if self.control.attachment_state() == AttachmentState::Attached
            && matches!(
                self.control.run_state(),
                RunState::Queued | RunState::Running
            )
        {
            Ok(())
        } else {
            Err(self.state_error(operation, "operation requires a queued or running test"))
        }
    }

    fn require_started(&self, operation: &'static str) -> TestEngineResult<()> {
        if self.control.attachment_state() == AttachmentState::Attached
            && self.control.run_state() != RunState::Inactive
        {
            Ok(())
        } else {
            Err(self.state_error(
                operation,
                "operation requires an attached engine that has not been stopped",
            ))
        }
    }

    fn require_attached(&self, operation: &'static str) -> TestEngineResult<()> {
        if self.control.attachment_state() == AttachmentState::Attached {
            Ok(())
        } else {
            Err(self.state_error(operation, "operation requires a live Context attachment"))
        }
    }

    fn state_error(&self, operation: &'static str, detail: &'static str) -> TestEngineError {
        TestEngineError::invalid_state(
            operation,
            self.control.attachment_state(),
            self.control.run_state(),
            detail,
        )
    }

    fn attached_binding(&self, operation: &'static str) -> TestEngineResult<ContextBinding> {
        if self.control.attachment_state() != AttachmentState::Attached {
            return Err(self.state_error(operation, "operation requires a live Context attachment"));
        }
        self.control.binding().ok_or_else(|| {
            self.state_error(
                operation,
                "attached engine has no Context binding capability",
            )
        })
    }

    fn call_attached(
        &self,
        operation: &'static str,
        call: impl FnOnce(*mut sys::ImGuiTestEngine) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<()> {
        let binding = self.attached_binding(operation)?;
        self.call_with_binding(&binding, operation, call)
    }

    fn call_attached_raw(
        &self,
        operation: &'static str,
        call: impl FnOnce(*mut sys::ImGuiTestEngine) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<sys::ImGuiTestEngineStatus> {
        let binding = self.attached_binding(operation)?;
        binding
            .try_with_bound_context(|| call(self.control.raw()))
            .map_err(|source| TestEngineError::ContextBinding { operation, source })
    }

    fn call_with_binding(
        &self,
        binding: &ContextBinding,
        operation: &'static str,
        call: impl FnOnce(*mut sys::ImGuiTestEngine) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<()> {
        let status = binding
            .try_with_bound_context(|| call(self.control.raw()))
            .map_err(|source| TestEngineError::ContextBinding { operation, source })?;
        ffi_status(operation, status)
    }

    fn query_bool(
        &self,
        operation: &'static str,
        query: impl FnOnce(*mut sys::ImGuiTestEngine, *mut bool) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<bool> {
        let binding = self.attached_binding(operation)?;
        self.query_bool_with_binding(&binding, operation, query)
    }

    fn query_bool_with_binding(
        &self,
        binding: &ContextBinding,
        operation: &'static str,
        query: impl FnOnce(*mut sys::ImGuiTestEngine, *mut bool) -> sys::ImGuiTestEngineStatus,
    ) -> TestEngineResult<bool> {
        let mut output = false;
        self.call_with_binding(binding, operation, |raw| query(raw, &mut output))?;
        Ok(output)
    }
}

impl Drop for TestEngine {
    fn drop(&mut self) {
        if self.shutdown().is_err() && self.control.attachment_state() != AttachmentState::Destroyed
        {
            // U7 exception injection is one-shot, so this retry covers every supported native
            // failure path. Persistent external ABI failure cannot be reported from Drop; callers
            // that need that signal must use explicit shutdown while ownership is retained.
            let _ = self.shutdown();
        }
    }
}

fn validate_c_string(
    operation: &'static str,
    argument: &'static str,
    value: &str,
) -> TestEngineResult<()> {
    if value.contains('\0') {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "string contains an interior NUL byte",
        ));
    }
    Ok(())
}

fn validate_name(
    operation: &'static str,
    argument: &'static str,
    value: &str,
) -> TestEngineResult<()> {
    validate_c_string(operation, argument, value)?;
    if value.is_empty() {
        return Err(TestEngineError::invalid_input(
            operation,
            argument,
            "string must not be empty",
        ));
    }
    Ok(())
}
