use std::ptr;
use std::sync::Mutex;

use dear_imgui_test_engine_sys as sys;

static TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe fn create_engine() -> *mut sys::ImGuiTestEngine {
    let mut engine = ptr::null_mut();
    assert_eq!(
        unsafe { sys::imgui_test_engine_create_context(&mut engine) },
        sys::ImGuiTestEngineStatus_Success
    );
    assert!(!engine.is_null());
    engine
}

#[test]
fn explicit_and_context_shutdown_unbind_each_engine_exactly_once() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    unsafe {
        assert_eq!(
            sys::imgui_test_engine_test_reset_lifecycle_counters(),
            sys::ImGuiTestEngineStatus_Success
        );

        let context_first_engine = create_engine();
        let context_first_ui = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert_eq!(
            sys::imgui_test_engine_start(context_first_engine, context_first_ui),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_stop(context_first_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(context_first_ui);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(context_first_engine),
            sys::ImGuiTestEngineStatus_Success
        );

        let explicit_engine = create_engine();
        let explicit_ui = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert_eq!(
            sys::imgui_test_engine_start(explicit_engine, explicit_ui),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_stop(explicit_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(explicit_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(explicit_ui);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(explicit_engine),
            sys::ImGuiTestEngineStatus_Success
        );

        let mut counters = sys::ImGuiTestEngineLifecycleCounters_c::default();
        assert_eq!(
            sys::imgui_test_engine_test_get_lifecycle_counters(&mut counters),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(counters.EnginesCreated, 2);
        assert_eq!(counters.EnginesStarted, 2);
        assert_eq!(counters.EnginesStopped, 2);
        assert_eq!(counters.EnginesUnbound, 2);
        assert_eq!(counters.EnginesDestroyed, 2);
    }
}

#[test]
fn process_binding_is_exclusive_until_native_unbind() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    unsafe {
        let first_engine = create_engine();
        let first_ui = dear_imgui_sys::igCreateContext(ptr::null_mut());
        let second_engine = create_engine();
        let second_ui = dear_imgui_sys::igCreateContext(ptr::null_mut());
        let mut first_id = 0;
        let mut second_id = 0;
        assert_eq!(
            sys::imgui_test_engine_get_engine_id(first_engine, &mut first_id),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_get_engine_id(second_engine, &mut second_id),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_ne!(first_id, 0);
        assert_ne!(first_id, second_id);

        assert_eq!(
            sys::imgui_test_engine_start(first_engine, first_ui),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_start(second_engine, second_ui),
            sys::ImGuiTestEngineStatus_BindingOccupied
        );
        assert_eq!(
            sys::imgui_test_engine_stop(first_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_start(second_engine, second_ui),
            sys::ImGuiTestEngineStatus_BindingOccupied,
            "stopping must not release the process binding"
        );

        dear_imgui_sys::igDestroyContext(first_ui);
        assert_eq!(
            sys::imgui_test_engine_start(second_engine, second_ui),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_stop(second_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(second_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(second_ui);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(first_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_destroy_context(second_engine),
            sys::ImGuiTestEngineStatus_Success
        );
    }
}

#[test]
fn post_bind_start_failure_rolls_back_and_can_retry() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    unsafe {
        assert_eq!(
            sys::imgui_test_engine_test_reset_lifecycle_counters(),
            sys::ImGuiTestEngineStatus_Success
        );
        let engine = create_engine();
        let ui = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_PostBind
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui),
            sys::ImGuiTestEngineStatus_Exception
        );
        let mut bound = true;
        let mut started = true;
        assert_eq!(
            sys::imgui_test_engine_is_bound(engine, &mut bound),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_is_started(engine, &mut started),
            sys::ImGuiTestEngineStatus_Success
        );
        assert!(!bound);
        assert!(!started);

        assert_eq!(
            sys::imgui_test_engine_start(engine, ui),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(ui);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        let mut counters = sys::ImGuiTestEngineLifecycleCounters_c::default();
        assert_eq!(
            sys::imgui_test_engine_test_get_lifecycle_counters(&mut counters),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(counters.EnginesCreated, 1);
        assert_eq!(counters.EnginesStarted, 1);
        assert_eq!(counters.EnginesStopped, 1);
        assert_eq!(counters.EnginesUnbound, 1);
        assert_eq!(counters.EnginesDestroyed, 1);
    }
}

#[test]
fn partial_builtin_suite_registration_rolls_back_and_retries() {
    let _guard = TEST_LOCK.lock().expect("test lock");
    unsafe {
        let engine = create_engine();
        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_SuiteRegistrationAfterFirstTest
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        let mut registered = -1;
        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_NativeDefaults,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_Exception
        );
        assert_eq!(registered, 0);
        let mut count = -1;
        assert_eq!(
            sys::imgui_test_engine_get_registered_test_count(
                engine,
                c"demo_tests".as_ptr(),
                &mut count,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(count, 0);

        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_NativeDefaults,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(registered, 2);
        assert_eq!(
            sys::imgui_test_engine_unregister_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_NativeDefaults,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_get_registered_test_count(
                engine,
                c"demo_tests".as_ptr(),
                &mut count,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(count, 0);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Success
        );
    }
}
