use dear_imgui_rs as imgui;
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[derive(Clone)]
struct TestClipboardBackend {
    value: Arc<Mutex<Option<String>>>,
}

impl imgui::ClipboardBackend for TestClipboardBackend {
    fn get(&mut self) -> Option<String> {
        self.value.lock().unwrap().clone()
    }

    fn set(&mut self, text: &str) {
        *self.value.lock().unwrap() = Some(text.to_owned());
    }
}

#[test]
fn clipboard_helpers_work_with_backend() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let shared = Arc::new(Mutex::new(None));
    ctx.set_clipboard_backend(TestClipboardBackend {
        value: shared.clone(),
    });

    ctx.set_clipboard_text("hello");
    assert_eq!(ctx.clipboard_text().as_deref(), Some("hello"));
    assert_eq!(shared.lock().unwrap().as_deref(), Some("hello"));
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn clipboard_callbacks_use_passed_context_not_current_context() {
    let _guard = test_guard();

    let mut ctx_a = imgui::Context::create();
    let raw_a = ctx_a.as_raw();
    let shared_a = Arc::new(Mutex::new(None));
    ctx_a.set_clipboard_backend(TestClipboardBackend {
        value: shared_a.clone(),
    });

    let set_a;
    let get_a;
    unsafe {
        let platform_io_a = imgui::sys::igGetPlatformIO_ContextPtr(raw_a);
        set_a = (*platform_io_a)
            .Platform_SetClipboardTextFn
            .expect("clipboard setter should be installed");
        get_a = (*platform_io_a)
            .Platform_GetClipboardTextFn
            .expect("clipboard getter should be installed");
    }

    let suspended_a = ctx_a.suspend();

    let mut ctx_b = imgui::Context::create();
    let shared_b = Arc::new(Mutex::new(Some("b".to_owned())));
    ctx_b.set_clipboard_backend(TestClipboardBackend {
        value: shared_b.clone(),
    });

    let value = CString::new("a").unwrap();
    unsafe {
        set_a(raw_a, value.as_ptr());
    }

    assert_eq!(shared_a.lock().unwrap().as_deref(), Some("a"));
    assert_eq!(shared_b.lock().unwrap().as_deref(), Some("b"));

    let ptr = unsafe { get_a(raw_a) };
    assert!(!ptr.is_null());
    let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
    assert_eq!(text, "a");

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn clipboard_reentry_into_different_context_is_allowed() {
    let _guard = test_guard();

    let mut ctx_a = imgui::Context::create();
    let raw_a = ctx_a.as_raw();
    ctx_a.set_clipboard_backend(TestClipboardBackend {
        value: Arc::new(Mutex::new(Some("a".to_owned()))),
    });

    let get_a = unsafe {
        let platform_io_a = imgui::sys::igGetPlatformIO_ContextPtr(raw_a);
        (*platform_io_a)
            .Platform_GetClipboardTextFn
            .expect("clipboard getter should be installed")
    };

    let suspended_a = ctx_a.suspend();

    struct CrossContextReentrantBackend {
        other_ctx: *mut imgui::sys::ImGuiContext,
        other_get:
            unsafe extern "C" fn(*mut imgui::sys::ImGuiContext) -> *const std::os::raw::c_char,
        observed: Arc<Mutex<Option<String>>>,
    }

    impl imgui::ClipboardBackend for CrossContextReentrantBackend {
        fn get(&mut self) -> Option<String> {
            let ptr = unsafe { (self.other_get)(self.other_ctx) };
            if !ptr.is_null() {
                let text = unsafe { CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned();
                *self.observed.lock().unwrap() = Some(text);
            }
            Some("b".to_owned())
        }

        fn set(&mut self, _text: &str) {}
    }

    let mut ctx_b = imgui::Context::create();
    let observed = Arc::new(Mutex::new(None));
    ctx_b.set_clipboard_backend(CrossContextReentrantBackend {
        other_ctx: raw_a,
        other_get: get_a,
        observed: observed.clone(),
    });

    assert_eq!(ctx_b.clipboard_text().as_deref(), Some("b"));
    assert_eq!(observed.lock().unwrap().as_deref(), Some("a"));

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn clipboard_reentry_into_same_context_fails_closed() {
    let _guard = test_guard();

    struct SameContextReentrantBackend {
        raw_ctx: *mut imgui::sys::ImGuiContext,
        get_fn: Arc<
            Mutex<
                Option<
                    unsafe extern "C" fn(
                        *mut imgui::sys::ImGuiContext,
                    ) -> *const std::os::raw::c_char,
                >,
            >,
        >,
        nested_was_null: Arc<Mutex<bool>>,
    }

    impl imgui::ClipboardBackend for SameContextReentrantBackend {
        fn get(&mut self) -> Option<String> {
            let get_fn = self
                .get_fn
                .lock()
                .unwrap()
                .expect("clipboard getter should be installed");
            let ptr = unsafe { get_fn(self.raw_ctx) };
            *self.nested_was_null.lock().unwrap() = ptr.is_null();
            Some("outer".to_owned())
        }

        fn set(&mut self, _text: &str) {}
    }

    let mut ctx = imgui::Context::create();
    let raw_ctx = ctx.as_raw();
    let get_fn = Arc::new(Mutex::new(None));
    let nested_was_null = Arc::new(Mutex::new(false));

    ctx.set_clipboard_backend(SameContextReentrantBackend {
        raw_ctx,
        get_fn: get_fn.clone(),
        nested_was_null: nested_was_null.clone(),
    });

    unsafe {
        let platform_io = imgui::sys::igGetPlatformIO_ContextPtr(raw_ctx);
        *get_fn.lock().unwrap() = (*platform_io).Platform_GetClipboardTextFn;
    }

    assert_eq!(ctx.clipboard_text().as_deref(), Some("outer"));
    assert!(*nested_was_null.lock().unwrap());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn ini_disk_helpers_no_panic() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();

    let path = std::env::temp_dir().join("dear-imgui-rs-test-imgui.ini");
    let _ = ctx.load_ini_settings_from_disk(&path);
    let _ = ctx.save_ini_settings_to_disk(&path);
}
