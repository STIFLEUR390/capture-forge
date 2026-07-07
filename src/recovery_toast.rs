use crate::error::Result;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element, HtmlElement, KeyboardEvent};

// ---------------------------------------------------------------------------
// CSS — inline in the toast element (no external stylesheets in V0.1)
// ---------------------------------------------------------------------------

/// Inline CSS for the crash recovery toast.
///
/// Follows UX-DR15: bottom-center, light/dark theme, primary button + text link.
/// Accessibility: `role="alert"`, `aria-live="assertive"`, keyboard navigation.
const RECOVERY_TOAST_CSS: &str = r#"
#recovery-toast {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 20px;
  border-radius: 6px;
  border: 1px solid var(--border, #E4E4E7);
  background: var(--bg, #FFFFFF);
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  font-size: 13px;
  color: var(--fg, #1A1B1E);
  min-width: 300px;
  max-width: 480px;
}
@media (prefers-color-scheme: light) {
  #recovery-toast {
    --bg: #FFFFFF;
    --fg: #1A1B1E;
    --border: #E4E4E7;
    --primary: #2563EB;
    --primary-fg: #FFFFFF;
    --muted-fg: #71717A;
  }
}
@media (prefers-color-scheme: dark) {
  #recovery-toast {
    --bg: #27272A;
    --fg: #E4E5E7;
    --border: #3F3F46;
    --primary: #60A5FA;
    --primary-fg: #FFFFFF;
    --muted-fg: #A1A1AA;
  }
}
#recovery-toast[hidden] { display: none; }
#toast-message {
  flex: 1;
  font-weight: 450;
}
#toast-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}
#toast-restore {
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.02em;
  cursor: pointer;
  border: none;
  background: var(--primary);
  color: var(--primary-fg);
  white-space: nowrap;
}
#toast-restore:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
#toast-dismiss {
  background: transparent;
  border: none;
  color: var(--muted-fg);
  font-size: 12px;
  font-weight: 400;
  cursor: pointer;
  padding: 6px 8px;
  text-decoration: underline;
  white-space: nowrap;
}
#toast-dismiss:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
"#;

// ---------------------------------------------------------------------------
// RecoveryToast
// ---------------------------------------------------------------------------

/// A non-modal, auto-dismissing crash recovery toast.
///
/// Rendered bottom-center on any active extension surface (popup, preview, or
/// service worker page). Follows UX-DR15, UX-DR18.
///
/// # Lifecycle
///
/// 1. `new()` — create the struct.
/// 2. `render()` — inject CSS + DOM into the document, register handlers.
/// 3. User clicks Restore → `on_restore` fires.
/// 4. User clicks Dismiss (or Escape, or auto-dismiss timer) → `on_dismiss` fires.
/// 5. `remove()` / `destroy()` — clean up DOM, timer, closures.
/// 6. `Drop` — safety net if dropped without explicit cleanup.
///
/// # State guards
///
/// | method  | Allowed when    | Behaviour |
/// |---------|-----------------|-----------|
/// | render  | new only        | Injects DOM into document.body |
/// | remove  | rendered        | Removes DOM, clears timer |
pub(crate) struct RecoveryToast {
    /// Whether the toast has been rendered in the DOM.
    rendered: bool,
    /// Whether the destroy guard has been set.
    destroyed: bool,

    // Callbacks
    on_restore: Option<Box<dyn FnMut() + Send>>,
    on_dismiss: Option<Box<dyn FnMut() + Send>>,

    // DOM references
    #[cfg(target_arch = "wasm32")]
    container: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    restore_btn: Option<HtmlElement>,
    #[cfg(target_arch = "wasm32")]
    dismiss_btn: Option<HtmlElement>,
    #[cfg(target_arch = "wasm32")]
    style_el: Option<Element>,
    /// Element that was focused before the toast appeared.
    #[cfg(target_arch = "wasm32")]
    previous_focus_el: Option<Element>,

