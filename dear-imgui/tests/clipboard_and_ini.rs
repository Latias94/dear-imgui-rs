use dear_imgui_rs as imgui;
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
