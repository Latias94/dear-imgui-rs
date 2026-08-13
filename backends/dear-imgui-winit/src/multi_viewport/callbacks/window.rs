use super::*;

enum CallbackDispatch<R> {
    Completed(R),
    Rejected,
}

pub(in super::super) fn run_callback<R>(
    name: &'static str,
    fallback: R,
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> R {
    run_callback_with_failure(name, fallback, || {}, callback)
}

fn run_callback_with_failure<R>(
    name: &'static str,
    fallback: R,
    failure: impl FnOnce(),
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> R {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_runtime(|control| {
            let authorized_destroy =
                name == "Platform_DestroyWindow" && control.teardown_callbacks_active();
            if !authorized_destroy {
                let contract = control
                    .platform_control()
                    .and_then(|platform| platform.validate_complete_contract_in_current_context());
                if let Err(error) = contract {
                    control.record_terminal_fault(error);
                    return CallbackDispatch::Rejected;
                }
            }
            CallbackDispatch::Completed(callback(control))
        })
    }));
    match result {
        Ok(Some(CallbackDispatch::Completed(value))) => value,
        Ok(Some(CallbackDispatch::Rejected)) | Ok(None) => {
            failure();
            fallback
        }
        Err(_) => {
            let _ = with_current_runtime(|control| {
                control
                    .record_terminal_fault(WinitPlatformError::CallbackPanicked { callback: name });
            });
            failure();
            fallback
        }
    }
}

// Platform callback functions following official ImGui backend pattern

/// Create a new viewport window
pub(in super::super) unsafe extern "C" fn winit_create_window(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback_with_failure(
        "Platform_CreateWindow",
        (),
        || {
            if !vp.is_null() {
                // SAFETY: Dear ImGui keeps the callback viewport alive for the call duration.
                unsafe { (*vp).PlatformRequestClose = true };
            }
        },
        |control| {
            if vp.is_null() {
                return;
            }
            clear_failed_viewport(control, vp);

            let Some(event_loop) = control.active_event_loop() else {
                record_viewport_failure(control, vp, WinitPlatformError::EventLoopUnavailable);
                return;
            };

            let vp_ref = unsafe { &mut *vp };
            if super::super::viewport_data::viewport_data_is_owned(control, vp) {
                return;
            }
            // Winit's lease covers all three platform fields. It intentionally leaves
            // PlatformHandleRaw null, but a foreign value there still makes the viewport
            // unavailable and must be rejected before allocating a native window or publishing
            // either of the fields Winit does own.
            if !vp_ref.PlatformUserData.is_null()
                || !vp_ref.PlatformHandle.is_null()
                || !vp_ref.PlatformHandleRaw.is_null()
            {
                record_viewport_failure(control, vp, WinitPlatformError::ForeignPlatformUserData);
                return;
            }

            // Handle viewport flags
            let viewport_flags = vp_ref.Flags;
            let window_policy = ViewportWindowPolicy::from_flags(viewport_flags);
            if let Err(error) =
                validate_policy_for_creation(window_policy, skip_taskbar_capability())
            {
                record_viewport_failure(control, vp, error);
                return;
            }
            // ImGui positions and sizes are in the native desktop coordinate space. The shared
            // coordinate bridge keeps that space physical on Windows/X11 and Cocoa logical on
            // macOS without applying the target window's scale to a global desktop coordinate.
            let position =
                sanitize::finite_vec2_f32([vp_ref.Pos.x, vp_ref.Pos.y]).unwrap_or([0.0, 0.0]);
            let mut size =
                sanitize::finite_vec2_f32([vp_ref.Size.x, vp_ref.Size.y]).unwrap_or([128.0, 128.0]);
            if size[0] <= 0.0 {
                size[0] = 128.0;
            }
            if size[1] <= 0.0 {
                size[1] = 128.0;
            }
            let mut window_attrs = WindowAttributes::default()
                .with_title("ImGui Viewport")
                .with_inner_size(window_size_from_desktop(size))
                .with_position(window_position_from_desktop(position))
                .with_visible(false)
                .with_decorations(window_policy.decorations);

            // Inactive creation is guaranteed only on the platforms where Winit exposes that
            // contract. Other window managers control focus themselves, but the advisory flag
            // must not block an otherwise valid viewport from being created.
            if supports_inactive_window_creation() {
                window_attrs = window_attrs.with_active(false);
            }

            if window_policy.top_most {
                window_attrs = window_attrs.with_window_level(WindowLevel::AlwaysOnTop);
            }

            if window_policy.skip_taskbar {
                #[cfg(target_os = "windows")]
                {
                    window_attrs = window_attrs.with_skip_taskbar(true);
                }
                #[cfg(target_os = "linux")]
                {
                    window_attrs = window_attrs.with_x11_window_type(vec![WindowType::Utility]);
                }
            }

            match event_loop.create_window(window_attrs) {
                Ok(window) => {
                    mvlog(format_args!(
                        "[winit-mv] Platform_CreateWindow id={} size=({}, {})",
                        vp_ref.ID, vp_ref.Size.x, vp_ref.Size.y
                    ));
                    // Ensure outer position matches ImGui expectation.
                    //
                    // ImGui platform coordinates are relative to the *client* origin, while winit only lets us
                    // position by outer window coordinates. Adjust by decoration offset when available.
                    let dpi_scale = unsafe { viewport_target_dpi_scale(vp) };
                    let outer_target = outer_position_from_client(&window, position, dpi_scale);
                    window.set_outer_position(window_position_from_desktop(outer_target));

                    let window = Arc::new(window);
                    let data = match ViewportData::new(Arc::clone(&window), false) {
                        Ok(data) => data,
                        Err(error) => {
                            record_viewport_failure(control, vp, error);
                            return;
                        }
                    };
                    if let Err(error) = data.set_cursor_hittest(window_policy.cursor_hittest) {
                        record_viewport_failure(control, vp, error);
                        return;
                    }
                    if let Err(error) = data.set_no_focus_on_click(window_policy.no_focus_on_click)
                    {
                        record_viewport_failure(control, vp, error);
                        return;
                    }
                    if let Ok(platform) = control.platform_control() {
                        platform.apply_current_window_state(&window);
                    }
                    data.window_policy.set(window_policy);
                    let data = match insert_viewport_data(control, vp, data) {
                        Ok(data) => data,
                        Err(error) => {
                            record_viewport_failure(control, vp, error);
                            return;
                        }
                    };
                    vp_ref.PlatformUserData = data.cast::<c_void>();
                    vp_ref.PlatformHandle = Arc::as_ptr(&window).cast_mut().cast();

                    // DPI controls UI scaling while framebuffer scale converts this backend's
                    // desktop coordinate unit into render-target pixels.
                    let scale = sanitize::positive_finite_f32_or(dpi_scale, 1.0);
                    vp_ref.DpiScale = scale;
                    let framebuffer_scale = framebuffer_scale_for_dpi_scale(f64::from(scale));
                    vp_ref.FramebufferScale.x = framebuffer_scale[0];
                    vp_ref.FramebufferScale.y = framebuffer_scale[1];

                    // Note: winit does not allow registering per-window event callbacks here.
                    // The application forwards events through `WinitPlatform::handle_event`.
                }
                Err(error) => {
                    record_viewport_failure(
                        control,
                        vp,
                        WinitPlatformError::WindowCreation {
                            message: error.to_string(),
                        },
                    );
                }
            }
        },
    );
}

