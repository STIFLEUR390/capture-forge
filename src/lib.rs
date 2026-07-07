mod chunk;
mod countdown;
mod error;
mod export;
mod lifecycle;
mod messaging;
mod preview;
mod recorder;
mod recovery;
mod recovery_toast;
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

/// Holds the active recovery toast so it can be removed from message handlers.
static RECOVERY_TOAST: OnceLock<Mutex<Option<recovery_toast::RecoveryToast>>> = OnceLock::new();

/// A single entry in the preview data store.
#[allow(dead_code)]
struct PreviewDataEntry {
    /// The raw WebM bytes from the export pipeline.
    webm_data: Vec<u8>,
    /// Integrity state label ("Clean", "Partial", "Incomplete").
    integrity: String,
    /// Optional detail message for the integrity badge (e.g., "Clean — up to chunk N of M").
    detail_message: Option<String>,
}

fn init_session() -> &'static Mutex<recorder::RecordingSession> {
    SESSION.get_or_init(|| Mutex::new(recorder::RecordingSession::new()))
}

fn init_preview_store() -> &'static Mutex<HashMap<String, PreviewDataEntry>> {
    PREVIEW_DATA.get_or_init(|| Mutex::new(HashMap::new()))
}

fn init_recovery_toast() -> &'static Mutex<Option<recovery_toast::RecoveryToast>> {
    RECOVERY_TOAST.get_or_init(|| Mutex::new(None))
}

/// Scan for orphaned chunks at startup and propose recovery if found.
///
/// 1. Checks if the session is active (skip if Recording/Paused — AC15).
/// 2. Reads `chrome.storage.local` for an `in_flight` lock (AC2).
/// 3. Enumerates OPFS `capture-forge/sessions/` for orphan chunks (AC1).
/// 4. If orphans found, renders a non-modal crash recovery toast.
#[cfg(target_arch = "wasm32")]
async fn scan_and_propose_recovery() {
    use js_sys::Reflect;

    // AC2: Check chrome.storage.local for in_flight lock.
    let lock_stale = check_in_flight_lock_stale().await;

    // AC1: Scan OPFS for orphan sessions (V0.1 scaffold — returns empty).
    // Full OPFS enumeration will be implemented in Story 2.1.
    let orphan_sessions = scan_opfs_orphans().await;

    // If no orphan chunks found (V0.1 scaffold always returns empty),
    // skip recovery proposal. The in_flight lock alone does not produce
    // recoverable data — actual OPFS enumeration is required (Story 2.1).
    if orphan_sessions.is_empty() {
        oxichrome::log!("scan_and_propose_recovery: no orphan sessions found — skipping");
        return;
    }

    // Determine the most recent session for recovery proposal (AC17).
    // Note: orphan_sessions are not sorted yet — full sorting depends on
    // real OPFS enumeration with timestamps (Story 2.1).
    let session_id = orphan_sessions
        .first()
        .map(|s| s.session_id.clone())
        .expect("invariant: orphan_sessions is non-empty");

    let chunk_count = orphan_sessions
        .first()
        .map(|s| s.files.len() as u32)
        .unwrap_or(0);

    // Create and render the recovery toast.
    // AC15: Defer if session is now active (checked after await to avoid
    // holding the lock across the async boundary).
    let session_active = SESSION.get().is_some_and(|mutex| {
        mutex.lock().map_or(false, |s| s.state().is_active())
    });
    if session_active {
        oxichrome::log!("scan_and_propose_recovery: session active — deferring (checked post-await)");
        return;
    }

    // Create and render the recovery toast.
    if let Some(mutex) = SESSION.get() {
        if let Ok(mut session) = mutex.lock() {
            if let Err(e) = session.transition(recorder::SessionState::CrashRecovery) {
                oxichrome::log!(
                    "scan_and_propose_recovery: transition to CrashRecovery failed: {:?}",
                    e,
                );
                return;
            }
        }
    }

    show_recovery_toast(&session_id, chunk_count).await;
}