    // Auto-dismiss timer handle (setTimeout ID, stored as f64 from js_sys)
    #[cfg(target_arch = "wasm32")]
    auto_dismiss_id: Option<f64>,

    // Closures that need to stay alive for the lifetime of the toast
    #[cfg(target_arch = "wasm32")]
    _restore_closure: Option<Closure<dyn FnMut(web_sys::MouseEvent)>>,
    #[cfg(target_arch = "wasm32")]
    _dismiss_closure: Option<Closure<dyn FnMut(web_sys::MouseEvent)>>,
    #[cfg(target_arch = "wasm32")]
    _keydown_closure: Option<Closure<dyn FnMut(web_sys::KeyboardEvent)>>,
    #[cfg(target_arch = "wasm32")]
    _auto_dismiss_closure: Option<Closure<dyn FnMut()>>,
}

impl RecoveryToast {
    /// Create a new unrendered recovery toast.
    pub(crate) fn new() -> Self {
        Self {
            rendered: false,
            destroyed: false,
            on_restore: None,
            on_dismiss: None,
            #[cfg(target_arch = "wasm32")]
            container: None,
            #[cfg(target_arch = "wasm32")]
            restore_btn: None,
            #[cfg(target_arch = "wasm32")]
            dismiss_btn: None,
            #[cfg(target_arch = "wasm32")]
            style_el: None,
            #[cfg(target_arch = "wasm32")]
            previous_focus_el: None,
            #[cfg(target_arch = "wasm32")]
            auto_dismiss_id: None,
            #[cfg(target_arch = "wasm32")]
            _restore_closure: None,
            #[cfg(target_arch = "wasm32")]
            _dismiss_closure: None,
            #[cfg(target_arch = "wasm32")]
            _keydown_closure: None,
            #[cfg(target_arch = "wasm32")]
            _auto_dismiss_closure: None,
        }
    }

    // ------------------------------------------------------------------
    // Callback setters
    // ------------------------------------------------------------------

    /// Set callback fired when the user clicks Restore.
    pub(crate) fn set_on_restore<F>(&mut self, callback: F)
    where
        F: FnMut() + Send + 'static,
    {
        self.on_restore = Some(Box::new(callback));
    }

    /// Set callback fired when the user dismisses the toast.
    pub(crate) fn set_on_dismiss<F>(&mut self, callback: F)
    where
        F: FnMut() + Send + 'static,
    {
        self.on_dismiss = Some(Box::new(callback));
    }

    // ------------------------------------------------------------------
    // State accessors — pure logic, testable natively
    // ------------------------------------------------------------------

    /// Return whether the toast has been rendered.
    pub(crate) fn is_rendered(&self) -> bool {
        self.rendered
    }

    /// Return whether the toast has been destroyed.
    pub(crate) fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    // ------------------------------------------------------------------
    // Render — inject CSS and DOM into the document
    // ------------------------------------------------------------------