/// Destroy a viewport window
pub(in super::super) unsafe extern "C" fn winit_destroy_window(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Platform_DestroyWindow", (), |control| {
        if vp.is_null() {
            return;
        }
        clear_failed_viewport(control, vp);
        if let Some(Err(error)) = with_viewport_data(control, vp, |data| {
            if data.is_main() {
                return Ok(());
            }
            let source_window = data.window();
            let source_id = source_window.id();
            let Some(main_window) = control.main_window() else {
                return control.retire_window_input(source_id, None);
            };
            let main_id = main_window.id();
            match transfer_mouse_capture(source_window, &main_window) {
                Ok(MouseCaptureTransfer::Transferred) => {
                    control.retire_window_input(source_id, Some(main_id))
                }
                Ok(MouseCaptureTransfer::NotOwned) => {
                    control.retire_window_input(source_id, Some(main_id))
                }
                Err(error) => {
                    control.retire_window_input(source_id, Some(main_id))?;
                    Err(error)
                }
            }
        }) {
            control.record_fault(error);
        }
        if !remove_viewport_data(control, vp) && unsafe { !(*vp).PlatformUserData.is_null() } {
            control.record_fault(WinitPlatformError::ForeignPlatformUserData);
        }
    });
}

/// Show a viewport window
pub(in super::super) unsafe extern "C" fn winit_show_window(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Platform_ShowWindow", (), |control| {
        if vp.is_null() {
            return;
        }
        let policy = ViewportWindowPolicy::from_flags(unsafe { (*vp).Flags });
        with_viewport_data(control, vp, |data| {
            if let Err(error) = sync_window_policy(data, policy, vp) {
                record_viewport_failure(control, vp, error);
                return;
            }
            let reconciliation =
                if requires_async_client_geometry_reconciliation() && policy.decorations {
                    let geometry = match viewport_client_geometry(vp) {
                        Ok(geometry) => geometry,
                        Err(error) => {
                            record_viewport_failure(control, vp, error);
                            return;
                        }
                    };
                    Some((geometry, decoration_offset(data.window())))
                } else {
                    None
                };
            if should_focus_on_show(policy) {
                data.window().set_visible(true);
                if let Err(error) = request_platform_window_focus(control, data.window()) {
                    record_viewport_failure(control, vp, error);
                }
            } else {
                if let Err(error) = show_window_without_activation(data.window()) {
                    record_viewport_failure(control, vp, error);
                    return;
                }
                if !policy.cursor_hittest
                    && let Err(error) = raise_window_without_activation(data.window())
                {
                    record_viewport_failure(control, vp, error);
                }
            }
            if let Some(((position, size), decoration_offset_before_show)) = reconciliation {
                data.request_client_geometry_reconciliation(
                    position,
                    size,
                    decoration_offset_before_show,
                );
            }
        });
    });
}