/// Check the chrome.storage.local in_flight lock.
///
/// Returns `true` if a stale lock was found (>30s old), meaning the session
/// potentially crashed. Returns `false` if no lock or lock is fresh.
#[cfg(target_arch = "wasm32")]
async fn check_in_flight_lock_stale() -> bool {
    use js_sys::{Reflect, Object};

    let chrome = match Reflect::get(&js_sys::global(), &"chrome".into()).ok() {
        Some(c) => c,
        None => return false,
    };
    let storage = match Reflect::get(&chrome, &"storage".into()).ok() {
        Some(s) => s,
        None => return false,
    };
    let local = match Reflect::get(&storage, &"local".into()).ok() {
        Some(l) => l,
        None => return false,
    };

    // chrome.storage.local.get(["in_flight"]) returns a Promise.
    let get_fn = match Reflect::get(&local, &"get".into()).ok() {
        Some(f) => f,
        None => return false,
    };

    let keys = js_sys::Array::new();
    keys.push(&"in_flight".into());

    let promise = match Reflect::apply(&get_fn, &local, &keys) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let result = wasm_bindgen_futures::JsFuture::from(
        js_sys::Promise::from(promise),
    )
    .await;

    match result {
        Ok(val) => {
            let obj = Object::unchecked_from_js(val);
            let in_flight = Reflect::get(&obj, &"in_flight".into()).ok();
            match in_flight {
                Some(lock) if !lock.is_undefined() => {
                    let started_at = Reflect::get(&lock, &"started_at".into())
                        .ok()
                        .and_then(|v| v.as_f64());
                    let now = js_sys::Date::now();
                    crate::recovery::is_lock_stale(started_at, now)
                }
                _ => false,
            }
        }
        Err(_) => false,
    }
}

/// Scan OPFS for orphan session directories (V0.1 scaffold).
///
/// Returns `Vec<SessionDir>`. In V0.1, this is a scaffold that returns empty
/// — the full OPFS enumeration will be implemented in Story 2.1. All recovery
/// logic (triple verification, report) is tested natively via MockFileSystem.
#[cfg(target_arch = "wasm32")]
async fn scan_opfs_orphans() -> Vec<crate::recovery::SessionDir> {
    // V0.1 scaffold: OPFS enumeration deferred to Story 2.1.
    // For now, return an empty list — no crash recovery from OPFS.
    //
    // When implemented (Story 2.1):
    //   1. Call navigator.storage.getDirectory() to get OPFS root
    //   2. Enumerate capture-forge/sessions/<sessionId>/ directories
    //   3. For each dir, collect chunk files (.bin, .written, .partial)
    //   4. Load and parse manifest.json
    //   5. Return as Vec<SessionDir> for recovery processing

    // Check chrome.storage.local for in_flight to propose recovery.
    // If in_flight lock exists and is stale, we have data to recover.
    let in_flight_data = get_in_flight_session_data().await;
    if let Some((session_id, started_at)) = in_flight_data {
        let now = js_sys::Date::now();
        if crate::recovery::is_lock_stale(Some(started_at), now) {
            // Propose recovery from in_flight data.
            // The actual OPFS data will be available once Story 2.1
            // implements the full OPFS storage path.
            oxichrome::log!(
                "scan_opfs_orphans: stale in_flight lock for session {}",
                session_id,
            );
            // Return empty for V0.1 — no actual OPFS recovery.
        }
    }

    vec![]
}

