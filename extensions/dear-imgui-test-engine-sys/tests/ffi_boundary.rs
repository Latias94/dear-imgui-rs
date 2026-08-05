use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
    ptr,
    sync::Mutex,
};

#[cfg(feature = "capture")]
use std::ffi::c_void;

use dear_imgui_test_engine_sys as sys;

static TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe fn diagnostic() -> String {
    let mut required = 0usize;
    assert_eq!(
        unsafe { sys::imgui_test_engine_get_last_error(ptr::null_mut(), 0, &mut required) },
        sys::ImGuiTestEngineStatus_Success
    );
    assert!(required >= 1);

    let mut bytes = vec![0 as c_char; required];
    assert_eq!(
        unsafe {
            sys::imgui_test_engine_get_last_error(bytes.as_mut_ptr(), bytes.len(), &mut required)
        },
        sys::ImGuiTestEngineStatus_Success
    );
    unsafe { CStr::from_ptr(bytes.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

unsafe fn create_engine() -> *mut sys::ImGuiTestEngine {
    let mut engine = ptr::null_mut();
    assert_eq!(
        unsafe { sys::imgui_test_engine_create_context(&mut engine) },
        sys::ImGuiTestEngineStatus_Success
    );
    assert!(!engine.is_null());
    engine
}

unsafe fn registered_test_name(
    engine: *mut sys::ImGuiTestEngine,
    category: &CStr,
    index: i32,
) -> String {
    let mut required = 0usize;
    assert_eq!(
        unsafe {
            sys::imgui_test_engine_get_registered_test_name(
                engine,
                category.as_ptr(),
                index,
                ptr::null_mut(),
                0,
                &mut required,
            )
        },
        sys::ImGuiTestEngineStatus_Success
    );
    assert!(required > 1);
    let mut bytes = vec![0 as c_char; required];
    assert_eq!(
        unsafe {
            sys::imgui_test_engine_get_registered_test_name(
                engine,
                category.as_ptr(),
                index,
                bytes.as_mut_ptr(),
                bytes.len(),
                &mut required,
            )
        },
        sys::ImGuiTestEngineStatus_Success
    );
    unsafe { CStr::from_ptr(bytes.as_ptr()) }
        .to_str()
        .expect("registered test names must be UTF-8")
        .to_owned()
}

#[test]
fn built_in_suites_register_once_and_expose_their_exact_manifest() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        let engine = create_engine();
        let mut registered = -1;

        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(engine, 99, &mut registered),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(registered, 0);
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
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_NativeDefaults,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_InvalidState
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
        assert_eq!(count, 2);
        assert_eq!(
            registered_test_name(engine, c"demo_tests", 0),
            "basic_interaction"
        );
        assert_eq!(
            registered_test_name(engine, c"demo_tests", 1),
            "input_value"
        );
        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_UpstreamDocking,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(registered, 39);
        assert_eq!(
            sys::imgui_test_engine_get_registered_test_count(
                engine,
                c"docking".as_ptr(),
                &mut count,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(count, 39);
        assert_eq!(
            registered_test_name(engine, c"docking", 0),
            "docking_move_does_not_dock"
        );
        assert_eq!(
            registered_test_name(engine, c"docking", 38),
            "docking_settings_invalid_1"
        );

        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_UpstreamViewports,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(registered, 0);

        let ui_context = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert!(!ui_context.is_null());
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui_context),
            sys::ImGuiTestEngineStatus_Success
        );

        let unrelated_context = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert!(!unrelated_context.is_null());
        (*unrelated_context).IO.ConfigFlags |= dear_imgui_sys::ImGuiConfigFlags_ViewportsEnable
            | dear_imgui_sys::ImGuiConfigFlags_DockingEnable;
        (*unrelated_context).IO.BackendFlags |=
            dear_imgui_sys::ImGuiBackendFlags_PlatformHasViewports
                | dear_imgui_sys::ImGuiBackendFlags_RendererHasViewports;
        dear_imgui_sys::igSetCurrentContext(unrelated_context);
        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_UpstreamViewports,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_Unsupported
        );
        assert_eq!(registered, 0);
        assert!(diagnostic().contains("ViewportsEnable"));

        (*ui_context).IO.ConfigFlags |= dear_imgui_sys::ImGuiConfigFlags_ViewportsEnable;
        (*ui_context).IO.BackendFlags |= dear_imgui_sys::ImGuiBackendFlags_PlatformHasViewports
            | dear_imgui_sys::ImGuiBackendFlags_RendererHasViewports;
        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_UpstreamViewports,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_Unsupported
        );
        assert_eq!(registered, 0);
        assert!(diagnostic().contains("DockingEnable"));

        (*unrelated_context).IO.ConfigFlags &= !(dear_imgui_sys::ImGuiConfigFlags_ViewportsEnable
            | dear_imgui_sys::ImGuiConfigFlags_DockingEnable);
        (*unrelated_context).IO.BackendFlags &=
            !(dear_imgui_sys::ImGuiBackendFlags_PlatformHasViewports
                | dear_imgui_sys::ImGuiBackendFlags_RendererHasViewports);
        (*ui_context).IO.ConfigFlags |= dear_imgui_sys::ImGuiConfigFlags_DockingEnable;
        assert_eq!(
            sys::imgui_test_engine_register_builtin_test_suite(
                engine,
                sys::ImGuiTestEngineBuiltinTestSuite_UpstreamViewports,
                &mut registered,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        const VIEWPORT_TESTS: &[&str] = &[
            "viewport_basic_1",
            "viewport_translate",
            "viewport_parent_id",
            "viewport_platform_focus",
            "viewport_platform_focus_2",
            "viewport_platform_focus_3",
            "viewport_platform_focus_4",
            "viewport_platform_close",
            "viewport_platform_close_2",
            "viewport_owner_change_1",
            "viewport_owner_change_2",
        ];
        assert_eq!(registered, VIEWPORT_TESTS.len() as i32);
        assert_eq!(
            sys::imgui_test_engine_get_registered_test_count(
                engine,
                c"viewport".as_ptr(),
                &mut count,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(count, VIEWPORT_TESTS.len() as i32);
        for (index, expected) in VIEWPORT_TESTS.iter().enumerate() {
            assert_eq!(
                registered_test_name(engine, c"viewport", index as i32),
                *expected
            );
        }

        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(unrelated_context);
        dear_imgui_sys::igDestroyContext(ui_context);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Success
        );
    }
}

unsafe fn create_script() -> *mut sys::ImGuiTestEngineScript {
    let mut script = ptr::null_mut();
    assert_eq!(
        unsafe { sys::imgui_test_engine_script_create(&mut script) },
        sys::ImGuiTestEngineStatus_Success
    );
    assert!(!script.is_null());
    script
}

#[test]
fn ffi_boundary_is_total_and_preserves_lifecycle_state() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        assert_eq!(
            sys::imgui_test_engine_test_reset_lifecycle_counters(),
            sys::ImGuiTestEngineStatus_Success
        );

        assert_eq!(
            sys::imgui_test_engine_create_context(ptr::null_mut()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        let copied_error = diagnostic();
        assert!(copied_error.contains("out_engine"));

        let mut required = 0usize;
        assert_eq!(
            sys::imgui_test_engine_get_last_error(ptr::null_mut(), 0, &mut required),
            sys::ImGuiTestEngineStatus_Success
        );
        let mut too_small = [1i8; 1];
        assert_eq!(
            sys::imgui_test_engine_get_last_error(
                too_small.as_mut_ptr(),
                too_small.len(),
                &mut required,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(too_small[0], 0);
        let mut oversized = vec![1i8; required + 8];
        assert_eq!(
            sys::imgui_test_engine_get_last_error(
                oversized.as_mut_ptr(),
                oversized.len(),
                &mut required,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            CStr::from_ptr(oversized.as_ptr()).to_string_lossy(),
            copied_error
        );
        assert_eq!(
            sys::imgui_test_engine_get_last_error(ptr::null_mut(), 4, &mut required),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_get_last_error(ptr::null_mut(), 0, ptr::null_mut()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(diagnostic(), copied_error);

        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_EngineAllocation,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        let mut failed_engine = ptr::null_mut();
        assert_eq!(
            sys::imgui_test_engine_create_context(&mut failed_engine),
            sys::ImGuiTestEngineStatus_Exception
        );
        assert!(failed_engine.is_null());
        assert!(diagnostic().contains("allocation failed"));

        let engine = create_engine();
        let unknown_engine = ptr::dangling_mut::<sys::ImGuiTestEngine>();
        assert_eq!(
            sys::imgui_test_engine_destroy_context(unknown_engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_is_bound(engine, ptr::null_mut()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );

        let mut target = ptr::null_mut();
        assert_eq!(
            sys::imgui_test_engine_get_ui_context_target(engine, &mut target),
            sys::ImGuiTestEngineStatus_NotFound
        );
        assert!(target.is_null());
        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_set_run_speed(engine, 99),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        let mut run_id = 0;
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                ptr::null(),
                sys::ImGuiTestEngineRunFlags_None,
                &mut run_id,
            ),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_queue_tests(engine, 99, c"".as_ptr(), 0, &mut run_id),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                c"".as_ptr(),
                1 << 30,
                &mut run_id,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_set_verbose_level(engine, 99),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_set_verbose_level_on_error(engine, 99),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_set_log_to_tty(engine, false),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(99),
            sys::ImGuiTestEngineStatus_OutOfRange
        );

        assert_eq!(
            sys::imgui_test_engine_script_create(ptr::null_mut()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_script_item_click(ptr::null_mut(), c"item".as_ptr()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );

        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_ScriptAllocation,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        let mut failed_script = ptr::null_mut();
        assert_eq!(
            sys::imgui_test_engine_script_create(&mut failed_script),
            sys::ImGuiTestEngineStatus_Exception
        );
        assert!(failed_script.is_null());

        let script = create_script();
        assert_eq!(
            sys::imgui_test_engine_script_item_click(script, ptr::null()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_script_mouse_click(script, -1),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_mouse_click_multi(script, 0, 0),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_mouse_move_to_pos(script, f32::NAN, 0.0),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_window_resize(script, c"window".as_ptr(), -1.0, 1.0,),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_dock_into(
                script,
                ptr::null(),
                c"destination".as_ptr(),
                -1,
                false,
            ),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_script_dock_into(
                script,
                c"source".as_ptr(),
                ptr::null(),
                -1,
                false,
            ),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_script_dock_into(
                script,
                c"source".as_ptr(),
                c"destination".as_ptr(),
                99,
                false,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_resize_column(script, c"table".as_ptr(), -1, 10.0,),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_resize_column(
                script,
                c"table".as_ptr(),
                0,
                f32::INFINITY,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_resize_column_by_label(
                script,
                c"table".as_ptr(),
                ptr::null(),
                10.0,
            ),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_resize_column_by_label(
                script,
                c"table".as_ptr(),
                c"column".as_ptr(),
                f32::INFINITY,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_open_context_menu(script, c"table".as_ptr(), -2,),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_click_header(
                script,
                c"table".as_ptr(),
                c"column".as_ptr(),
                1,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_wait_for_item(script, c"item".as_ptr(), 0),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_item_open_all(script, c"item".as_ptr(), 0, -1),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_yield(script, 0),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_set_input_mode(script, 99),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_key_press(script, 0, 1),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        let long_ref = CString::new("x".repeat(300)).unwrap();
        assert_eq!(
            sys::imgui_test_engine_script_item_click(script, long_ref.as_ptr()),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_script_dock_into(
                script,
                long_ref.as_ptr(),
                c"destination".as_ptr(),
                -1,
                false,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );

        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_ScriptVectorGrowth,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_item_click(script, c"button".as_ptr()),
            sys::ImGuiTestEngineStatus_Exception
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_open_context_menu(
                script,
                c"missing table".as_ptr(),
                0,
            ),
            sys::ImGuiTestEngineStatus_Success
        );

        assert_eq!(
            sys::imgui_test_engine_register_script_test(
                engine,
                c"boundary".as_ptr(),
                c"lifecycle".as_ptr(),
                script,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_destroy(script),
            sys::ImGuiTestEngineStatus_InvalidState
        );

        let caller_owned_script = create_script();
        assert_eq!(
            sys::imgui_test_engine_script_mouse_down(caller_owned_script, 0),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_item_click(caller_owned_script, c"button".as_ptr(),),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_script_dock_into(
                caller_owned_script,
                c"source".as_ptr(),
                c"destination".as_ptr(),
                -1,
                false,
            ),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_register_script_test(
                engine,
                c"boundary".as_ptr(),
                c"pressed_mouse".as_ptr(),
                caller_owned_script,
            ),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_script_mouse_up(caller_owned_script, 0),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_destroy(caller_owned_script),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_destroy(caller_owned_script),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        let replacement_script = create_script();
        assert_eq!(
            sys::imgui_test_engine_script_item_click(replacement_script, c"fresh".as_ptr()),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_destroy(replacement_script),
            sys::ImGuiTestEngineStatus_Success
        );

        let missing_label_script = create_script();
        assert_eq!(
            sys::imgui_test_engine_script_set_ref(missing_label_script, c"Boundary Host".as_ptr(),),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_resize_column_by_label(
                missing_label_script,
                c"BoundaryTable".as_ptr(),
                c"missing label".as_ptr(),
                10.0,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_register_script_test(
                engine,
                c"boundary".as_ptr(),
                c"missing_label".as_ptr(),
                missing_label_script,
            ),
            sys::ImGuiTestEngineStatus_Success
        );

        let invalid_column_script = create_script();
        assert_eq!(
            sys::imgui_test_engine_script_set_ref(invalid_column_script, c"Boundary Host".as_ptr(),),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_script_table_resize_column(
                invalid_column_script,
                c"BoundaryTable".as_ptr(),
                99,
                10.0,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_register_script_test(
                engine,
                c"boundary".as_ptr(),
                c"invalid_column".as_ptr(),
                invalid_column_script,
            ),
            sys::ImGuiTestEngineStatus_Success
        );

        let ui_context = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert!(!ui_context.is_null());
        assert_eq!(
            sys::imgui_test_engine_start(engine, ptr::null_mut()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_UpstreamCall,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui_context),
            sys::ImGuiTestEngineStatus_Exception
        );
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui_context),
            sys::ImGuiTestEngineStatus_Success
        );

        let io = dear_imgui_sys::igGetIO_Nil();
        (*io).BackendFlags |= dear_imgui_sys::ImGuiBackendFlags_RendererHasTextures;
        (*io).DisplaySize = dear_imgui_sys::ImVec2_c { x: 800.0, y: 600.0 };
        (*io).DisplayFramebufferScale = dear_imgui_sys::ImVec2_c { x: 1.0, y: 1.0 };
        (*io).DeltaTime = 1.0 / 60.0;
        let mut run_id = 0;
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                c"".as_ptr(),
                sys::ImGuiTestEngineRunFlags_None,
                &mut run_id,
            ),
            sys::ImGuiTestEngineStatus_Success
        );

        let mut completed = false;
        for _ in 0..120 {
            dear_imgui_sys::igNewFrame();
            if dear_imgui_sys::igBegin(c"Boundary Host".as_ptr(), ptr::null_mut(), 0)
                && dear_imgui_sys::igBeginTable(
                    c"BoundaryTable".as_ptr(),
                    2,
                    dear_imgui_sys::ImGuiTableFlags_Hideable
                        | dear_imgui_sys::ImGuiTableFlags_Resizable,
                    dear_imgui_sys::ImVec2_c::default(),
                    0.0,
                )
            {
                dear_imgui_sys::igTableSetupColumn(
                    c"Known".as_ptr(),
                    dear_imgui_sys::ImGuiTableColumnFlags_None,
                    0.0,
                    0,
                );
                dear_imgui_sys::igTableSetupColumn(
                    c"Other".as_ptr(),
                    dear_imgui_sys::ImGuiTableColumnFlags_None,
                    0.0,
                    0,
                );
                dear_imgui_sys::igTableHeadersRow();
                dear_imgui_sys::igEndTable();
            }
            dear_imgui_sys::igEnd();
            dear_imgui_sys::igRender();
            assert_eq!(
                sys::imgui_test_engine_pre_swap(engine),
                sys::ImGuiTestEngineStatus_Success
            );
            let mut presentation_completed = false;
            assert_eq!(
                sys::imgui_test_engine_post_swap(engine, &mut presentation_completed),
                sys::ImGuiTestEngineStatus_Success
            );
            assert!(presentation_completed);

            let mut queue_empty = false;
            let mut running = true;
            assert_eq!(
                sys::imgui_test_engine_is_test_queue_empty(engine, &mut queue_empty),
                sys::ImGuiTestEngineStatus_Success
            );
            assert_eq!(
                sys::imgui_test_engine_is_running_tests(engine, &mut running),
                sys::ImGuiTestEngineStatus_Success
            );
            if queue_empty && !running {
                completed = true;
                break;
            }
        }
        assert!(completed, "scripted missing-table failure did not complete");
        assert_ne!(run_id, 0);
        let mut run_test_count = 0;
        assert_eq!(
            sys::imgui_test_engine_get_run_test_count(engine, run_id, &mut run_test_count),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(run_test_count, 3);

        let mut runtime_summary = sys::ImGuiTestEngineResultSummary_c::default();
        assert_eq!(
            sys::imgui_test_engine_get_result_summary(engine, &mut runtime_summary),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(runtime_summary.CountTested, 3);
        assert_eq!(runtime_summary.CountSuccess, 0);
        assert_eq!(
            sys::imgui_test_engine_finish_run(engine, run_id),
            sys::ImGuiTestEngineStatus_Success
        );
        let mut no_match_run_id = 0;
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                c"no-such-boundary-test".as_ptr(),
                sys::ImGuiTestEngineRunFlags_None,
                &mut no_match_run_id,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_ne!(no_match_run_id, run_id);
        assert_eq!(
            sys::imgui_test_engine_get_run_test_count(engine, no_match_run_id, &mut run_test_count,),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(run_test_count, 0);
        assert_eq!(
            sys::imgui_test_engine_finish_run(engine, no_match_run_id),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui_context),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(ui_context);

        assert_eq!(
            sys::imgui_test_engine_test_set_exception_injection(
                sys::ImGuiTestEngineExceptionPoint_UpstreamCall,
            ),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Exception
        );
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Success
        );

        let replacement_engine = create_engine();
        let mut replacement_bound = true;
        assert_eq!(
            sys::imgui_test_engine_is_bound(replacement_engine, &mut replacement_bound),
            sys::ImGuiTestEngineStatus_Success
        );
        assert!(!replacement_bound);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(replacement_engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );

        let mut bound = false;
        assert_eq!(
            sys::imgui_test_engine_is_bound(engine, &mut bound),
            sys::ImGuiTestEngineStatus_InvalidState
        );

        let mut counters = sys::ImGuiTestEngineLifecycleCounters_c::default();
        assert_eq!(
            sys::imgui_test_engine_test_get_lifecycle_counters(&mut counters),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(counters.EnginesCreated, 2);
        assert_eq!(counters.EnginesDestroyed, 2);
        assert_eq!(counters.EnginesStarted, 1);
        assert_eq!(counters.EnginesStopped, 1);
        assert_eq!(counters.EnginesUnbound, 1);
        assert_eq!(counters.ScriptsCreated, 5);
        assert_eq!(counters.ScriptsDestroyed, 5);
        assert_eq!(counters.ScriptsRegistered, 3);
        assert_eq!(
            sys::imgui_test_engine_test_reset_lifecycle_counters(),
            sys::ImGuiTestEngineStatus_Success
        );
    }
}

#[cfg(feature = "capture")]
unsafe extern "C" fn capture_probe(
    _viewport_id: u32,
    _x: i32,
    _y: i32,
    width: i32,
    height: i32,
    pixels: *mut u32,
    user_data: *mut c_void,
) -> bool {
    if user_data.is_null() || width < 0 || height < 0 {
        return false;
    }
    let calls = unsafe { &mut *user_data.cast::<usize>() };
    *calls += 1;
    let Some(pixel_count) = (width as usize).checked_mul(height as usize) else {
        return false;
    };
    if pixel_count != 0 && pixels.is_null() {
        return false;
    }
    if pixel_count != 0 {
        unsafe { std::slice::from_raw_parts_mut(pixels, pixel_count) }.fill(0xff00_00ff);
    }
    true
}

#[test]
fn presentation_cycle_rejects_missing_duplicate_and_aborted_boundaries() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        let engine = create_engine();
        let ui_context = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert!(!ui_context.is_null());
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui_context),
            sys::ImGuiTestEngineStatus_Success
        );

        let io = dear_imgui_sys::igGetIO_Nil();
        (*io).BackendFlags |= dear_imgui_sys::ImGuiBackendFlags_RendererHasTextures;
        (*io).DisplaySize = dear_imgui_sys::ImVec2_c { x: 64.0, y: 64.0 };
        (*io).DisplayFramebufferScale = dear_imgui_sys::ImVec2_c { x: 1.0, y: 1.0 };
        (*io).DeltaTime = 1.0 / 60.0;
        dear_imgui_sys::igNewFrame();
        dear_imgui_sys::igRender();

        let mut completed = true;
        assert_eq!(
            sys::imgui_test_engine_post_swap(engine, &mut completed),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert!(!completed);
        assert_eq!(
            sys::imgui_test_engine_abort_presentation(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );

        assert_eq!(
            sys::imgui_test_engine_pre_swap(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_pre_swap(engine),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert_eq!(
            sys::imgui_test_engine_post_swap(engine, ptr::null_mut()),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_abort_presentation(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        completed = true;
        assert_eq!(
            sys::imgui_test_engine_post_swap(engine, &mut completed),
            sys::ImGuiTestEngineStatus_InvalidState
        );
        assert!(!completed);

        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(ui_context);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Success
        );
    }
}

#[cfg(feature = "capture")]
#[test]
fn capture_provider_abort_and_stop_clear_every_borrowed_callback_state() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        let engine = create_engine();
        let ui_context = dear_imgui_sys::igCreateContext(ptr::null_mut());
        assert!(!ui_context.is_null());
        assert_eq!(
            sys::imgui_test_engine_start(engine, ui_context),
            sys::ImGuiTestEngineStatus_Success
        );

        let io = dear_imgui_sys::igGetIO_Nil();
        (*io).BackendFlags |= dear_imgui_sys::ImGuiBackendFlags_RendererHasTextures;
        (*io).DisplaySize = dear_imgui_sys::ImVec2_c { x: 64.0, y: 64.0 };
        (*io).DisplayFramebufferScale = dear_imgui_sys::ImVec2_c { x: 1.0, y: 1.0 };
        (*io).DeltaTime = 1.0 / 60.0;
        dear_imgui_sys::igNewFrame();
        dear_imgui_sys::igRender();

        let mut calls = 0usize;
        let owner = (&mut calls as *mut usize).cast::<c_void>();
        assert_eq!(
            sys::imgui_test_engine_install_capture_provider(engine, Some(capture_probe), owner,),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_test_set_interactive_capture_state(engine, true, false),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_pre_swap(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_abort_presentation(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_clear_capture_provider(engine, owner),
            sys::ImGuiTestEngineStatus_Success
        );

        let mut state = sys::ImGuiTestEngineCaptureState_c::default();
        assert_eq!(
            sys::imgui_test_engine_test_get_capture_state(engine, &mut state),
            sys::ImGuiTestEngineStatus_Success
        );
        assert!(!state.PresentationPending);
        assert!(!state.CaptureAbortRequested);
        assert!(!state.CaptureWaitPending);
        assert!(!state.ProviderInstalled);
        assert!(!state.ContextCapturing);
        assert!(!state.ToolCapturing);
        assert!(!state.IoCapturing);

        assert_eq!(
            sys::imgui_test_engine_install_capture_provider(engine, Some(capture_probe), owner,),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_stop(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            sys::imgui_test_engine_test_get_capture_state(engine, &mut state),
            sys::ImGuiTestEngineStatus_Success
        );
        assert!(!state.CaptureAbortRequested);
        assert!(!state.ProviderInstalled);
        assert_eq!(
            sys::imgui_test_engine_unbind(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        dear_imgui_sys::igDestroyContext(ui_context);
        assert_eq!(
            sys::imgui_test_engine_destroy_context(engine),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(
            calls, 0,
            "the fixture did not request a framebuffer readback"
        );
    }
}
