use std::{
    ffi::{CStr, CString},
    ptr,
};

use dear_imgui_test_engine_sys as sys;

unsafe fn diagnostic() -> String {
    let mut required = 0usize;
    assert_eq!(
        unsafe { sys::imgui_test_engine_get_last_error(ptr::null_mut(), 0, &mut required) },
        sys::ImGuiTestEngineStatus_Success
    );
    assert!(required >= 1);

    let mut bytes = vec![0i8; required];
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
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                ptr::null(),
                sys::ImGuiTestEngineRunFlags_None,
            ),
            sys::ImGuiTestEngineStatus_InvalidArgument
        );
        assert_eq!(
            sys::imgui_test_engine_queue_tests(engine, 99, c"".as_ptr(), 0),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                c"".as_ptr(),
                1 << 30,
            ),
            sys::ImGuiTestEngineStatus_OutOfRange
        );
        assert_eq!(
            sys::imgui_test_engine_set_verbose_level(engine, 99),
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
            sys::imgui_test_engine_script_table_set_column_enabled_by_label(
                missing_label_script,
                c"BoundaryTable".as_ptr(),
                c"missing label".as_ptr(),
                true,
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
        assert_eq!(
            sys::imgui_test_engine_queue_tests(
                engine,
                sys::ImGuiTestEngineGroup_Tests,
                c"".as_ptr(),
                sys::ImGuiTestEngineRunFlags_None,
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
                sys::imgui_test_engine_post_swap(engine),
                sys::ImGuiTestEngineStatus_Success
            );

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

        let mut runtime_summary = sys::ImGuiTestEngineResultSummary_c::default();
        assert_eq!(
            sys::imgui_test_engine_get_result_summary(engine, &mut runtime_summary),
            sys::ImGuiTestEngineStatus_Success
        );
        assert_eq!(runtime_summary.CountTested, 3);
        assert_eq!(runtime_summary.CountSuccess, 0);
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
