//! Shared test-only helpers.

use std::sync::{Mutex, MutexGuard};

/// Held by every test that reads or removes API-key environment variables, so
/// `remove_var` never races a concurrent `var` lookup.
pub static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn env_guard() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
