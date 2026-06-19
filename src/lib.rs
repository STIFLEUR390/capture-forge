mod error;
mod messaging;
mod recorder;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use oxichrome::log;
use wasm_bindgen::prelude::*;

/// Guards the panic hook against re-entrant invocation.
///
/// If `log!()` or string formatting panics inside the hook itself, this flag
/// prevents infinite recursion.  The hook exits immediately on re-entry,
/// allowing the default abort behaviour.
static PANICKING: AtomicBool = AtomicBool::new(false);

/// A global session handle set during `start()` and available for the
/// panic hook and message handlers.
static SESSION: OnceLock<Mutex<recorder::RecordingSession>> = OnceLock::new();

fn init_session() -> &'static Mutex<recorder::RecordingSession> {
    SESSION.get_or_init(|| Mutex::new(recorder::RecordingSession::new()))
}

/// Thin `extern` shim so the panic hook can call `console.error()` without
/// requiring `web-sys` as a direct dependency.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[oxichrome::extension(
    name = "Capture Forge",
    version = "0.1.0",
    permissions = ["storage"]
)]
struct Extension;

#[oxichrome::background]
async fn start() {
    // Preserve any hook that oxichrome or wasm-bindgen may have installed.
    let prev = std::panic::take_hook();

    // Install a custom panic hook that prevents WASM instance death.
    //
    // Without this, any Rust panic inside a WASM module would abort the
    // extension's entire WebAssembly instance, killing the service worker.
    std::panic::set_hook(Box::new(move |panic_info| {
        // Re-entrancy guard — if the hook itself panics, bail immediately.
        if PANICKING.swap(true, Ordering::SeqCst) {
            return; // Allow default abort.
        }

        let details = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic cause".to_string()
        };

        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown location".into());

        let message = format!("{} — at {}", details, location);

        // Log via console.error so the message appears in the error console.
        error(&message);

        // Attempt to transition the global session to Error state.
        //
        // If the session has not yet been initialised, the transition is
        // skipped — the extension will recover on next user interaction.
        if let Some(mutex) = SESSION.get() {
            if let Ok(mut session) = mutex.try_lock() {
                let _ = session.transition(recorder::SessionState::Error);
            }
        }

        // Re-invoke the previous hook so the runtime still gets the panic
        // for diagnostic purposes.
        prev(panic_info);

        // Reset the re-entrancy guard.
        PANICKING.store(false, Ordering::SeqCst);
    }));

    log!("Capture Forge started!");

    // Initialise the global session so the panic hook can reference it.
    init_session();
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    log!("Capture Forge installed: {:?}", details);
}
