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

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn moving_a_context_preserves_published_strings_and_clipboard() {
    fn configured_context() -> imgui::Context {
        let mut context = imgui::Context::create();
        context
            .set_ini_filename(Some("moved-settings.ini"))
            .unwrap();
        context.set_log_filename(Some("moved-log.txt")).unwrap();
        context.set_platform_name(Some("moved platform")).unwrap();
        context.set_renderer_name(Some("moved renderer")).unwrap();
        context.set_clipboard_backend(TestClipboardBackend {
            value: Arc::new(Mutex::new(Some("initial".into()))),
        });
        context
    }

    let _guard = test_guard();
    let mut contexts = Vec::with_capacity(1);
    contexts.push(configured_context());
    let mut moved = Box::new(contexts.pop().unwrap());
    let read_native = |context: &imgui::Context| {
        let io = unsafe { imgui::sys::igGetIO_ContextPtr(context.as_raw()) };
        [
            (unsafe { (*io).IniFilename }, c"moved-settings.ini"),
            (unsafe { (*io).LogFilename }, c"moved-log.txt"),
            (unsafe { (*io).BackendPlatformName }, c"moved platform"),
            (unsafe { (*io).BackendRendererName }, c"moved renderer"),
        ]
        .map(|(ptr, expected)| {
            assert_eq!(unsafe { CStr::from_ptr(ptr) }, expected);
            ptr
        })
    };
    let published = read_native(&moved);
    assert_eq!(moved.clipboard_text().as_deref(), Some("initial"));
    let moved_again = *moved;
    assert_eq!(read_native(&moved_again), published);
    moved_again.set_clipboard_text("updated");
    assert_eq!(moved_again.clipboard_text().as_deref(), Some("updated"));
    moved = Box::new(moved_again);
    assert_eq!(read_native(&moved), published);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn clipboard_setter_owns_the_text_passed_to_user_code() {
    use std::{cell::Cell, rc::Rc};

    struct Setter {
        native_text: *const u8,
        observed: Rc<Cell<Option<bool>>>,
    }

    impl imgui::ClipboardBackend for Setter {
        fn get(&mut self) -> Option<String> {
            None
        }

        fn set(&mut self, text: &str) {
            self.observed
                .set(Some(text == "copy me" && text.as_ptr() != self.native_text));
        }
    }

    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    let text = c"copy me";
    let observed = Rc::new(Cell::new(None));
    ctx.set_clipboard_backend(Setter {
        native_text: text.as_ptr().cast(),
        observed: observed.clone(),
    });
    unsafe { imgui::sys::igSetClipboardText(text.as_ptr()) };
    assert_eq!(observed.get(), Some(true));
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

    let suspended_a = ctx_a;

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

    let suspended_a = ctx_a;

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
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();

    let path = std::env::temp_dir().join("dear-imgui-rs-test-imgui.ini");
    let _ = ctx.load_ini_settings_from_disk(&path);
    let _ = ctx.save_ini_settings_to_disk(&path);
}