/// Read in_flight session data from chrome.storage.local.
#[cfg(target_arch = "wasm32")]
async fn get_in_flight_session_data() -> Option<(String, f64)> {
    use js_sys::{Object, Reflect};

    let chrome = Reflect::get(&js_sys::global(), &"chrome".into()).ok()?;
    let storage = Reflect::get(&chrome, &"storage".into()).ok()?;
    let local = Reflect::get(&storage, &"local".into()).ok()?;
    let get_fn = Reflect::get(&local, &"get".into()).ok()?;

    let keys = js_sys::Array::new();
    keys.push(&"in_flight".into());

    let promise = Reflect::apply(&get_fn, &local, &keys).ok()?;
    let result = wasm_bindgen_futures::JsFuture::from(
        js_sys::Promise::from(promise),
    )
    .await
    .ok()?;

    let obj = Object::unchecked_from_js(result);
    let in_flight = Reflect::get(&obj, &"in_flight".into()).ok()?;
    if in_flight.is_undefined() {
        return None;
    }

    let session_id = Reflect::get(&in_flight, &"session_id".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let started_at = Reflect::get(&in_flight, &"started_at".into())
        .ok()
        .and_then(|v| v.as_f64())?;

    Some((session_id, started_at))
}

/// Create and render the recovery toast with callbacks.
#[cfg(target_arch = "wasm32")]
async fn show_recovery_toast(session_id: &str, chunk_count: u32) {
    let sid = session_id.to_owned();

    let mut toast = recovery_toast::RecoveryToast::new();

    // Restore callback: send RESTORE_RECORDING message to the background.
    {
        let restore_sid = sid.clone();
        toast.set_on_restore(move || {
            let _ = (|| -> Option<()> {
                let runtime = js_sys::Reflect::get(
                    &js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?,
                    &"runtime".into(),
                )
                .ok()?;
                let send_msg =
                    js_sys::Reflect::get(&runtime, &"sendMessage".into()).ok()?;
                let msg = js_sys::Object::new();
                js_sys::Reflect::set(
                    &msg,
                    &"type".into(),
                    &"RESTORE_RECORDING".into(),
                )
                .ok()?;
                js_sys::Reflect::set(
                    &msg,
                    &"sessionId".into(),
                    &restore_sid,
                )
                .ok()?;
                let _ = js_sys::Reflect::apply(&send_msg, &runtime, &js_sys::Array::of1(&msg))
                    .ok()?;
                Some(())
            })();
        });
    }

    // Dismiss callback: send DISMISS_RECOVERY message.
    toast.set_on_dismiss(move || {
        let _ = (|| -> Option<()> {
            let runtime = js_sys::Reflect::get(
                &js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?,
                &"runtime".into(),
            )
            .ok()?;
            let send_msg =
                js_sys::Reflect::get(&runtime, &"sendMessage".into()).ok()?;
            let msg = js_sys::Object::new();
            js_sys::Reflect::set(
                &msg,
                &"type".into(),
                &"DISMISS_RECOVERY".into(),
            )
            .ok()?;
            let _ = js_sys::Reflect::apply(&send_msg, &runtime, &js_sys::Array::of1(&msg))
                .ok()?;
            Some(())
        })();
    });

    // Render the toast.
    if let Err(e) = toast.render() {
        oxichrome::log!(
            "show_recovery_toast: render failed for session {}: {:?}",
            sid,
            e,
        );
        return;
    }

    // Store the toast so DISMISS_RECOVERY can clean it up.
    if let Some(mutex) = RECOVERY_TOAST.get() {
        if let Ok(mut guard) = mutex.lock() {
            *guard = Some(toast);
        }
    }
}

/// Store exported preview data for a session so the preview page can retrieve it.
///
/// Called by the background orchestration after the export pipeline completes.
/// The `detail` parameter is an optional integrity detail message (e.g.,
/// "Clean — up to chunk N of M" for Partial recovery).
#[wasm_bindgen]
pub fn store_preview_data(session_id: &str, webm_data: &[u8], integrity: &str, detail: Option<String>) {
    if let Some(store) = PREVIEW_DATA.get() {
        match store.lock() {
            Ok(mut map) => {
                map.insert(
                    session_id.to_owned(),
                    PreviewDataEntry {
                        webm_data: webm_data.to_vec(),
                        integrity: integrity.to_owned(),
                        detail_message: detail,
                    },
                );
            }
            Err(_) => {
                log!("store_preview_data: mutex poisoned for session {}", session_id);
            }
        }
    } else {
        log!("store_preview_data: PREVIEW_DATA not initialised for session {}", session_id);
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

    // Initialise the recovery toast store.
    init_recovery_toast();

    // Register the runtime message handler for preview page communication.
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        use js_sys::{Array, Object, Reflect, Uint8Array};

        if let Some(chrome) = Reflect::get(&js_sys::global(), &"chrome".into()).ok() {
            if let Some(runtime) = Reflect::get(&chrome, &"runtime".into()).ok() {
                let handler = Closure::wrap(Box::new(move |message: JsValue, _sender: JsValue, send_response: JsValue| {
                    // Track whether we will call sendResponse asynchronously.
                    // Only GET_PREVIEW_DATA may respond; one-way messages do not.
                    let mut will_respond = false;

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
                                                    Reflect::set(&response, &"integrity".into(), &entry.integrity.into()).ok();
                                                    if let Some(ref detail) = entry.detail_message {
                                                        Reflect::set(&response, &"detailMessage".into(), &detail.into()).ok();
                                                    }
                                                    if let Some(sr) = send_response.dyn_ref::<js_sys::Function>() {
                                                        let _ = sr.call1(&JsValue::NULL, &response);
                                                        will_respond = true;
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // If no response was sent (missing data), send an empty
                                    // response so the caller's Promise settles.
                                    if !will_respond {
                                        if let Some(sr) = send_response.dyn_ref::<js_sys::Function>() {
                                            let err = Object::new();
                                            Reflect::set(&err, &"error".into(), &"not_found".into()).ok();
                                            let _ = sr.call1(&JsValue::NULL, &err);
                                            will_respond = true;
                                        }
                                    }
                                }
                                "DELETE_RECORDING" => {
                                    // Clean up the preview data store before transitioning.
                                    if let Some(sid) = Reflect::get(&msg_obj, &"sessionId".into())
                                        .and_then(|v| v.as_string().ok_or(wasm_bindgen::JsValue::UNDEFINED))
                                        .ok()
                                        .filter(|s: &String| !s.is_empty())
                                    {
                                        if let Some(store) = PREVIEW_DATA.get() {
                                            if let Ok(mut map) = store.lock() {
                                                map.remove(&sid);
                                            }
                                        }
                                    }
                                    // Transition session to Idle.
                                    if let Some(mutex) = SESSION.get() {
                                        if let Ok(mut session) = mutex.lock() {
                                            if let Err(e) = session.transition(recorder::SessionState::Idle) {
                                                log!("DELETE_RECORDING: transition to Idle failed: {:?}", e);
                                            }
                                        }
                                    }
                                }
                                "PREVIEW_CLOSED" => {
                                    // Transition session to Idle (no data cleanup needed).
                                    if let Some(mutex) = SESSION.get() {
                                        if let Ok(mut session) = mutex.lock() {
                                            if let Err(e) = session.transition(recorder::SessionState::Idle) {
                                                log!("PREVIEW_CLOSED: transition to Idle failed: {:?}", e);
                                            }
                                        }
                                    }
                                }
                                "RESTORE_RECORDING" => {
                                    // Clean up the toast.
                                    if let Some(mutex) = RECOVERY_TOAST.get() {
                                        if let Ok(mut guard) = mutex.lock() {
                                            if let Some(ref mut toast) = *guard {
                                                toast.remove();
                                            }
                                            *guard = None;
                                        }
                                    }

                                    // In V0.1, OPFS enumeration is not yet implemented
                                    // (Story 2.1), so no orphan data exists to recover.
                                    // The full recovery pipeline — triple verification,
                                    // export concatenation, and preview data storage —
                                    // will be wired once real OPFS data is available.
                                    //
                                    // For now, transition gracefully to Idle since there
                                    // is nothing to present in the preview page.
                                    // AC16: If the transition fails, the error is logged
                                    // and the session remains in CrashRecovery (orphan
                                    // data stays on disk — never deleted on error).
                                    if let Some(mutex) = SESSION.get() {
                                        if let Ok(mut session) = mutex.lock() {
                                            if let Err(e) = session.transition(recorder::SessionState::Idle) {
                                                log!("RESTORE_RECORDING: transition to Idle failed: {:?}", e);
                                            }
                                        }
                                    }
                                }
                                "DISMISS_RECOVERY" => {
                                    // Clean up the toast.
                                    if let Some(mutex) = RECOVERY_TOAST.get() {
                                        if let Ok(mut guard) = mutex.lock() {
                                            if let Some(ref mut toast) = *guard {
                                                toast.remove();
                                            }
                                            *guard = None;
                                        }
                                    }

                                    // Transition: CrashRecovery → Idle.
                                    if let Some(mutex) = SESSION.get() {
                                        if let Ok(mut session) = mutex.lock() {
                                            if let Err(e) = session.transition(recorder::SessionState::Idle) {
                                                log!("DISMISS_RECOVERY: transition to Idle failed: {:?}", e);
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    // Return true only if we will call sendResponse asynchronously;
                    // return undefined (JsValue::UNDEFINED) for one-way messages.
                    if will_respond {
                        wasm_bindgen::JsValue::from(true)
                    } else {
                        wasm_bindgen::JsValue::UNDEFINED
                    }
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

    // Run crash recovery scan at startup (WASM-only).
    #[cfg(target_arch = "wasm32")]
    scan_and_propose_recovery().await;
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    log!("Capture Forge installed: {:?}", details);
}
