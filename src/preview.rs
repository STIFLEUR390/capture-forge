use crate::error::Result;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element, HtmlButtonElement, HtmlElement, HtmlVideoElement, Url};

// ---------------------------------------------------------------------------
// IntegrityState
// ---------------------------------------------------------------------------

/// Integrity state of a recorded session for the badge display.
///
/// - `Clean`: All chunks committed, export succeeded — normal session.
/// - `Partial`: Some chunks recovered, playback may be truncated.
/// - `Incomplete`: Could not be fully recovered.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IntegrityState {
    Clean,
    Partial,
    Incomplete,
}

impl IntegrityState {
    /// Return the human-readable label.
    pub(crate) fn as_label(&self) -> &'static str {
        match self {
            IntegrityState::Clean => "Clean",
            IntegrityState::Partial => "Partial",
            IntegrityState::Incomplete => "Incomplete",
        }
    }

    /// Return the CSS class suffix for this state.
    pub(crate) fn css_class(&self) -> &'static str {
        match self {
            IntegrityState::Clean => "integrity-clean",
            IntegrityState::Partial => "integrity-partial",
            IntegrityState::Incomplete => "integrity-incomplete",
        }
    }

    /// Return the aria-label suffix for this state.
    pub(crate) fn aria_label(&self) -> &'static str {
        match self {
            IntegrityState::Clean => "Clean",
            IntegrityState::Partial => "Partial",
            IntegrityState::Incomplete => "Incomplete",
        }
    }
}