/// Get window position through an out-parameter to avoid MSVC small-aggregate returns.
pub(in super::super) unsafe extern "C" fn winit_get_window_pos_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_pos: *mut dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_get_window_pos_out", (), |control| {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = unsafe { &*vp };
            let position = with_viewport_data(control, vp, |data| {
                let window = data.window();
                window.inner_position().ok().and_then(|position| {
                    desktop_position_from_physical(position, window.scale_factor())
                })
            })
            .flatten()
            .unwrap_or([vp_ref.Pos.x, vp_ref.Pos.y]);
            r.x = position[0];
            r.y = position[1];
        }
        if !out_pos.is_null() {
            unsafe { *out_pos = r };
        }
    });
}

/// Set window position
pub(in super::super) unsafe extern "C" fn winit_set_window_pos(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    pos: *const dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_set_window_pos", (), |control| {
        if vp.is_null() || pos.is_null() {
            return;
        }
        let pos = unsafe { *pos };
        let Some(requested_position) = validate_viewport_position([pos.x, pos.y]) else {
            record_viewport_failure(
                control,
                vp,
                WinitPlatformError::InvalidViewportGeometry {
                    operation: "window positioning",
                    reason: "position must be finite and representable by a native window",
                },
            );
            return;
        };

        with_viewport_data(control, vp, |data| {
            let [x, y] = requested_position;
            let dpi_scale = unsafe { viewport_target_dpi_scale(vp) };
            let window = data.window();
            let desired_client = [x, y];
            let requires_reconciliation =
                requires_async_client_geometry_reconciliation() && window.is_decorated();
            let decoration_offset_before_move = requires_reconciliation
                .then(|| decoration_offset(window))
                .flatten();
            let outer_target = outer_position_from_client(window, desired_client, dpi_scale);
            window.set_outer_position(window_position_from_desktop(outer_target));
            if requires_reconciliation {
                let size = unsafe { &*vp }.Size;
                if let Some(size) = validate_viewport_size([size.x, size.y]) {
                    data.request_client_geometry_reconciliation(
                        desired_client,
                        size,
                        decoration_offset_before_move,
                    );
                } else {
                    data.update_reconciling_client_position(desired_client);
                }
            } else {
                data.update_reconciling_client_position(desired_client);
            }
        });
    });
}

/// Get window size through an out-parameter to avoid MSVC small-aggregate returns.
pub(in super::super) unsafe extern "C" fn winit_get_window_size_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_size: *mut dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_get_window_size_out", (), |control| {
        let mut r = dear_imgui_rs::sys::ImVec2 { x: 0.0, y: 0.0 };
        if !vp.is_null() {
            let vp_ref = unsafe { &*vp };
            let size =
                with_viewport_data(control, vp, |data| desktop_size_for_window(data.window()))
                    .unwrap_or([vp_ref.Size.x, vp_ref.Size.y]);
            r.x = size[0];
            r.y = size[1];
        }
        if !out_size.is_null() {
            unsafe { *out_size = r };
        }
    });
}

