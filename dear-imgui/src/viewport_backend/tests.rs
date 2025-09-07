//! Tests for viewport backend functionality

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform_io::Viewport;
    use std::sync::{Arc, Mutex};

    // Test platform backend that records method calls
    #[derive(Debug, Default)]
    struct TestPlatformBackend {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl TestPlatformBackend {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn record_call(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_string());
        }
    }

    impl PlatformViewportBackend for TestPlatformBackend {
        fn create_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("create_window");
        }

        fn destroy_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("destroy_window");
        }

        fn show_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("show_window");
        }

        fn set_window_pos(&mut self, _viewport: &mut Viewport, pos: [f32; 2]) {
            self.record_call(&format!("set_window_pos({}, {})", pos[0], pos[1]));
        }

        fn get_window_pos(&mut self, _viewport: &mut Viewport) -> [f32; 2] {
            self.record_call("get_window_pos");
            [100.0, 200.0]
        }

        fn set_window_size(&mut self, _viewport: &mut Viewport, size: [f32; 2]) {
            self.record_call(&format!("set_window_size({}, {})", size[0], size[1]));
        }

        fn get_window_size(&mut self, _viewport: &mut Viewport) -> [f32; 2] {
            self.record_call("get_window_size");
            [800.0, 600.0]
        }

        fn set_window_focus(&mut self, _viewport: &mut Viewport) {
            self.record_call("set_window_focus");
        }

        fn get_window_focus(&mut self, _viewport: &mut Viewport) -> bool {
            self.record_call("get_window_focus");
            true
        }

        fn get_window_minimized(&mut self, _viewport: &mut Viewport) -> bool {
            self.record_call("get_window_minimized");
            false
        }

        fn set_window_title(&mut self, _viewport: &mut Viewport, title: &str) {
            self.record_call(&format!("set_window_title({})", title));
        }

        fn set_window_alpha(&mut self, _viewport: &mut Viewport, alpha: f32) {
            self.record_call(&format!("set_window_alpha({})", alpha));
        }

        fn update_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("update_window");
        }

        fn render_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("render_window");
        }

        fn swap_buffers(&mut self, _viewport: &mut Viewport) {
            self.record_call("swap_buffers");
        }

        fn create_vk_surface(
            &mut self,
            _viewport: &mut Viewport,
            _instance: u64,
            _out_surface: &mut u64,
        ) -> i32 {
            self.record_call("create_vk_surface");
            0
        }
    }

    // Test renderer backend
    #[derive(Debug, Default)]
    struct TestRendererBackend {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl TestRendererBackend {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn record_call(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_string());
        }
    }

    impl RendererViewportBackend for TestRendererBackend {
        fn create_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("create_window");
        }

        fn destroy_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("destroy_window");
        }

        fn set_window_size(&mut self, _viewport: &mut Viewport, size: [f32; 2]) {
            self.record_call(&format!("set_window_size({}, {})", size[0], size[1]));
        }

        fn render_window(&mut self, _viewport: &mut Viewport) {
            self.record_call("render_window");
        }

        fn swap_buffers(&mut self, _viewport: &mut Viewport) {
            self.record_call("swap_buffers");
        }
    }

    #[test]
    fn test_platform_viewport_context() {
        // Clear any existing context
        clear_platform_viewport_context();

        // Initially no context should be available
        assert!(!has_platform_viewport_context());

        // Create and set a test backend
        let backend = TestPlatformBackend::new();
        let calls_ref = backend.calls.clone();
        let context = PlatformViewportContext::new(backend);

        set_platform_viewport_context(context);

        // Now context should be available
        assert!(has_platform_viewport_context());

        // Test calling methods through the context
        let mut dummy_viewport = Viewport::dummy();

        with_platform_viewport_context(|backend| {
            backend.create_window(&mut dummy_viewport);
            backend.show_window(&mut dummy_viewport);
            backend.set_window_pos(&mut dummy_viewport, [10.0, 20.0]);
            let _pos = backend.get_window_pos(&mut dummy_viewport);
        });

        // Verify the calls were recorded
        let calls = calls_ref.lock().unwrap();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0], "create_window");
        assert_eq!(calls[1], "show_window");
        assert_eq!(calls[2], "set_window_pos(10, 20)");
        assert_eq!(calls[3], "get_window_pos");

        // Clear context
        clear_platform_viewport_context();
        assert!(!has_platform_viewport_context());
    }

    #[test]
    fn test_renderer_viewport_context() {
        // Clear any existing context
        clear_renderer_viewport_context();

        // Initially no context should be available
        assert!(!has_renderer_viewport_context());

        // Create and set a test backend
        let backend = TestRendererBackend::new();
        let calls_ref = backend.calls.clone();
        let context = RendererViewportContext::new(backend);

        set_renderer_viewport_context(context);

        // Now context should be available
        assert!(has_renderer_viewport_context());

        // Test calling methods through the context
        let mut dummy_viewport = Viewport::dummy();

        with_renderer_viewport_context(|backend| {
            backend.create_window(&mut dummy_viewport);
            backend.set_window_size(&mut dummy_viewport, [1024.0, 768.0]);
            backend.render_window(&mut dummy_viewport);
        });

        // Verify the calls were recorded
        let calls = calls_ref.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], "create_window");
        assert_eq!(calls[1], "set_window_size(1024, 768)");
        assert_eq!(calls[2], "render_window");

        // Clear context
        clear_renderer_viewport_context();
        assert!(!has_renderer_viewport_context());
    }

    #[test]
    fn test_context_error_handling() {
        // Clear contexts
        clear_platform_viewport_context();
        clear_renderer_viewport_context();

        // Test error handling when no context is available
        let result = try_with_platform_viewport_context::<_, (), ViewportError>(|_backend| Ok(()));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ViewportError::NoContext);

        let result = try_with_renderer_viewport_context::<_, (), ViewportError>(|_backend| Ok(()));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ViewportError::NoContext);
    }

    #[test]
    fn test_viewport_error_display() {
        let error = ViewportError::NoContext;
        assert_eq!(error.to_string(), "No viewport context available");

        let error = ViewportError::InvalidViewport;
        assert_eq!(error.to_string(), "Invalid viewport handle");

        let error = ViewportError::PlatformError("test error".to_string());
        assert_eq!(error.to_string(), "Platform error: test error");

        let error = ViewportError::RendererError("render failed".to_string());
        assert_eq!(error.to_string(), "Renderer error: render failed");
    }
}