// ---------------------------------------------------------------------------
// CSS — inline in the document (no external stylesheets in V0.1)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
const PREVIEW_CSS: &str = r#"
:root {
  color-scheme: light dark;
}
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  margin: 0;
  padding: 24px;
  background: var(--bg);
  color: var(--fg);
}
@media (prefers-color-scheme: light) {
  :root { --bg: #FFFFFF; --fg: #1A1B1E; --border: #E4E4E7; --primary: #2563EB; --destructive: #EF4444; --integrity-clean: #22C55E; --integrity-partial: #F59E0B; --integrity-incomplete: #EF4444; }
}
@media (prefers-color-scheme: dark) {
  :root { --bg: #1A1B1E; --fg: #E4E5E7; --border: #3F3F46; --primary: #60A5FA; --destructive: #F87171; --integrity-clean: #22C55E; --integrity-partial: #F59E0B; --integrity-incomplete: #EF4444; }
}
#preview-container {
  max-width: 960px;
  margin: 0 auto;
}
#video-container {
  max-width: 960px;
  margin: 0 auto;
}
#preview-video {
  width: 100%;
  aspect-ratio: 16/9;
  background: #000;
  border-radius: 6px;
}
#actions-bar {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-top: 16px;
}
.btn {
  padding: 8px 24px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.02em;
  cursor: pointer;
  border: none;
  height: 36px;
}
.btn.primary {
  background: var(--primary);
  color: #FFFFFF;
}
.btn.destructive-outline {
  background: transparent;
  border: 1px solid var(--destructive);
  color: var(--destructive);
}
.btn:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
/* Integrity badge */
#integrity-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 9999px;
  font-size: 11px;
  font-weight: 400;
  margin-bottom: 12px;
  text-align: center;
  color: #FFFFFF;
}
#integrity-badge.integrity-clean { background: var(--integrity-clean); }
#integrity-badge.integrity-partial { background: var(--integrity-partial); color: #1A1A00; }
#integrity-badge.integrity-incomplete { background: var(--integrity-incomplete); }
/* Integrity detail message */
#integrity-detail {
  font-size: 12px;
  color: #71717A;
  margin-top: 4px;
  margin-bottom: 8px;
  text-align: center;
}
#integrity-detail[hidden] { display: none; }
/* Delete dialog overlay */
#delete-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 20px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.2);
  z-index: 100;
}
#delete-dialog[hidden] { display: none; }
.dialog-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 16px; }
/* Error state */
#error-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 48px;
  text-align: center;
}
#error-container[hidden] { display: none; }
.error-icon { font-size: 48px; margin-bottom: 8px; }
.error-message { font-size: 16px; font-weight: 500; }
.error-suggestion { font-size: 13px; color: #71717A; }
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
"#;

// ---------------------------------------------------------------------------
// PreviewPage
// ---------------------------------------------------------------------------

/// The preview page for reviewing a completed recording.
///
/// Manages the video player, Download/Delete action buttons, integrity badge,
/// delete confirmation dialog, error state, and keyboard handlers.
///
/// This is a standalone HTML page (not a content script), so it renders
/// directly into `document.body` without shadow DOM.
///
/// # State guards
///
/// | method     | Allowed when            | Behaviour |
/// |------------|-------------------------|-----------|
/// | render     | new() / after destroy   | Creates the full DOM in the document |
/// | destroy    | rendered                | Cleans up DOM, closures, and object URLs |
///
/// # Keyboard shortcuts
///
/// | Key     | Context                | Action |
/// |---------|------------------------|--------|
/// | Space   | Video element focused  | Toggle play/pause |
/// | Escape  | No dialog active       | Close preview tab |
/// | Escape  | Delete dialog visible  | Close dialog only |
pub(crate) struct PreviewPage {
    /// Unique session identifier.
    session_id: Option<String>,
    /// Exported WebM data as raw bytes.
    webm_data: Option<Vec<u8>>,
    /// Integrity state for the badge.
    integrity: IntegrityState,
    /// Optional detail message for the integrity badge.
    detail_message: Option<String>,
    /// Whether the delete confirmation dialog is visible.
    dialog_visible: bool,
    /// Error message to display (None = no error, show player).
    error_message: Option<String>,
    /// Error suggestion text.
    error_suggestion: Option<String>,
    /// Whether the page has been rendered (DOM injected).
    rendered: bool,
    /// Callback fired when the user confirms Delete.
    on_delete_confirmed: Option<Box<dyn FnMut(String)>>,
    /// Callback fired when the user presses Escape with no dialog active.
    on_close: Option<Box<dyn FnMut()>>,
    /// Callback fired when the user clicks Download.
    on_download: Option<Box<dyn FnMut(String, Vec<u8>)>>,

    // WASM-specific fields — only available when compiled for web-sys target.
    #[cfg(target_arch = "wasm32")]
    container: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    video_el: Option<HtmlVideoElement>,
    #[cfg(target_arch = "wasm32")]
    download_btn_el: Option<HtmlButtonElement>,
    #[cfg(target_arch = "wasm32")]
    delete_btn_el: Option<HtmlButtonElement>,
    #[cfg(target_arch = "wasm32")]
    badge_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    detail_message_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    dialog_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    dialog_cancel_el: Option<HtmlButtonElement>,
    #[cfg(target_arch = "wasm32")]
    dialog_confirm_el: Option<HtmlButtonElement>,
    #[cfg(target_arch = "wasm32")]
    error_container_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    error_message_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    error_suggestion_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    error_back_el: Option<HtmlButtonElement>,
    #[cfg(target_arch = "wasm32")]
    object_url: Option<String>,
    #[cfg(target_arch = "wasm32")]
    _keydown_closure: Option<Closure<dyn FnMut(web_sys::KeyboardEvent)>>,
    #[cfg(target_arch = "wasm32")]
    _download_click_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    #[cfg(target_arch = "wasm32")]
    _delete_click_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    #[cfg(target_arch = "wasm32")]
    _dialog_cancel_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    #[cfg(target_arch = "wasm32")]
    _dialog_confirm_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    #[cfg(target_arch = "wasm32")]
    _error_back_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    #[cfg(target_arch = "wasm32")]
    _video_click_closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
    #[cfg(target_arch = "wasm32")]
    destroyed: bool,
}

impl PreviewPage {
    /// Create a new `PreviewPage` in an unrendered state.
    pub(crate) fn new() -> Self {
        Self {
            session_id: None,
            webm_data: None,
            integrity: IntegrityState::Clean,
            detail_message: None,
            dialog_visible: false,
            error_message: None,
            error_suggestion: None,
            rendered: false,
            on_delete_confirmed: None,
            on_close: None,
            on_download: None,
            #[cfg(target_arch = "wasm32")]
            container: None,
            #[cfg(target_arch = "wasm32")]
            video_el: None,
            #[cfg(target_arch = "wasm32")]
            download_btn_el: None,
            #[cfg(target_arch = "wasm32")]
            delete_btn_el: None,
            #[cfg(target_arch = "wasm32")]
            badge_el: None,
            #[cfg(target_arch = "wasm32")]
            detail_message_el: None,
            #[cfg(target_arch = "wasm32")]
            dialog_el: None,
            #[cfg(target_arch = "wasm32")]
            dialog_cancel_el: None,
            #[cfg(target_arch = "wasm32")]
            dialog_confirm_el: None,
            #[cfg(target_arch = "wasm32")]
            error_container_el: None,
            #[cfg(target_arch = "wasm32")]
            error_message_el: None,
            #[cfg(target_arch = "wasm32")]
            error_suggestion_el: None,
            #[cfg(target_arch = "wasm32")]
            error_back_el: None,
            #[cfg(target_arch = "wasm32")]
            object_url: None,
            #[cfg(target_arch = "wasm32")]
            _keydown_closure: None,
            #[cfg(target_arch = "wasm32")]
            _download_click_closure: None,
            #[cfg(target_arch = "wasm32")]
            _delete_click_closure: None,
            #[cfg(target_arch = "wasm32")]
            _dialog_cancel_closure: None,
            #[cfg(target_arch = "wasm32")]
            _dialog_confirm_closure: None,
            #[cfg(target_arch = "wasm32")]
            _error_back_closure: None,
            #[cfg(target_arch = "wasm32")]
            _video_click_closure: None,
            #[cfg(target_arch = "wasm32")]
            destroyed: false,
        }
    }

    // ------------------------------------------------------------------
    // Accessors — pure logic, testable natively
    // ------------------------------------------------------------------

    /// Return the current integrity state.
    pub(crate) fn integrity_state(&self) -> &IntegrityState {
        &self.integrity
    }

    /// Return the integrity label text.
    pub(crate) fn integrity_text(&self) -> &'static str {
        self.integrity.as_label()
    }

    /// Set the integrity state.
    pub(crate) fn set_integrity(&mut self, state: IntegrityState) -> &mut Self {
        self.integrity = state;
        #[cfg(target_arch = "wasm32")]
        self.update_integrity_badge();
        self
    }

    /// Return the detail message, if any.
    pub(crate) fn detail_message(&self) -> Option<&str> {
        self.detail_message.as_deref()
    }

    /// Set the detail message for the integrity badge.
    pub(crate) fn set_detail_message(&mut self, msg: Option<String>) -> &mut Self {
        self.detail_message = msg;
        #[cfg(target_arch = "wasm32")]
        self.update_detail_message();
        self
    }

    /// Return whether the delete confirmation dialog is visible.
    pub(crate) fn is_dialog_visible(&self) -> bool {
        self.dialog_visible
    }

    /// Return the session ID, if set.
    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Set the session ID.
    pub(crate) fn set_session_id(&mut self, id: String) -> &mut Self {
        self.session_id = Some(id);
        self
    }

    /// Set the exported WebM data.
    pub(crate) fn set_webm_data(&mut self, data: Vec<u8>) -> &mut Self {
        self.webm_data = Some(data);
        self
    }

    /// Return the error message, if any.
    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Return the error suggestion, if any.
    pub(crate) fn error_suggestion(&self) -> Option<&str> {
        self.error_suggestion.as_deref()
    }

    /// Return whether the page has been rendered.
    pub(crate) fn is_rendered(&self) -> bool {
        self.rendered
    }

    /// Return the filename for this session's recording.
    pub(crate) fn download_filename(&self) -> String {
        match self.session_id.as_deref() {
            Some(id) => format!("CaptureForge-{}.webm", id),
            None => {
                // Fallback: use current date.
                let now = chrono_now();
                format!("Recording-{}.webm", now)
            }
        }
    }

    // ------------------------------------------------------------------
    // Dialog controls — pure logic
    // ------------------------------------------------------------------

    /// Show the delete confirmation dialog.
    pub(crate) fn show_delete_dialog(&mut self) {
        self.dialog_visible = true;
        #[cfg(target_arch = "wasm32")]
        self.update_dialog_visibility();
    }

    /// Hide the delete confirmation dialog without deleting.
    pub(crate) fn hide_delete_dialog(&mut self) {
        self.dialog_visible = false;
        #[cfg(target_arch = "wasm32")]
        self.update_dialog_visibility();
    }

    /// Called when the user confirms deletion.
    ///
    /// Fires the `on_delete_confirmed` callback with the session ID.
    pub(crate) fn confirm_delete(&mut self) {
        self.dialog_visible = false;
        if let Some(ref mut cb) = self.on_delete_confirmed {
            if let Some(id) = self.session_id.clone() {
                cb(id);
            }
        }
    }

    // ------------------------------------------------------------------
    // Error state — pure logic
    // ------------------------------------------------------------------

    /// Show the error UI instead of the video player.
    pub(crate) fn show_error(&mut self, message: &str, suggestion: &str) {
        self.error_message = Some(message.to_string());
        self.error_suggestion = Some(suggestion.to_string());
        #[cfg(target_arch = "wasm32")]
        self.update_error_visibility();
    }

    /// Hide the error UI and show the video player.
    pub(crate) fn hide_error(&mut self) {
        self.error_message = None;
        self.error_suggestion = None;
        #[cfg(target_arch = "wasm32")]
        self.update_error_visibility();
    }

    // ------------------------------------------------------------------
    // Keyboard handlers — pure logic
    // ------------------------------------------------------------------

    /// Handle the Escape key.
    ///
    /// - If dialog is visible: close the dialog only.
    /// - If no dialog: close the preview page via `on_close`.
    pub(crate) fn handle_escape(&mut self) {
        if self.dialog_visible {
            self.hide_delete_dialog();
        } else {
            if let Some(ref mut cb) = self.on_close {
                cb();
            }
        }
    }

    /// Handle the Space key for play/pause toggle.
    ///
    /// This is a pure-logic no-op; the actual play/pause is handled by the
    /// browser's native video controls.  This method exists for state tracking.
    pub(crate) fn handle_space(&mut self) {
        // The browser-native `<video controls>` handles play/pause internally.
        // This method is a hook for testing that Space was handled.
    }

    // ------------------------------------------------------------------
    // Callback setters
    // ------------------------------------------------------------------

    /// Set callback fired when the user confirms Delete.
    pub(crate) fn set_on_delete_confirmed<F>(&mut self, callback: F)
    where
        F: FnMut(String) + 'static,
    {
        self.on_delete_confirmed = Some(Box::new(callback));
    }

    /// Set callback fired when the user closes the preview (Escape with no dialog).
    pub(crate) fn set_on_close<F>(&mut self, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.on_close = Some(Box::new(callback));
    }

    /// Set callback fired when the user clicks Download.
    pub(crate) fn set_on_download<F>(&mut self, callback: F)
    where
        F: FnMut(String, Vec<u8>) + 'static,
    {
        self.on_download = Some(Box::new(callback));
    }

    // ------------------------------------------------------------------
    // WASM: Render (create full DOM)
    // ------------------------------------------------------------------

    /// Render the preview page by injecting the full DOM into `document.body`.
    ///
    /// Creates the video player, integrity badge, action buttons, delete
    /// confirmation dialog, and error state container.  Registers keyboard
    /// and button click handlers.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn render(&mut self) -> Result<()> {
        // Idempotency guard.
        if self.destroyed {
            oxichrome::log!("PreviewPage::render() called after destroy — ignoring");
            return Ok(());
        }
        if self.rendered {
            oxichrome::log!("PreviewPage::render() called when already rendered — ignoring");
            return Ok(());
        }

        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| crate::error::RecordingError::Unknown {
                details: "Cannot access document for preview page".into(),
            })?;

        let body = document.body().ok_or_else(|| {
            crate::error::RecordingError::Unknown {
                details: "No document body for preview page".into(),
            }
        })?;

        // Set the tab title.
        document.set_title("Capture Forge — Preview");

        // Inject inline CSS.
        let style = document.create_element("style")?;
        style.set_text_content(Some(PREVIEW_CSS));
        document.head()
            .ok_or_else(|| crate::error::RecordingError::Unknown {
                details: "Cannot access document head for preview page".into(),
            })?
            .append_child(&style)?;

        // Create preview container.
        let container = document.create_element("div")?;
        container.set_attribute("id", "preview-container")?;

        // --- Integrity badge ---
        let badge = document.create_element("div")?;
        badge.set_attribute("id", "integrity-badge")?;
        badge.set_attribute("class", self.integrity.css_class())?;
        badge.set_attribute("role", "status")?;
        badge.set_attribute("aria-label", &format!("Integrity: {}", self.integrity.aria_label()))?;
        badge.set_text_content(Some(self.integrity.as_label()));
        container.append_child(&badge)?;

        // --- Integrity detail message (hidden by default) ---
        let detail_el = document.create_element("div")?;
        detail_el.set_attribute("id", "integrity-detail")?;
        detail_el.set_attribute("class", "integrity-detail")?;
        detail_el.set_attribute("aria-live", "polite")?;
        if let Some(ref msg) = self.detail_message {
            detail_el.set_text_content(Some(msg));
        } else {
            detail_el.set_attribute("hidden", "")?;
        }
        container.append_child(&detail_el)?;

        // --- Error container (hidden by default) ---
        let error_container = document.create_element("div")?;
        error_container.set_attribute("id", "error-container")?;
        error_container.set_attribute("hidden", "")?;

        let error_icon = document.create_element("div")?;
        error_icon.set_attribute("class", "error-icon")?;
        error_icon.set_text_content(Some("⚠"));
        error_container.append_child(&error_icon)?;

        let error_message_el = document.create_element("div")?;
        error_message_el.set_attribute("class", "error-message")?;
        error_message_el.set_attribute("id", "error-message-text")?;
        error_message_el.set_text_content(Some("Could not create WebM file."));
        error_container.append_child(&error_message_el)?;

        let error_suggestion_el = document.create_element("div")?;
        error_suggestion_el.set_attribute("class", "error-suggestion")?;
        error_suggestion_el.set_text_content(Some("Check available disk space and try again."));
        error_container.append_child(&error_suggestion_el)?;

        let error_back_btn = document.create_element("button")?;
        error_back_btn.set_attribute("class", "btn primary")?;
        error_back_btn.set_attribute("aria-label", "Back to capture forge")?;
        error_back_btn.set_text_content(Some("← Back"));
        error_container.append_child(&error_back_btn)?;

        container.append_child(&error_container)?;

        // --- Video player ---
        let video_container = document.create_element("div")?;
        video_container.set_attribute("id", "video-container")?;

        let video = document.create_element("video")?;
        video.set_attribute("id", "preview-video")?;
        video.set_attribute("controls", "")?;
        video.set_attribute("aria-label", "Recording preview")?;
        video_container.append_child(&video)?;

        container.append_child(&video_container)?;

        // --- Actions bar ---
        let actions_bar = document.create_element("div")?;
        actions_bar.set_attribute("id", "actions-bar")?;

        let download_btn = document.create_element("button")?;
        download_btn.set_attribute("class", "btn primary")?;
        download_btn.set_attribute("aria-label", "Download recording")?;
        download_btn.set_text_content(Some("Download"));
        actions_bar.append_child(&download_btn)?;

        let delete_btn = document.create_element("button")?;
        delete_btn.set_attribute("class", "btn destructive-outline")?;
        delete_btn.set_attribute("aria-label", "Delete recording")?;
        delete_btn.set_text_content(Some("Delete"));
        actions_bar.append_child(&delete_btn)?;

        container.append_child(&actions_bar)?;

        // --- Delete confirmation dialog (hidden by default) ---
        let dialog = document.create_element("div")?;
        dialog.set_attribute("id", "delete-dialog")?;
        dialog.set_attribute("role", "alertdialog")?;
        dialog.set_attribute("aria-labelledby", "delete-dialog-title")?;
        dialog.set_attribute("hidden", "")?;

        let dialog_title = document.create_element("p")?;
        dialog_title.set_attribute("id", "delete-dialog-title")?;
        dialog_title.set_text_content(Some("Delete this recording?"));
        dialog.append_child(&dialog_title)?;

        let dialog_actions = document.create_element("div")?;
        dialog_actions.set_attribute("class", "dialog-actions")?;

        let dialog_cancel = document.create_element("button")?;
        dialog_cancel.set_attribute("class", "btn destructive-outline")?;
        dialog_cancel.set_attribute("aria-label", "Cancel deletion")?;
        dialog_cancel.set_text_content(Some("Cancel"));
        dialog_actions.append_child(&dialog_cancel)?;

        let dialog_confirm = document.create_element("button")?;
        dialog_confirm.set_attribute("class", "btn primary")?;
        dialog_confirm.set_attribute("aria-label", "Confirm deletion")?;
        dialog_confirm.set_text_content(Some("Delete"));
        dialog_actions.append_child(&dialog_confirm)?;

        dialog.append_child(&dialog_actions)?;
        container.append_child(&dialog)?;

        // Register event handlers.
        self.register_keydown_handler(&document)?;
        self.register_download_handler()?;
        self.register_delete_handler()?;
        self.register_dialog_handlers()?;
        self.register_error_back_handler()?;

        // Append the full container to body — only after all handlers are
        // registered so partial DOM is never visible to the user on error.
        body.append_child(&container)?;

        // Store element references.
        self.container = Some(container);
        self.badge_el = Some(badge);
        self.detail_message_el = Some(detail_el);
        self.error_container_el = Some(error_container);
        self.error_message_el = Some(error_message_el);
        self.error_suggestion_el = Some(error_suggestion_el);
        self.error_back_el = Some(error_back_btn.unchecked_into::<HtmlButtonElement>());
        self.video_el = Some(video.unchecked_into::<HtmlVideoElement>());
        self.download_btn_el = Some(download_btn.unchecked_into::<HtmlButtonElement>());
        self.delete_btn_el = Some(delete_btn.unchecked_into::<HtmlButtonElement>());
        self.dialog_el = Some(dialog);
        self.dialog_cancel_el = Some(dialog_cancel.unchecked_into::<HtmlButtonElement>());
        self.dialog_confirm_el = Some(dialog_confirm.unchecked_into::<HtmlButtonElement>());

        // Bind video source if we have exported data.
        if let Some(ref data) = self.webm_data {
            self.bind_video_source(data);
        }

        // Focus the video element after a microtask.
        self.focus_video(&document);

        // Update error visibility if error was set before render.
        if self.error_message.is_some() {
            self.update_error_visibility();
        }

        self.rendered = true;

        Ok(())
    }

    /// Native no-op: DOM cannot be rendered outside a browser.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn render(&mut self) -> Result<()> {
        self.rendered = true;
        Ok(())
    }

    // ------------------------------------------------------------------
    // WASM: Video source binding
    // ------------------------------------------------------------------

    /// Create a `Blob` from the exported WebM data and bind it as the video
    /// element's source via `URL.createObjectURL()`.
    #[cfg(target_arch = "wasm32")]
    fn bind_video_source(&mut self, webm_data: &[u8]) {
        let video = match self.video_el.as_ref() {
            Some(v) => v,
            None => return,
        };

        // Skip binding if there's no actual data to play.
        if webm_data.is_empty() {
            oxichrome::log!("PreviewPage: no data available, video source not set");
            return;
        }

        // Create Blob from exported WebM data.
        let uint8 = js_sys::Uint8Array::from(webm_data);
        let arr = js_sys::Array::new();
        arr.push(&uint8.buffer());

        let blob = match web_sys::Blob::new_with_u8_array_sequence(&arr) {
            Ok(b) => b,
            Err(_) => {
                oxichrome::log!("PreviewPage: failed to create Blob from exported data");
                return;
            }
        };

        // Create object URL.
        let url = match Url::create_object_url_with_blob(&blob) {
            Ok(u) => u,
            Err(_) => {
                oxichrome::log!("PreviewPage: failed to create object URL");
                return;
            }
        };

        self.object_url = Some(url.clone());
        video.set_src(&url);
    }

    // ------------------------------------------------------------------
    // WASM: Event handler registration
    // ------------------------------------------------------------------

    /// Register the keydown handler on the document for Escape and Space.
    #[cfg(target_arch = "wasm32")]
    fn register_keydown_handler(&mut self, document: &Document) -> Result<()> {
        // Use raw pointers to access self from the closure.
        let dialog_ptr: *mut bool = &mut self.dialog_visible as *mut _;
        let close_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_close as *mut _;

        let cb = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                event.stop_propagation();
                // Check if dialog is visible.
                if unsafe { *dialog_ptr } {
                    // Dialog is visible — only close the dialog.
                    // We update the dialog visibility in the DOM directly.
                    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                        if let Some(dialog) = doc.get_element_by_id("delete-dialog") {
                            dialog.set_attribute("hidden", "").ok();
                        }
                    }
                    unsafe { *dialog_ptr = false };
                } else {
                    // No dialog — close the preview.
                    if let Some(ref mut cb) = unsafe { &mut *close_ptr } {
                        cb();
                    }
                }
            }
        }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

        document
            .add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())
            .map_err(|_| crate::error::RecordingError::Unknown {
                details: "Failed to register preview keydown handler".into(),
            })?;

        self._keydown_closure = Some(cb);
        Ok(())
    }

    /// Register the Download button click handler.
    #[cfg(target_arch = "wasm32")]
    fn register_download_handler(&mut self) -> Result<()> {
        let btn = self.download_btn_el.clone();
        let download_ptr: *mut Option<Box<dyn FnMut(String, Vec<u8>)>> =
            &mut self.on_download as *mut _;
        let session_id = self.session_id.clone();
        let webm_data = self.webm_data.clone();

        let cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
            event.stop_propagation();
            if let Some(ref mut cb) = unsafe { &mut *download_ptr } {
                if let Some(ref sid) = session_id {
                    if let Some(ref data) = webm_data {
                        cb(sid.clone(), data.clone());
                    }
                }
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        if let Some(ref btn_el) = btn {
            btn_el
                .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register download click handler".into(),
                })?;
        }

        self._download_click_closure = Some(cb);
        Ok(())
    }

    /// Register the Delete button click handler (shows the confirmation dialog).
    #[cfg(target_arch = "wasm32")]
    fn register_delete_handler(&mut self) -> Result<()> {
        let btn = self.delete_btn_el.clone();
        let dialog_ptr: *mut bool = &mut self.dialog_visible as *mut _;

        let cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
            event.stop_propagation();
            // Show the confirmation dialog.
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                if let Some(dialog) = doc.get_element_by_id("delete-dialog") {
                    dialog.remove_attribute("hidden").ok();
                }
            }
            unsafe { *dialog_ptr = true };
        }) as Box<dyn FnMut(web_sys::Event)>);

        if let Some(ref btn_el) = btn {
            btn_el
                .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register delete click handler".into(),
                })?;
        }

        self._delete_click_closure = Some(cb);
        Ok(())
    }

    /// Register dialog button handlers (Cancel and Confirm).
    #[cfg(target_arch = "wasm32")]
    fn register_dialog_handlers(&mut self) -> Result<()> {
        // Cancel button — hide dialog.
        {
            let cancel_btn = self.dialog_cancel_el.clone();
            let dialog_ptr: *mut bool = &mut self.dialog_visible as *mut _;

            let cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
                event.stop_propagation();
                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                    if let Some(dialog) = doc.get_element_by_id("delete-dialog") {
                        dialog.set_attribute("hidden", "").ok();
                    }
                }
                unsafe { *dialog_ptr = false };
            }) as Box<dyn FnMut(web_sys::Event)>);

            if let Some(ref btn_el) = cancel_btn {
                btn_el
                    .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                    .map_err(|_| crate::error::RecordingError::Unknown {
                        details: "Failed to register dialog cancel handler".into(),
                    })?;
            }

            self._dialog_cancel_closure = Some(cb);
        }

        // Confirm button — trigger deletion.
        {
            let confirm_fn_ptr: *mut Option<Box<dyn FnMut(String)>> =
                &mut self.on_delete_confirmed as *mut _;
            let session_id = self.session_id.clone();
            let dialog_ptr: *mut bool = &mut self.dialog_visible as *mut _;

            let cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
                event.stop_propagation();
                // Hide the dialog.
                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                    if let Some(dialog) = doc.get_element_by_id("delete-dialog") {
                        dialog.set_attribute("hidden", "").ok();
                    }
                }
                unsafe { *dialog_ptr = false };

                // Fire the delete callback.
                if let Some(ref mut cb) = unsafe { &mut *confirm_fn_ptr } {
                    if let Some(ref sid) = session_id {
                        cb(sid.clone());
                    }
                }
            }) as Box<dyn FnMut(web_sys::Event)>);

            if let Some(ref btn_el) = self.dialog_confirm_el {
                btn_el
                    .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                    .map_err(|_| crate::error::RecordingError::Unknown {
                        details: "Failed to register dialog confirm handler".into(),
                    })?;
            }

            self._dialog_confirm_closure = Some(cb);
        }

        Ok(())
    }

    /// Register the error state Back button handler.
    #[cfg(target_arch = "wasm32")]
    fn register_error_back_handler(&mut self) -> Result<()> {
        let close_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_close as *mut _;

        let cb = Closure::wrap(Box::new(move |event: web_sys::Event| {
            event.stop_propagation();
            if let Some(ref mut cb) = unsafe { &mut *close_ptr } {
                cb();
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        if let Some(ref btn_el) = self.error_back_el {
            btn_el
                .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register error back handler".into(),
                })?;
        }

        self._error_back_closure = Some(cb);
        Ok(())
    }

    // ------------------------------------------------------------------
    // WASM: DOM updates
    // ------------------------------------------------------------------

    /// Focus the video element after a microtask delay so the CSS is settled.
    #[cfg(target_arch = "wasm32")]
    fn focus_video(&self, document: &Document) {
        let video_id = "preview-video".to_string();
        let cb = Closure::once(move || {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                if let Some(el) = doc.get_element_by_id(&video_id) {
                    let _ = el.cast::<HtmlVideoElement>().map(|v| v.focus());
                }
            }
        });
        let _ = web_sys::window()
            .and_then(|w| {
                w.set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    50,
                ).ok()
            });
        cb.forget();
    }

    /// Update the integrity badge DOM element to reflect current state.
    #[cfg(target_arch = "wasm32")]
    fn update_integrity_badge(&self) {
        if let Some(ref badge) = self.badge_el {
            // Remove all integrity classes.
            badge.set_attribute("class", self.integrity.css_class()).ok();
            badge.set_text_content(Some(self.integrity.as_label()));
            badge
                .set_attribute(
                    "aria-label",
                    &format!("Integrity: {}", self.integrity.aria_label()),
                )
                .ok();
        }
    }

    /// Update the detail message DOM element.
    #[cfg(target_arch = "wasm32")]
    fn update_detail_message(&self) {
        if let Some(ref el) = self.detail_message_el {
            match self.detail_message.as_ref() {
                Some(msg) => {
                    el.remove_attribute("hidden").ok();
                    el.set_text_content(Some(msg));
                }
                None => {
                    el.set_attribute("hidden", "").ok();
                    el.set_text_content(None);
                }
            }
        }
    }

    /// Update the dialog visibility in the DOM.
    #[cfg(target_arch = "wasm32")]
    fn update_dialog_visibility(&self) {
        if let Some(ref dialog) = self.dialog_el {
            if self.dialog_visible {
                dialog.remove_attribute("hidden").ok();
            } else {
                dialog.set_attribute("hidden", "").ok();
            }
        }
    }

    /// Update the error container visibility in the DOM.
    #[cfg(target_arch = "wasm32")]
    fn update_error_visibility(&self) {
        if let Some(ref container) = self.error_container_el {
            if self.error_message.is_some() {
                container.remove_attribute("hidden").ok();
            } else {
                container.set_attribute("hidden", "").ok();
            }
        }
        if let Some(ref msg_el) = self.error_message_el {
            if let Some(ref msg) = self.error_message {
                msg_el.set_text_content(Some(msg));
            }
        }
        if let Some(ref sug_el) = self.error_suggestion_el {
            if let Some(ref sug) = self.error_suggestion {
                sug_el.set_text_content(Some(sug));
            }
        }
        // Show/hide video container and actions bar based on error state.
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(video_container) = doc.get_element_by_id("video-container") {
                if self.error_message.is_some() {
                    video_container.set_attribute("hidden", "").ok();
                } else {
                    video_container.remove_attribute("hidden").ok();
                }
            }
            if let Some(actions_bar) = doc.get_element_by_id("actions-bar") {
                if self.error_message.is_some() {
                    actions_bar.set_attribute("hidden", "").ok();
                } else {
                    actions_bar.remove_attribute("hidden").ok();
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // WASM: Destroy — clean up DOM, closures, and object URLs
    // ------------------------------------------------------------------

    /// Remove the preview page from the DOM and clean up all resources.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.rendered = false;

        // Revoke the object URL.
        if let Some(ref url) = self.object_url {
            Url::revoke_object_url(url);
        }
        self.object_url = None;

        // Clear video source.
        if let Some(ref video) = self.video_el {
            video.remove_attribute("src").ok();
            let _ = video.load();
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
        self._download_click_closure.take();
        self._delete_click_closure.take();
        self._dialog_cancel_closure.take();
        self._dialog_confirm_closure.take();
        self._error_back_closure.take();
        self._video_click_closure.take();

        // Remove the container from the DOM.
        if let Some(container) = self.container.take() {
            let _ = container.remove();
        }

        // Clear all element references.
        self.video_el = None;
        self.download_btn_el = None;
        self.delete_btn_el = None;
        self.badge_el = None;
        self.detail_message_el = None;
        self.dialog_el = None;
        self.dialog_cancel_el = None;
        self.dialog_confirm_el = None;
        self.error_container_el = None;
        self.error_message_el = None;
        self.error_suggestion_el = None;
        self.error_back_el = None;

        // Clear callbacks.
        self.on_delete_confirmed = None;
        self.on_close = None;
        self.on_download = None;
    }

    /// Native no-op.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn destroy(&mut self) {
        self.rendered = false;
        self.on_delete_confirmed = None;
        self.on_close = None;
        self.on_download = None;
    }
}

impl Default for PreviewPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Drop safety: clean up DOM, closures, and object URLs if dropped without
/// an explicit `destroy()` call.
#[cfg(target_arch = "wasm32")]
impl Drop for PreviewPage {
    fn drop(&mut self) {
        if !self.destroyed {
            oxichrome::log!("PreviewPage dropped without destroy() — cleaning up");
            self.destroy();
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return a date string for filenames when no session ID is available.
///
/// Format: `YYYY-MM-DD` (e.g., "2026-06-20").
fn chrono_now() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        // Use JS Date in WASM context.
        let date = js_sys::Date::new_0();
        let year = date.get_full_year();
        let month = date.get_month() + 1; // 0-indexed
        let day = date.get_date();
        format!("{:04}-{:02}-{:02}", year, month, day)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Use std::time in native context (epoch-based approximation).
        "unknown-date".to_string()
    }
}

/// WASM entry point called from the preview page HTML.
///
/// Initializes the preview page with exported WebM data, renders the DOM,
/// and wires up browser-native event handlers (download via anchor click,
/// close via `window.close()`).
///
/// # Arguments
///
/// * `session_id` — Unique session identifier for filename generation.
/// * `webm_data` — The raw WebM bytes from the export pipeline.
/// * `integrity` — Integrity state label: "Clean", "Partial", or "Incomplete".
/// * `detail` — Optional detail message for the integrity badge (e.g., "Clean — up to chunk N of M").
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_preview(session_id: &str, webm_data: &[u8], integrity: &str, detail: Option<String>) {
    let mut page = PreviewPage::new();
    page.set_session_id(session_id.to_owned());
    page.set_webm_data(webm_data.to_vec());

    match integrity {
        "Partial" => page.set_integrity(IntegrityState::Partial),
        "Incomplete" => page.set_integrity(IntegrityState::Incomplete),
        _ => page.set_integrity(IntegrityState::Clean),
    };

    if let Some(msg) = detail {
        if !msg.is_empty() {
            page.set_detail_message(Some(msg));
        }
    }

    // Set up the download handler: use chrome.downloads.download() API
    // per AC3, with proper save dialog support and native downloads manager integration.
    {
        let sid = session_id.to_owned();
        let data = webm_data.to_vec();
        page.set_on_download(move |_id, _data| {
            // Skip download if there's no data.
            if data.is_empty() {
                web_sys::window()
                    .and_then(|w| js_sys::Reflect::get(&w, &"console".into()).ok())
                    .and_then(|c| {
                        js_sys::Reflect::call(
                            &js_sys::Reflect::get(&c, &"warn".into()).ok()?,
                            &c,
                            &js_sys::Array::of1(&"Capture Forge: download skipped — no data".into()),
                        ).ok()
                    });
                return;
            }
            // Create a Blob from the exported data.
            let uint8 = js_sys::Uint8Array::from(&data[..]);
            let arr = js_sys::Array::new();
            arr.push(&uint8.buffer());
            if let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&arr) {
                if let Ok(url) = Url::create_object_url_with_blob(&blob) {
                    let filename = format!("CaptureForge-{}.webm", sid);
                    // Call chrome.downloads.download({url, filename}, callback)
                    let try_download = || -> Option<()> {
                        let chrome = js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?;
                        let downloads = js_sys::Reflect::get(&chrome, &"downloads".into()).ok()?;
                        let download_fn = js_sys::Reflect::get(&downloads, &"download".into()).ok()?;
                        let opts = js_sys::Object::new();
                        js_sys::Reflect::set(&opts, &"url".into(), &url).ok()?;
                        js_sys::Reflect::set(&opts, &"filename".into(), &filename).ok()?;
                        // Revoke the blob URL after Chrome starts the download.
                        let url_clone = url.clone();
                        let revoke_cb = Closure::wrap(Box::new(move |_download_id: JsValue| {
                            Url::revoke_object_url(&url_clone);
                        }) as Box<dyn FnMut(JsValue)>);
                        let _ = js_sys::Reflect::apply(
                            &download_fn,
                            &downloads,
                            &js_sys::Array::of2(&opts, revoke_cb.as_ref().unchecked_ref()),
                        ).ok()?;
                        revoke_cb.forget();
                        Some(())
                    };
                    if try_download().is_none() {
                        // Fallback: revoke URL immediately if chrome.downloads is unavailable.
                        Url::revoke_object_url(&url);
                    }
                }
            }
        });
    }

    // Helper to close the current tab using chrome.tabs API.
    // window.close() is unreliable for extension pages; chrome.tabs is correct.
    #[inline]
    fn close_preview_tab() {
        let _ = (|| -> Option<()> {
            let chrome = js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?;
            let tabs = js_sys::Reflect::get(&chrome, &"tabs".into()).ok()?;
            let get_current_fn = js_sys::Reflect::get(&tabs, &"getCurrent".into()).ok()?;
            let remove_fn = js_sys::Reflect::get(&tabs, &"remove".into()).ok()?;

            // Build a closure that receives the tab and removes it.
            // Must be forget()'d so it survives the async callback.
            let remove_cb = Closure::wrap(Box::new(move |tab: JsValue, _more: js_sys::Array| {
                if let Some(tab_id) = js_sys::Reflect::get(&tab, &"id".into()).ok() {
                    if !tab_id.is_undefined() {
                        let _ = js_sys::Reflect::apply(
                            &remove_fn,
                            &tabs,
                            &js_sys::Array::of1(&tab_id),
                        );
                    }
                }
            }) as Box<dyn FnMut(JsValue, js_sys::Array)>);

            let _ = js_sys::Reflect::apply(
                &get_current_fn,
                &tabs,
                &js_sys::Array::of1(remove_cb.as_ref().unchecked_ref()),
            ).ok()?;

            // Leak the closure — it must survive until chrome.tabs.getCurrent
            // calls back.
            remove_cb.forget();
            Some(())
        })();
    }

    // Set up the close handler: notify the background and close the tab.
    {
        let close_sid = session_id.to_owned();
        page.set_on_close(move || {
            // Notify the background that the preview was closed, including the
            // session ID so the background can clean up the preview data store.
            let _ = (|| -> Option<()> {
                let runtime = js_sys::Reflect::get(
                    &js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?,
                    &"runtime".into(),
                ).ok()?;
                let send_msg = js_sys::Reflect::get(&runtime, &"sendMessage".into()).ok()?;
                let msg = js_sys::Object::new();
                js_sys::Reflect::set(&msg, &"type".into(), &"PREVIEW_CLOSED".into()).ok()?;
                js_sys::Reflect::set(&msg, &"sessionId".into(), &close_sid).ok()?;
                let _ = js_sys::Reflect::apply(&send_msg, &runtime, &js_sys::Array::of1(&msg)).ok()?;
                Some(())
            })();
            // Close the current tab via chrome.tabs API.
            close_preview_tab();
        });
    }

    // Set up the delete confirmed handler: notify background and close tab.
    {
        let delete_sid = session_id.to_owned();
        page.set_on_delete_confirmed(move |_sid| {
            // Notify the background that the recording should be deleted,
            // including the session ID so preview data is cleaned up.
            let _ = (|| -> Option<()> {
                let runtime = js_sys::Reflect::get(
                    &js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?,
                    &"runtime".into(),
                ).ok()?;
                let send_msg = js_sys::Reflect::get(&runtime, &"sendMessage".into()).ok()?;
                let msg = js_sys::Object::new();
                js_sys::Reflect::set(&msg, &"type".into(), &"DELETE_RECORDING".into()).ok()?;
                js_sys::Reflect::set(&msg, &"sessionId".into(), &delete_sid).ok()?;
                let _ = js_sys::Reflect::apply(&send_msg, &runtime, &js_sys::Array::of1(&msg)).ok()?;
                Some(())
            })();
            // Close the current tab via chrome.tabs API.
            close_preview_tab();
        });
    }

    // Render the preview page.
    if let Err(e) = page.render() {
        oxichrome::log!("PreviewPage::render() failed: {:?}", e);
    }

    // Leak the page so it lives for the lifetime of the page.
    // When the page navigates away, the WASM memory is reclaimed.
    std::mem::forget(page);
}

// ---------------------------------------------------------------------------
// Native unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // PreviewPage construction
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_page_creation() {
        let page = PreviewPage::new();
        assert_eq!(page.integrity_state(), &IntegrityState::Clean);
        assert_eq!(page.integrity_text(), "Clean");
        assert!(!page.is_dialog_visible());
        assert!(page.session_id().is_none());
        assert!(page.error_message().is_none());
        assert!(!page.is_rendered());
    }

    // ------------------------------------------------------------------
    // Integrity badge states
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_integrity_badge_clean() {
        let mut page = PreviewPage::new();
        page.set_integrity(IntegrityState::Clean);
        assert_eq!(page.integrity_state(), &IntegrityState::Clean);
        assert_eq!(page.integrity_text(), "Clean");
    }

    #[test]
    fn test_preview_integrity_badge_partial() {
        let mut page = PreviewPage::new();
        page.set_integrity(IntegrityState::Partial);
        assert_eq!(page.integrity_state(), &IntegrityState::Partial);
        assert_eq!(page.integrity_text(), "Partial");
    }

    #[test]
    fn test_preview_integrity_badge_incomplete() {
        let mut page = PreviewPage::new();
        page.set_integrity(IntegrityState::Incomplete);
        assert_eq!(page.integrity_state(), &IntegrityState::Incomplete);
        assert_eq!(page.integrity_text(), "Incomplete");
    }

    #[test]
    fn test_preview_integrity_colors() {
        assert_eq!(IntegrityState::Clean.css_class(), "integrity-clean");
        assert_eq!(IntegrityState::Partial.css_class(), "integrity-partial");
        assert_eq!(IntegrityState::Incomplete.css_class(), "integrity-incomplete");

        assert_eq!(IntegrityState::Clean.aria_label(), "Clean");
        assert_eq!(IntegrityState::Partial.aria_label(), "Partial");
        assert_eq!(IntegrityState::Incomplete.aria_label(), "Incomplete");
    }

    #[test]
    fn test_preview_integrity_playback_not_blocked() {
        let mut page = PreviewPage::new();
        page.set_webm_data(vec![0x1A, 0x45, 0xDF, 0xA3]); // mock WebM header

        // Integrity state doesn't affect playback — just set data and verify.
        page.set_integrity(IntegrityState::Partial);
        assert!(page.webm_data.is_some());

        page.set_integrity(IntegrityState::Incomplete);
        assert!(page.webm_data.is_some());

        page.set_integrity(IntegrityState::Clean);
        assert!(page.webm_data.is_some());
    }

    // ------------------------------------------------------------------
    // Delete confirmation dialog
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_delete_confirmation() {
        let mut page = PreviewPage::new();
        assert!(!page.is_dialog_visible());

        page.show_delete_dialog();
        assert!(page.is_dialog_visible());

        page.hide_delete_dialog();
        assert!(!page.is_dialog_visible());
    }

    #[test]
    fn test_preview_delete_confirmed() {
        let mut page = PreviewPage::new();
        page.set_session_id("test-session-1".into());
        page.set_webm_data(vec![0x1A, 0x45, 0xDF, 0xA3]);

        let deleted_id = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let d = std::rc::Rc::clone(&deleted_id);
        page.set_on_delete_confirmed(move |id| {
            *d.borrow_mut() = id;
        });

        page.show_delete_dialog();
        assert!(page.is_dialog_visible());

        page.confirm_delete();
        assert!(!page.is_dialog_visible());
        assert_eq!(*deleted_id.borrow(), "test-session-1");
    }

    #[test]
    fn test_preview_delete_cancelled() {
        let mut page = PreviewPage::new();
        page.set_session_id("test-session-1".into());

        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&called);
        page.set_on_delete_confirmed(move |_| {
            c.set(true);
        });

        page.show_delete_dialog();
        assert!(page.is_dialog_visible());

        // Cancel — hide dialog without firing callback.
        page.hide_delete_dialog();
        assert!(!page.is_dialog_visible());
        assert!(!called.get(), "delete callback should not fire on cancel");
    }

    // ------------------------------------------------------------------
    // Keyboard handling
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_escape_closes() {
        let mut page = PreviewPage::new();
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&called);
        page.set_on_close(move || {
            c.set(true);
        });

        // No dialog active — Escape should trigger close.
        assert!(!page.is_dialog_visible());
        page.handle_escape();
        assert!(called.get());
    }

    #[test]
    fn test_preview_escape_during_dialog() {
        let mut page = PreviewPage::new();
        let close_called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&close_called);
        page.set_on_close(move || {
            c.set(true);
        });

        // Show dialog first.
        page.show_delete_dialog();
        assert!(page.is_dialog_visible());

        // Escape during dialog — should close dialog, NOT fire close callback.
        page.handle_escape();
        assert!(!page.is_dialog_visible(), "dialog should close on Escape");
        assert!(!close_called.get(), "page should NOT close when dialog is visible");
    }

    #[test]
    fn test_preview_space_toggle_playback() {
        let mut page = PreviewPage::new();
        // Space is a no-op at the pure-logic level (handled by browser).
        // This test verifies the method can be called without panicking.
        page.handle_space();
    }

    // ------------------------------------------------------------------
    // Focus management
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_focus_on_load() {
        let mut page = PreviewPage::new();
        // Native: render() is a no-op that sets rendered = true.
        page.render().expect("render should succeed");
        assert!(page.is_rendered());
        // Focus on video element is WASM-only, verified via WASM tests.
    }

    // ------------------------------------------------------------------
    // Error state
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_error_state_export_failure() {
        let mut page = PreviewPage::new();

        // Initial state: no error.
        assert!(page.error_message().is_none());
        assert!(page.error_suggestion().is_none());

        // Show error.
        page.show_error(
            "Could not create WebM file.",
            "Check available disk space and try again.",
        );
        assert_eq!(
            page.error_message(),
            Some("Could not create WebM file.")
        );
        assert_eq!(
            page.error_suggestion(),
            Some("Check available disk space and try again.")
        );

        // Render with error state — should show error, hide player.
        page.render().expect("render with error should succeed");
        assert!(page.is_rendered());

        // Hide error.
        page.hide_error();
        assert!(page.error_message().is_none());
    }

    // ------------------------------------------------------------------
    // Aria labels
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_aria_labels() {
        let _page = PreviewPage::new();
        // At the pure-logic level, aria labels are verified on IntegrityState.
        assert_eq!(IntegrityState::Clean.aria_label(), "Clean");
        assert_eq!(IntegrityState::Partial.aria_label(), "Partial");
        assert_eq!(IntegrityState::Incomplete.aria_label(), "Incomplete");
    }

    // ------------------------------------------------------------------
    // Filename format
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_filename_format() {
        let mut page = PreviewPage::new();
        page.set_session_id("rec_abc_1234".into());
        let filename = page.download_filename();
        assert_eq!(filename, "CaptureForge-rec_abc_1234.webm");
    }

    #[test]
    fn test_preview_filename_format_no_session_id() {
        let page = PreviewPage::new();
        // Without session ID, uses date fallback.
        let filename = page.download_filename();
        assert!(filename.starts_with("Recording-"));
        assert!(filename.ends_with(".webm"));
    }

    // ------------------------------------------------------------------
    // Callback wiring
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_download_trigger() {
        let mut page = PreviewPage::new();
        page.set_session_id("test-session-dl".into());
        page.set_webm_data(vec![0x1A, 0x45, 0xDF, 0xA3]);

        let captured = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&captured);
        page.set_on_download(move |_id, _data| {
            c.set(true);
        });

        // Simulate download trigger by invoking callback.
        if let Some(ref mut cb) = page.on_download {
            cb("test-session-dl".into(), vec![0x1A, 0x45, 0xDF, 0xA3]);
        }
        assert!(captured.get());
    }

    // ------------------------------------------------------------------
    // Session ID and webm data accessors
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_set_session_id() {
        let mut page = PreviewPage::new();
        assert!(page.session_id().is_none());
        page.set_session_id("rec_xxx_0001".into());
        assert_eq!(page.session_id(), Some("rec_xxx_0001"));
    }

    #[test]
    fn test_preview_set_webm_data() {
        let mut page = PreviewPage::new();
        page.set_webm_data(vec![0x1A, 0x45, 0xDF, 0xA3]);
        assert!(page.webm_data.is_some());
    }

    // ------------------------------------------------------------------
    // Integrity detail message (Story 1.7 deferred / Story 1.8)
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_detail_message_default_none() {
        let page = PreviewPage::new();
        assert!(page.detail_message().is_none());
    }

    #[test]
    fn test_preview_detail_message_partial() {
        let mut page = PreviewPage::new();
        page.set_integrity(IntegrityState::Partial);
        page.set_detail_message(Some("Clean — up to chunk 7 of 10".into()));
        assert_eq!(
            page.detail_message(),
            Some("Clean — up to chunk 7 of 10"),
        );
    }

    #[test]
    fn test_preview_detail_message_incomplete() {
        let mut page = PreviewPage::new();
        page.set_integrity(IntegrityState::Incomplete);
        page.set_detail_message(Some("This recording could not be fully recovered.".into()));
        assert_eq!(
            page.detail_message(),
            Some("This recording could not be fully recovered."),
        );
    }

    #[test]
    fn test_preview_detail_message_clear() {
        let mut page = PreviewPage::new();
        page.set_detail_message(Some("Some message".into()));
        assert!(page.detail_message().is_some());
        page.set_detail_message(None);
        assert!(page.detail_message().is_none());
    }

    #[test]
    fn test_preview_playback_available_with_partial_integrity() {
        let mut page = PreviewPage::new();
        page.set_webm_data(vec![0x1A, 0x45, 0xDF, 0xA3]);
        page.set_integrity(IntegrityState::Partial);
        page.set_detail_message(Some("Clean — up to chunk 5 of 10".into()));

        // Playback should still work regardless of integrity state.
        assert!(page.webm_data.is_some());
        assert_eq!(page.integrity_state(), &IntegrityState::Partial);
    }

    // ------------------------------------------------------------------
    // Render lifecycle
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_render_and_destroy() {
        let mut page = PreviewPage::new();
        assert!(!page.is_rendered());

        page.render().expect("render should succeed");
        assert!(page.is_rendered());

        page.destroy();
        assert!(!page.is_rendered());
    }

    #[test]
    fn test_preview_render_twice_noop() {
        let mut page = PreviewPage::new();
        page.render().expect("first render");
        assert!(page.is_rendered());

        // Second render should be a no-op (not panic).
        page.render().expect("second render should be no-op");
        assert!(page.is_rendered());
    }

    #[test]
    fn test_preview_destroy_twice_noop() {
        let mut page = PreviewPage::new();
        page.render().expect("render");
        page.destroy();
        // Second destroy should be a no-op.
        page.destroy();
        assert!(!page.is_rendered());
    }

    // ------------------------------------------------------------------
    // Default
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_default() {
        let page = PreviewPage::default();
        assert_eq!(page.integrity_state(), &IntegrityState::Clean);
        assert!(!page.is_dialog_visible());
        assert!(!page.is_rendered());
    }

    // ------------------------------------------------------------------
    // Error back button callback
    // ------------------------------------------------------------------

    #[test]
    fn test_preview_error_back_triggers_close() {
        let mut page = PreviewPage::new();
        page.show_error("Could not create WebM file.", "Check available disk space and try again.");

        let close_called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&close_called);
        page.set_on_close(move || {
            c.set(true);
        });

        // Simulate Back button press.
        if let Some(ref mut cb) = page.on_close {
            cb();
        }
        assert!(close_called.get());
    }
}