    /// Render the recovery toast: inject CSS + DOM, register handlers.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn render(&mut self) -> Result<()> {
        // Guard: no-op if already rendered or destroyed.
        if self.destroyed {
            oxichrome::log!("RecoveryToast::render() called after destroy — ignoring");
            return Ok(());
        }
        if self.rendered {
            oxichrome::log!("RecoveryToast::render() called when already rendered — ignoring");
            return Ok(());
        }

        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| crate::error::RecordingError::Unknown {
                details: "Cannot access document for recovery toast".into(),
            })?;

        let body = document.body().ok_or_else(|| {
            crate::error::RecordingError::Unknown {
                details: "No document body for recovery toast".into(),
            }
        })?;

        // Guard: check if a toast already exists in the DOM (AC3: only one toast).
        if document.get_element_by_id("recovery-toast").is_some() {
            oxichrome::log!("RecoveryToast: toast already exists in DOM — skipping render");
            return Ok(());
        }

        // Inject inline CSS (idempotent: skip if style already injected).
        let style = match document.get_element_by_id("recovery-toast-style") {
            Some(s) => s,
            None => {
                let s = document.create_element("style")?;
                s.set_attribute("id", "recovery-toast-style")?;
                s.set_text_content(Some(RECOVERY_TOAST_CSS));
                document.head()
                    .ok_or_else(|| crate::error::RecordingError::Unknown {
                        details: "Cannot access document head for recovery toast".into(),
                    })?
                    .append_child(&s)?;
                s
            }
        };

        // Create the toast container.
        let container = document.create_element("div")?;
        container.set_attribute("id", "recovery-toast")?;
        container.set_attribute("role", "alert")?;
        container.set_attribute("aria-live", "assertive")?;
        container.set_attribute("tabindex", "-1")?;

        // Message.
        let message = document.create_element("div")?;
        message.set_attribute("id", "toast-message")?;
        message.set_text_content(Some("A previous recording session was found."));
        container.append_child(&message)?;

        // Actions.
        let actions = document.create_element("div")?;
        actions.set_attribute("id", "toast-actions")?;

        let restore_btn = document.create_element("button")?;
        restore_btn.set_attribute("id", "toast-restore")?;
        restore_btn.set_attribute("aria-label", "Restore recording")?;
        restore_btn.set_text_content(Some("Restore"));
        actions.append_child(&restore_btn)?;

        let dismiss_btn = document.create_element("button")?;
        dismiss_btn.set_attribute("id", "toast-dismiss")?;
        dismiss_btn.set_attribute("aria-label", "Dismiss recovery")?;
        dismiss_btn.set_text_content(Some("Dismiss"));
        actions.append_child(&dismiss_btn)?;

        container.append_child(&actions)?;

        // Append to body.
        body.append_child(&container)?;

        // Store element references.
        self.container = Some(container);
        self.restore_btn = Some(restore_btn.unchecked_into::<HtmlElement>());
        self.dismiss_btn = Some(dismiss_btn.unchecked_into::<HtmlElement>());
        self.style_el = Some(style);

        // Register event handlers.
        self.register_restore_handler()?;
        self.register_dismiss_handler()?;
        self.register_keydown_handler(&document)?;
        self.start_auto_dismiss_timer(&document);

        // Save the previously focused element for focus restoration on dismiss (AC5).
        self.previous_focus_el = document.active_element();

        // Focus the toast container.
        if let Some(ref el) = self.container {
            let _ = el.cast::<HtmlElement>().map(|e| e.focus());
        }

        self.rendered = true;

        Ok(())
    }

    /// Native no-op — DOM cannot be rendered outside a browser.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn render(&mut self) -> Result<()> {
        if self.destroyed {
            return Ok(());
        }
        self.rendered = true;
        Ok(())
    }

    // ------------------------------------------------------------------
    // WASM: Event handler registration
    // ------------------------------------------------------------------

    /// Register the Restore button click handler.
    #[cfg(target_arch = "wasm32")]
    fn register_restore_handler(&mut self) -> Result<()> {
        let btn = self.restore_btn.clone();
        let restore_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_restore as *mut _;
        let dismiss_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_dismiss as *mut _;

        let cb = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            event.stop_propagation();

            // Cancel auto-dismiss timer.
            if let Some(window) = web_sys::window() {
                if let Some(timer_id) = Self::get_auto_dismiss_id_from_dom() {
                    window.clear_timeout_with_handle(timer_id);
                }
            }

            // Fire restore callback.
            if let Some(ref mut cb) = unsafe { &mut *restore_ptr } {
                cb();
            }
        }) as Box<dyn FnMut(web_sys::MouseEvent)>);

        if let Some(ref btn_el) = btn {
            btn_el
                .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register toast restore handler".into(),
                })?;
        }

        self._restore_closure = Some(cb);
        Ok(())
    }

    /// Register the Dismiss button click handler.
    #[cfg(target_arch = "wasm32")]
    fn register_dismiss_handler(&mut self) -> Result<()> {
        let btn = self.dismiss_btn.clone();
        let dismiss_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_dismiss as *mut _;

        let cb = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            event.stop_propagation();

            // Cancel auto-dismiss timer.
            if let Some(window) = web_sys::window() {
                if let Some(timer_id) = Self::get_auto_dismiss_id_from_dom() {
                    window.clear_timeout_with_handle(timer_id);
                }
            }

            // Fire dismiss callback.
            if let Some(ref mut cb) = unsafe { &mut *dismiss_ptr } {
                cb();
            }
        }) as Box<dyn FnMut(web_sys::MouseEvent)>);

        if let Some(ref btn_el) = btn {
            btn_el
                .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register toast dismiss handler".into(),
                })?;
        }

        self._dismiss_closure = Some(cb);
        Ok(())
    }

    /// Register the keydown handler for keyboard navigation.
    ///
    /// - Escape: dismiss the toast.
    /// - Tab: cycle through Restore → Dismiss.
    /// - Enter: activate the focused button.
    #[cfg(target_arch = "wasm32")]
    fn register_keydown_handler(&mut self, document: &Document) -> Result<()> {
        let dismiss_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_dismiss as *mut _;

        let cb = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                event.stop_propagation();

                // Cancel auto-dismiss timer.
                if let Some(window) = web_sys::window() {
                    if let Some(timer_id) = Self::get_auto_dismiss_id_from_dom() {
                        window.clear_timeout_with_handle(timer_id);
                    }
                }

                // Fire dismiss callback.
                if let Some(ref mut cb) = unsafe { &mut *dismiss_ptr } {
                    cb();
                }
            }
            // Tab and Enter are handled natively by the browser's focus management
            // and button activation — no custom handling needed.
        }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

        document
            .add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())
            .map_err(|_| crate::error::RecordingError::Unknown {
                details: "Failed to register toast keydown handler".into(),
            })?;

        self._keydown_closure = Some(cb);
        Ok(())
    }

    /// Start the 8-second auto-dismiss timer.
    #[cfg(target_arch = "wasm32")]
    fn start_auto_dismiss_timer(&mut self, document: &Document) {
        let dismiss_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_dismiss as *mut _;

        let cb = Closure::wrap(Box::new(move || {
            // Timer fired — auto-dismiss.
            if let Some(ref mut cb) = unsafe { &mut *dismiss_ptr } {
                cb();
            }
        }) as Box<dyn FnMut()>);

        if let Some(window) = web_sys::window() {
            let id = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    8000,
                )
                .ok();
            self.auto_dismiss_id = id;

            // Store the timer ID on the DOM element so handlers can cancel it.
            if let Some(ref container) = self.container {
                if let Some(timer_id) = self.auto_dismiss_id {
                    let _ = container.set_attribute("data-dismiss-id", &timer_id.to_string());
                }
            }
        }

        self._auto_dismiss_closure = Some(cb);
    }

    /// Read the auto-dismiss timer ID from the toast's data attribute.
    ///
    /// The timer ID is stored in a data attribute on the toast container so
    /// the handlers can find it without a reference to the struct.
    #[cfg(target_arch = "wasm32")]
    fn get_auto_dismiss_id_from_dom() -> Option<f64> {
        let doc = web_sys::window()?.document()?;
        let toast = doc.get_element_by_id("recovery-toast")?;
        let id_str = toast.get_attribute("data-dismiss-id")?;
        id_str.parse::<f64>().ok()
    }

    // ------------------------------------------------------------------
    // Remove / Destroy
    // ------------------------------------------------------------------

    /// Remove the toast from the DOM and clean up resources.
    ///
    /// Cancels the auto-dismiss timer, removes DOM elements, and resets state.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn remove(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.rendered = false;

        // Cancel auto-dismiss timer.
        if let Some(id) = self.auto_dismiss_id.take() {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(id);
            }
        }

        // Remove the keydown listener.
        if let Some(closure) = self._keydown_closure.take() {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                let _ = doc.remove_event_listener_with_callback(
                    "keydown",
                    closure.as_ref().unchecked_ref(),
                );
            }
        }

        // Drop all closures.
        self._restore_closure.take();
        self._dismiss_closure.take();
        self._auto_dismiss_closure.take();

        // Restore focus to the previously focused element (or body).
        if let Some(prev) = self.previous_focus_el.take() {
            let _ = prev.cast::<HtmlElement>().map(|e| e.focus());
        } else {
            // Fallback: focus document body.
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                let _ = doc.body().map(|b| b.cast::<HtmlElement>().map(|e| e.focus()));
            }
        }

        // Remove the toast container from the DOM.
        if let Some(container) = self.container.take() {
            let _ = container.remove();
        }

        // Clear element references.
        self.restore_btn = None;
        self.dismiss_btn = None;
        self.style_el = None;

        // Clear callbacks.
        self.on_restore = None;
        self.on_dismiss = None;
    }

    /// Native no-op.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn remove(&mut self) {
        self.destroyed = true;
        self.rendered = false;
        self.on_restore = None;
        self.on_dismiss = None;
    }
}

