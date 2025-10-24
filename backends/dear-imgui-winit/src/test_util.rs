#[cfg(test)]
pub(crate) mod test_sync {
    use std::sync::{Mutex, OnceLock};

    // Single global mutex shared across all tests in this crate
    static CTX_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    pub fn lock_context() -> std::sync::MutexGuard<'static, ()> {
        CTX_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test context mutex poisoned")
    }
}
