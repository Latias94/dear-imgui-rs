mod lifecycle;
mod managed_textures;
mod recovery;
mod runner;
mod state;

pub(crate) use runner::run;

#[cfg(test)]
pub(crate) fn imgui_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
