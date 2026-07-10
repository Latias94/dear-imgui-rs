use std::sync::{Mutex, MutexGuard, OnceLock};

/// Global test guard for dear-imgui-reflect integration tests.
///
/// Dear ImGui's current-context state is not designed for these test contexts
/// to be driven concurrently. The reflection sessions themselves are local;
/// this guard only serializes the native Dear ImGui test harness.
pub fn test_guard() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}