impl Default for RecoveryToast {
    fn default() -> Self {
        Self::new()
    }
}

/// Drop safety: clean up DOM, timer, and closures if dropped without an
/// explicit `remove()` call.
#[cfg(target_arch = "wasm32")]
impl Drop for RecoveryToast {
    fn drop(&mut self) {
        if !self.destroyed {
            oxichrome::log!("RecoveryToast dropped without remove() — cleaning up");
            self.remove();
        }
    }
}

// ---------------------------------------------------------------------------
// Native unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_initial_state() {
        let toast = RecoveryToast::new();
        assert!(!toast.is_rendered());
        assert!(!toast.is_destroyed());
    }

    #[test]
    fn test_toast_show_sets_visible() {
        let mut toast = RecoveryToast::new();
        toast.render().expect("render should succeed");
        assert!(toast.is_rendered());
        assert!(!toast.is_destroyed());
    }

    #[test]
    fn test_toast_dismiss_hides() {
        let mut toast = RecoveryToast::new();
        toast.render().expect("render should succeed");
        assert!(toast.is_rendered());

        toast.remove();
        assert!(!toast.is_rendered());
        assert!(toast.is_destroyed());
    }

    #[test]
    fn test_toast_no_double_render() {
        let mut toast = RecoveryToast::new();
        toast.render().expect("first render should succeed");
        assert!(toast.is_rendered());

        // Second render should be a no-op.
        toast.render().expect("second render should be no-op");
        assert!(toast.is_rendered());
    }

    #[test]
    fn test_toast_remove_twice_noop() {
        let mut toast = RecoveryToast::new();
        toast.render().expect("render");
        toast.remove();
        assert!(toast.is_destroyed());

        // Second remove should be a no-op.
        toast.remove();
        assert!(toast.is_destroyed());
    }

    #[test]
    fn test_toast_default() {
        let toast = RecoveryToast::default();
        assert!(!toast.is_rendered());
        assert!(!toast.is_destroyed());
    }

    #[test]
    fn test_toast_restore_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut toast = RecoveryToast::new();
        let called = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&called);
        toast.set_on_restore(move || {
            c.store(true, Ordering::SeqCst);
        });

        // Simulate restore click by invoking callback.
        if let Some(ref mut cb) = toast.on_restore {
            cb();
        }
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_toast_dismiss_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut toast = RecoveryToast::new();
        let called = Arc::new(AtomicBool::new(false));
        let c = Arc::clone(&called);
        toast.set_on_dismiss(move || {
            c.store(true, Ordering::SeqCst);
        });

        // Simulate dismiss by invoking callback.
        if let Some(ref mut cb) = toast.on_dismiss {
            cb();
        }
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_toast_render_after_remove_allowed() {
        let mut toast = RecoveryToast::new();
        toast.render().expect("first render");
        toast.remove();
        assert!(toast.is_destroyed());

        // After remove, render should be no-op (guard against re-use).
        toast.render().expect("render after remove should be no-op");
        assert!(!toast.is_rendered(), "should not re-render after destroy");
    }
}
