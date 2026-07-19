use std::ptr;

use dear_imgui_test_engine_sys as sys;

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
