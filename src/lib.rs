mod chunk;
mod countdown;
mod error;
mod export;
mod lifecycle;
mod messaging;
mod preview;
mod recorder;
mod status_bar;
mod stream;

use std::collections::HashMap;
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

/// Stores exported preview data keyed by session ID.
///
/// The background writes exported WebM data here before opening the preview
/// tab. The runtime message handler reads it when the preview page requests
/// the data via `GET_PREVIEW_DATA`.
static PREVIEW_DATA: OnceLock<Mutex<HashMap<String, PreviewDataEntry>>> = OnceLock::new();

/// A single entry in the preview data store.
#[allow(dead_code)]
struct PreviewDataEntry {
    /// The raw WebM bytes from the export pipeline.
    webm_data: Vec<u8>,
    /// Integrity state label ("Clean", "Partial", "Incomplete").
    integrity: String,
}

fn init_session() -> &'static Mutex<recorder::RecordingSession> {
    SESSION.get_or_init(|| Mutex::new(recorder::RecordingSession::new()))
}

fn init_preview_store() -> &'static Mutex<HashMap<String, PreviewDataEntry>> {
    PREVIEW_DATA.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store exported preview data for a session so the preview page can retrieve it.
///
/// Called by the background orchestration after the export pipeline completes.
#[wasm_bindgen]
pub fn store_preview_data(session_id: &str, webm_data: &[u8], integrity: &str) {
    if let Some(store) = PREVIEW_DATA.get() {
        if let Ok(mut map) = store.lock() {
            map.insert(
                session_id.to_owned(),
                PreviewDataEntry {
                    webm_data: webm_data.to_vec(),
                    integrity: integrity.to_owned(),
                },
            );
        }
    }
}

/// Remove stored preview data for a session (after Delete or tab close).
#[wasm_bindgen]
pub fn clear_preview_data(session_id: &str) {
    if let Some(store) = PREVIEW_DATA.get() {
        if let Ok(mut map) = store.lock() {
            map.remove(session_id);
        }
    }
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
    permissions = ["storage", "unlimitedStorage", "desktopCapture", "tabCapture", "downloads"]
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

    // Initialise the preview data store.
    init_preview_store();

    // Register the runtime message handler for preview page communication.
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        use js_sys::{Array, Object, Reflect, Uint8Array};

        if let Some(chrome) = Reflect::get(&js_sys::global(), &"chrome".into()).ok() {
            if let Some(runtime) = Reflect::get(&chrome, &"runtime".into()).ok() {
                let handler = Closure::wrap(Box::new(move |message: JsValue, _sender: JsValue, send_response: JsValue| {
                    if let Ok(msg_obj) = message.dyn_into::<Object>() {
                        if let Ok(msg_type) = Reflect::get(&msg_obj, &"type".into())
                            .and_then(|v| v.as_string().ok_or(wasm_bindgen::JsValue::UNDEFINED))
                        {
                            match msg_type.as_str() {
                                "GET_PREVIEW_DATA" => {
                                    // Read the session ID from the message.
                                    if let Ok(sid) = Reflect::get(&msg_obj, &"sessionId".into())
                                        .and_then(|v| v.as_string().ok_or(wasm_bindgen::JsValue::UNDEFINED))
                                    {
                                        if let Some(store) = PREVIEW_DATA.get() {
                                            if let Ok(map) = store.lock() {
                                                if let Some(entry) = map.get(&sid) {
                                                    // Build the response object.
                                                    let response = Object::new();
                                                    let arr = Uint8Array::from(&entry.webm_data[..]);
                                                    Reflect::set(&response, &"webmData".into(), &arr.buffer()).ok();
                                                    // Call sendResponse.
                                                    if let Some(sr) = send_response.dyn_ref::<js_sys::Function>() {
                                                        let _ = sr.call1(&JsValue::NULL, &response);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                "PREVIEW_CLOSED" | "DELETE_RECORDING" => {
                                    // Transition session to Idle.
                                    if let Some(mutex) = SESSION.get() {
                                        if let Ok(mut session) = mutex.lock() {
                                            let _ = session.transition(recorder::SessionState::Idle);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    // Return true to indicate we'll call sendResponse asynchronously.
                    // For synchronous responses, Chrome handles this correctly.
                    wasm_bindgen::JsValue::from(true)
                }) as Box<dyn FnMut(JsValue, JsValue, JsValue) -> JsValue>);

                if let Ok(on_message) = Reflect::get(&runtime, &"onMessage".into()) {
                    let _ = Reflect::apply(
                        &Reflect::get(&on_message, &"addListener".into())
                            .expect("invariant: chrome.runtime.onMessage.addListener exists"),
                        &on_message,
                        &Array::of1(handler.as_ref().unchecked_ref()),
                    );
                }
                // Leak the closure — it lives for the extension's lifetime.
                handler.forget();
            }
        }
    }
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    log!("Capture Forge installed: {:?}", details);
}