/// Set window size
pub(in super::super) unsafe extern "C" fn winit_set_window_size(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: *const dear_imgui_rs::sys::ImVec2,
) {
    run_callback("winit_set_window_size", (), |control| {
        if vp.is_null() || size.is_null() {
            return;
        }
        let size = unsafe { *size };
        let Some(size) = validate_viewport_size([size.x, size.y]) else {
            record_viewport_failure(
                control,
                vp,
                WinitPlatformError::InvalidViewportGeometry {
                    operation: "window resizing",
                    reason: "size must be finite, positive, and representable by a native window",
                },
            );
            return;
        };

        with_viewport_data(control, vp, |data| {
            let window = data.window();
            data.update_reconciling_client_size(size);
            if window
                .request_inner_size(window_size_from_desktop(size))
                .is_some()
            {
                data.request_geometry_refresh(false, true);
            }
        });
    });
}

/// Set window focus
pub(in super::super) unsafe extern "C" fn winit_set_window_focus(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("winit_set_window_focus", (), |control| {
        if vp.is_null() {
            return;
        }
        if let Some(Err(error)) = with_viewport_data(control, vp, |data| {
            request_platform_window_focus(control, data.window())
        }) {
            control.record_fault(error);
        }
    });
}

/// Get window focus
pub(in super::super) unsafe extern "C" fn winit_get_window_focus(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    run_callback("winit_get_window_focus", false, |control| {
        if vp.is_null() {
            return false;
        }
        with_viewport_data(control, vp, |data| {
            control.platform_window_focus(data.window().id(), data.window().has_focus())
        })
        .unwrap_or(false)
    })
}

/// Get window minimized state
pub(in super::super) unsafe extern "C" fn winit_get_window_minimized(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    run_callback("Platform_GetWindowMinimized", false, |control| {
        if vp.is_null() {
            return false;
        }
        with_viewport_data(control, vp, |data| {
            data.window().is_minimized().unwrap_or(false)
        })
        .unwrap_or(false)
    })
}

/// Set window title
pub(in super::super) unsafe extern "C" fn winit_set_window_title(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    title: *const c_char,
) {
    run_callback("Platform_SetWindowTitle", (), |control| {
        if vp.is_null() || title.is_null() {
            return;
        }
        let title = unsafe { CStr::from_ptr(title) }.to_string_lossy();
        with_viewport_data(control, vp, |data| data.window().set_title(title.as_ref()));
    });
}

/// Get window framebuffer scale
pub(in super::super) unsafe extern "C" fn winit_get_window_framebuffer_scale_out(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    out_scale: *mut dear_imgui_rs::sys::ImVec2,
) {
    run_callback("Platform_GetWindowFramebufferScale", (), |control| {
        if out_scale.is_null() {
            return;
        }

        let mut result = dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 };
        if vp.is_null() {
            unsafe { *out_scale = result };
            return;
        }

        let vp_ref = unsafe { &*vp };
        with_viewport_data(control, vp, |data| {
            let window = data.window();
            let scale = framebuffer_scale_for_window(window);
            if cfg!(feature = "mv-log") && (scale[0] - data.last_log_fb_scale.get()).abs() > 0.01 {
                mvlog(format_args!(
                    "[winit-mv] fb_scale changed id={} -> {:.2}",
                    vp_ref.ID, scale[0]
                ));
                data.last_log_fb_scale.set(scale[0]);
            }
            result = dear_imgui_rs::sys::ImVec2 {
                x: scale[0],
                y: scale[1],
            };
        });
        unsafe { *out_scale = result };
    })
}

/// Get window DPI scale (float)
pub(in super::super) unsafe extern "C" fn winit_get_window_dpi_scale(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> f32 {
    run_callback("Platform_GetWindowDpiScale", 1.0, |control| {
        if vp.is_null() {
            return 1.0;
        }
        with_viewport_data(control, vp, |data| {
            sanitize::positive_finite_f32_or(data.window().scale_factor() as f32, 1.0)
        })
        .unwrap_or(1.0)
    })
}

/// Notify viewport changed.
///
/// Dear ImGui calls this when a viewport changes monitor or ownership. We use it
/// for targeted debug output to diagnose DPI/scale transitions without per-frame spam.
pub(in super::super) unsafe extern "C" fn winit_on_changed_viewport(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Platform_OnChangedViewport", (), |_| {
        if vp.is_null() {
            return;
        }
        let vp_ref = &*vp;
        mvlog(format_args!(
            "[winit-mv] OnChangedViewport id={} pos=({:.1},{:.1}) size=({:.1},{:.1}) dpi_scale={:.2} fb_scale=({:.2},{:.2})",
            vp_ref.ID,
            vp_ref.Pos.x,
            vp_ref.Pos.y,
            vp_ref.Size.x,
            vp_ref.Size.y,
            vp_ref.DpiScale,
            vp_ref.FramebufferScale.x,
            vp_ref.FramebufferScale.y
        ));
    });
}

