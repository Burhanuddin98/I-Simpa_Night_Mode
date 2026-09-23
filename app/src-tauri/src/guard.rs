//! The command boundary. Every command body runs through [`blocking`], so a panic inside it comes
//! back to the UI as a [`CmdError`] with code `PANIC` and the window lives on.
//!
//! Tauri alone does not do this: an async command's future is spawned as a tokio task, and when
//! that task panics its resolver is dropped without a reply, so the JS promise never settles
//! (`panic_probe_unguarded` in the self-test measures exactly that). The release profile keeps
//! `panic = "unwind"` (root `Cargo.toml`), which this boundary depends on.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard};

use schemars::JsonSchema;
use serde::Serialize;

/// An error for the UI: a stable reason code and a message for people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CmdError {
    pub code: String,
    pub message: String,
}

pub type CmdResult<T> = Result<T, CmdError>;

impl CmdError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        CmdError {
            code: code.into(),
            message: message.into(),
        }
    }

    /// A panic caught at the boundary of `command`.
    pub fn panic(command: &str, payload: &(dyn Any + Send)) -> Self {
        CmdError::new(
            "PANIC",
            format!("command '{command}' panicked: {}", payload_message(payload)),
        )
    }
}

/// The text of a panic payload (`panic!` gives a `&str` or a `String`).
pub fn payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Runs `body` on Tauri's blocking pool, off the UI thread and off the async workers, and turns a
/// panic in it into `Err(CmdError { code: "PANIC", .. })`.
pub async fn blocking<T, F>(command: &'static str, body: F) -> CmdResult<T>
where
    F: FnOnce() -> CmdResult<T> + Send + 'static,
    T: Send + 'static,
{
    let joined =
        tauri::async_runtime::spawn_blocking(move || catch_unwind(AssertUnwindSafe(body))).await;
    match joined {
        Ok(Ok(result)) => result,
        Ok(Err(payload)) => Err(CmdError::panic(command, &*payload)),
        Err(tauri::Error::JoinError(e)) if e.is_panic() => {
            Err(CmdError::panic(command, &*e.into_panic()))
        }
        Err(e) => Err(CmdError::new(
            "TASK_FAILED",
            format!("command '{command}' did not complete: {e}"),
        )),
    }
}

/// Locks shared state. A lock poisoned by an earlier panic is refused rather than trusted: the
/// panic may have left the state half-changed.
pub fn lock<'a, T>(mutex: &'a Mutex<T>, what: &str) -> CmdResult<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| {
        CmdError::new(
            "STATE_POISONED",
            format!("the {what} state was poisoned by an earlier panic; restart the app"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_becomes_an_error_and_the_next_call_works() {
        let err = tauri::async_runtime::block_on(blocking("probe", || -> CmdResult<()> {
            panic!("deliberate")
        }))
        .unwrap_err();
        assert_eq!(err.code, "PANIC");
        assert!(err.message.contains("deliberate"), "{}", err.message);
        let ok = tauri::async_runtime::block_on(blocking("ping", || Ok("pong")));
        assert_eq!(ok, Ok("pong"));
    }

    #[test]
    fn string_payloads_are_kept() {
        let err = tauri::async_runtime::block_on(blocking("probe", || -> CmdResult<()> {
            panic!("{} {}", "formatted", 7)
        }))
        .unwrap_err();
        assert!(err.message.ends_with("formatted 7"), "{}", err.message);
    }

    #[test]
    fn a_poisoned_lock_is_refused() {
        let m = std::sync::Arc::new(Mutex::new(0));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert_eq!(lock(&m, "test").unwrap_err().code, "STATE_POISONED");
    }
}
