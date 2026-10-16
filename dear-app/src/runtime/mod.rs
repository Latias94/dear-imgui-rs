mod admission;
mod lifecycle;
mod ownership;
mod recovery;
mod runner;
#[cfg(test)]
mod runner_tests;
mod shutdown;
mod state;
mod surface;

pub(crate) use runner::run;
pub(crate) use state::RuntimeEvent;

#[cfg(test)]
pub(crate) fn imgui_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