/// Platform render window (no-op; renderer handles rendering)
pub(in super::super) unsafe extern "C" fn winit_platform_render_window(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    run_callback("Platform_RenderWindow", (), |_| {});
}

/// Platform swap buffers (no-op; renderer handles present)
pub(in super::super) unsafe extern "C" fn winit_platform_swap_buffers(
    _vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    run_callback("Platform_SwapBuffers", (), |_| {});
}

/// Apply flags that can change while a viewport is alive.
pub(in super::super) unsafe extern "C" fn winit_update_window(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
) {
    run_callback("Platform_UpdateWindow", (), |control| {
        if vp.is_null() {
            return;
        }
        let policy = ViewportWindowPolicy::from_flags(unsafe { (*vp).Flags });
        with_viewport_data(control, vp, |data| {
            if let Err(error) = sync_window_policy(data, policy, vp) {
                record_viewport_failure(control, vp, error);
            }
        });
    });
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn viewport_geometry_rejects_values_native_windows_cannot_represent() {
        assert_eq!(
            validate_viewport_position([-1920.0, 32.0]),
            Some([-1920.0, 32.0])
        );
        assert_eq!(validate_viewport_position([f32::NAN, 32.0]), None);
        assert_eq!(validate_viewport_position([f32::MAX, 32.0]), None);

        assert_eq!(
            validate_viewport_size([1280.0, 720.0]),
            Some([1280.0, 720.0])
        );
        assert_eq!(validate_viewport_size([0.0, 720.0]), None);
        assert_eq!(validate_viewport_size([f32::INFINITY, 720.0]), None);
        assert_eq!(validate_viewport_size([f32::MAX, 720.0]), None);
    }

    #[test]
    fn focus_click_and_appearance_are_best_effort_but_taskbar_requires_capability() {
        let no_focus_on_click = ViewportWindowPolicy {
            no_focus_on_click: true,
            ..ViewportWindowPolicy::default()
        };
        assert!(
            validate_policy_for_creation(no_focus_on_click, SkipTaskbarCapability::Dynamic).is_ok()
        );

        let no_focus_on_appearing = ViewportWindowPolicy {
            no_focus_on_appearing: true,
            ..ViewportWindowPolicy::default()
        };
        assert!(
            validate_policy_for_creation(no_focus_on_appearing, SkipTaskbarCapability::Dynamic)
                .is_ok()
        );

        let no_taskbar = ViewportWindowPolicy {
            skip_taskbar: true,
            ..ViewportWindowPolicy::default()
        };
        assert!(matches!(
            validate_policy_for_creation(no_taskbar, SkipTaskbarCapability::Unsupported),
            Err(WinitPlatformError::UnsupportedViewportFlag {
                flag: "NoTaskBarIcon",
                ..
            })
        ));
        assert!(validate_policy_for_creation(no_taskbar, SkipTaskbarCapability::Inherent).is_ok());
    }

    #[test]
    fn show_focus_follows_no_focus_on_appearing_policy() {
        assert!(should_focus_on_show(ViewportWindowPolicy::default()));
        assert!(!should_focus_on_show(ViewportWindowPolicy {
            no_focus_on_appearing: true,
            ..ViewportWindowPolicy::default()
        }));
    }

    #[test]
    fn changing_decorations_requires_client_geometry_reconciliation() {
        let current = ViewportWindowPolicy::default();
        let decorations_changed = ViewportWindowPolicy {
            decorations: false,
            ..current
        };
        let top_most_changed = ViewportWindowPolicy {
            top_most: true,
            ..current
        };

        assert!(requires_client_geometry_reconciliation(
            current,
            decorations_changed
        ));
        assert!(!requires_client_geometry_reconciliation(
            current,
            top_most_changed
        ));
    }

    #[test]
    fn late_focus_and_unsupported_taskbar_changes_fail_closed() {
        let current = ViewportWindowPolicy::default();
        let late_no_focus = ViewportWindowPolicy {
            no_focus_on_click: true,
            ..current
        };
        assert!(
            validate_policy_transition(current, late_no_focus, SkipTaskbarCapability::Dynamic)
                .is_ok()
        );

        let taskbar_change = ViewportWindowPolicy {
            skip_taskbar: true,
            ..current
        };
        assert!(matches!(
            validate_policy_transition(current, taskbar_change, SkipTaskbarCapability::CreateOnly),
            Err(WinitPlatformError::UnsupportedViewportFlag {
                flag: "NoTaskBarIcon",
                ..
            })
        ));
        assert!(
            validate_policy_transition(current, taskbar_change, SkipTaskbarCapability::Inherent)
                .is_ok()
        );
    }
}
