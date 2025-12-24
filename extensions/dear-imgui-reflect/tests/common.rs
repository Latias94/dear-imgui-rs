use std::sync::{Mutex, MutexGuard, OnceLock};

/// Global test guard for dear-imgui-reflect integration tests.
///
/// Dear ImGui (and the global reflect settings) are not designed to be mutated
/// concurrently across threads. These tests serialize access to avoid flaky
/// failures under `cargo test`'s default parallel execution.
pub fn test_guard() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}
