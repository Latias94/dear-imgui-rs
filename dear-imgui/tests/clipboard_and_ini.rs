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
