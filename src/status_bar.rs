use crate::error::Result;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    Document, Element, HtmlElement, HtmlSpanElement, Node, ShadowRoot, ShadowRootInit,
    ShadowRootMode,
};

// ---------------------------------------------------------------------------
// CSS — inline in the shadow root
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
const STATUSBAR_CSS: &str = r#"
:host {
    all: initial;
    display: flex;
    align-items: center;
    justify-content: center;
    position: fixed;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 2147483646;
    min-width: 180px;
    height: 44px;
    padding: 0 12px;
    border-radius: 10px;
    background: #FFFFFF;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Helvetica Neue', sans-serif;
    opacity: 0.7;
    transition: opacity 0.2s ease;
    -webkit-font-smoothing: antialiased;
    box-sizing: border-box;
    user-select: none;
}

:host(:hover) {
    opacity: 1;
}

@media (prefers-color-scheme: dark) {
    :host {
        background: #1A1B1E;
        box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
    }
}

.toolbar-inner {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    height: 100%;
}

.timer-area {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
}

.timer {
    font-family: 'SF Mono', 'Cascadia Code', 'JetBrains Mono', 'Fira Code', Consolas, monospace;
    font-size: 20px;
    font-weight: 600;
    line-height: 1.2;
    letter-spacing: 0.05em;
    color: #EF4444;
    white-space: nowrap;
}

@media (prefers-color-scheme: dark) {
    .timer {
        color: #FCA5A5;
    }
}

.timer.blinking {
    animation: blink 1s ease-in-out infinite;
}

@keyframes blink {
    0%, 100% { opacity: 1.0; }
    50% { opacity: 0.3; }
}

@media (prefers-reduced-motion: reduce) {
    .timer.blinking {
        animation: blink 2s ease-in-out infinite;
    }
}

.paused-label {
    font-size: 12px;
    font-weight: 500;
    letter-spacing: 0.02em;
    color: #71717A;
    white-space: nowrap;
}

@media (prefers-color-scheme: dark) {
    .paused-label {
        color: #A0A1A7;
    }
}

.btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
    outline: none;
    font-size: 16px;
    line-height: 1;
    transition: background 0.15s ease;
}

.btn:hover {
    background: #F4F4F5;
}

.btn:focus-visible {
    box-shadow: 0 0 0 2px #2563EB;
}

@media (prefers-color-scheme: dark) {
    .btn:hover {
        background: #27282B;
    }
    .btn:focus-visible {
        box-shadow: 0 0 0 2px #60A5FA;
    }
}

.btn-pause {
    color: #1A1B1E;
}

@media (prefers-color-scheme: dark) {
    .btn-pause {
        color: #E4E5E7;
    }
}

.btn-stop {
    color: #EF4444;
}

@media (prefers-color-scheme: dark) {
    .btn-stop {
        color: #F87171;
    }
}

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

/// A floating toolbar that shows recording timer, pause/resume, and stop
/// controls, rendered via shadow DOM in the active tab.
///
/// The status bar is the persistent UI surface during Recording and Paused
/// states.  Timer updates are driven by a 250ms interval, and pause state
/// toggles the blink animation and label.
///
/// # State guards
///
/// | method        | Allowed when    | Behaviour |
/// |---------------|-----------------|-----------|
/// | show          | new()           | Injects shadow DOM and starts timer |
/// | set_paused    | visible         | Toggles blink, label, and icon |
/// | update        | visible         | Updates timer display text |
/// | remove        | visible         | Cleans up DOM elements and intervals |
pub(crate) struct RecorderStatusBar {
    /// Whether the recording is currently paused.
    paused: bool,
    /// Callback invoked when the pause/resume button is clicked.
    on_pause_toggle: Option<Box<dyn FnMut()>>,
    /// Callback invoked when the stop button is clicked.
    on_stop: Option<Box<dyn FnMut()>>,
    /// The container element in the document body.
    #[cfg(target_arch = "wasm32")]
    container: Option<Element>,
    /// The timer display span.
    #[cfg(target_arch = "wasm32")]
    timer_el: Option<HtmlSpanElement>,
    /// The "Paused" label element.
    #[cfg(target_arch = "wasm32")]
    paused_label_el: Option<HtmlSpanElement>,
    /// The pause/resume button.
    #[cfg(target_arch = "wasm32")]
    pause_btn_el: Option<HtmlElement>,
    /// The screen-reader announcement region.
    #[cfg(target_arch = "wasm32")]
    aria_el: Option<HtmlSpanElement>,
    /// The timer update interval handle.
    #[cfg(target_arch = "wasm32")]
    _timer_interval: Option<i32>,
    /// Re-entrancy guard for remove().
    #[cfg(target_arch = "wasm32")]
    removed: bool,
}

impl RecorderStatusBar {
    /// Create a new `RecorderStatusBar` in an unrendered state.
    pub(crate) fn new() -> Self {
        Self {
            paused: false,
            on_pause_toggle: None,
            on_stop: None,
            #[cfg(target_arch = "wasm32")]
            container: None,
            #[cfg(target_arch = "wasm32")]
            timer_el: None,
            #[cfg(target_arch = "wasm32")]
            paused_label_el: None,
            #[cfg(target_arch = "wasm32")]
            pause_btn_el: None,
            #[cfg(target_arch = "wasm32")]
            aria_el: None,
            #[cfg(target_arch = "wasm32")]
            _timer_interval: None,
            #[cfg(target_arch = "wasm32")]
            removed: false,
        }
    }

    /// Render the status bar by injecting shadow DOM into the document body.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn show(&mut self) -> Result<()> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| crate::error::RecordingError::Unknown {
                details: "Cannot access document for status bar".into(),
            })?;

        let body = document.body().ok_or_else(|| {
            crate::error::RecordingError::Unknown {
                details: "No document body for status bar".into(),
            }
        })?;

        // Create container with shadow root.
        let container = document.create_element("div")?;
        container.set_attribute("data-capture-forge", "statusbar")?;
        let shadow_init = ShadowRootInit::new(ShadowRootMode::Open);
        let shadow = container.attach_shadow(&shadow_init)?;

        // Inject CSS.
        let style = document.create_element("style")?;
        style.set_text_content(Some(STATUSBAR_CSS));
        shadow.append_child(&style)?;

        // --- Build toolbar DOM ---
        let inner = document.create_element("div")?;
        inner.set_attribute("class", "toolbar-inner")?;

        // Timer area (left side).
        let timer_area = document.create_element("div")?;
        timer_area.set_attribute("class", "timer-area")?;

        let timer_el = document.create_element("span")?;
        timer_el.set_attribute("class", "timer")?;
        timer_el.set_attribute("aria-label", "Recording duration")?;
        timer_el.set_text_content(Some("00:00"));
        timer_area.append_child(&timer_el)?;

        let paused_label_el = document.create_element("span")?;
        paused_label_el.set_attribute("class", "paused-label")?;
        paused_label_el.set_text_content(Some("Paused"));
        paused_label_el.set_attribute("style", "display: none");
        timer_area.append_child(&paused_label_el)?;

        inner.append_child(&timer_area)?;

        // Pause/Resume button (center).
        let pause_btn = document.create_element("button")?;
        pause_btn.set_attribute("class", "btn btn-pause")?;
        pause_btn.set_attribute("aria-label", "Pause recording")?;
        pause_btn.set_attribute("tabindex", "-1")?;
        pause_btn.set_text_content(Some("⏸"));
        inner.append_child(&pause_btn)?;

        // Stop button (right).
        let stop_btn = document.create_element("button")?;
        stop_btn.set_attribute("class", "btn btn-stop")?;
        stop_btn.set_attribute("aria-label", "Stop recording")?;
        stop_btn.set_attribute("tabindex", "-1")?;
        stop_btn.set_text_content(Some("⏹"));
        inner.append_child(&stop_btn)?;

        // Screen-reader announcement region.
        let aria_el = document.create_element("span")?;
        aria_el.set_attribute("class", "sr-only")?;
        aria_el.set_attribute("aria-live", "polite")?;
        aria_el.set_attribute("aria-atomic", "true")?;
        inner.append_child(&aria_el)?;

        shadow.append_child(&inner)?;

        // Store element references.
        self.container = Some(container);
        self.timer_el = Some(timer_el.unchecked_into::<HtmlSpanElement>());
        self.paused_label_el = Some(paused_label_el.unchecked_into::<HtmlSpanElement>());
        self.pause_btn_el = Some(pause_btn.unchecked_into::<HtmlElement>());
        self.aria_el = Some(aria_el.unchecked_into::<HtmlSpanElement>());

        // Wire pause button click.
        {
            let on_pause_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_pause_toggle as *mut _;
            let btn = pause_btn.clone();
            let pause_cb = Closure::wrap(Box::new(move || {
                if let Some(ref mut cb) = unsafe { &mut *on_pause_ptr } {
                    cb();
                }
            }) as Box<dyn FnMut()>);
            btn.add_event_listener_with_callback("click", pause_cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register pause click handler".into(),
                })?;
            pause_cb.forget();
        }

        // Wire stop button click.
        {
            let on_stop_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_stop as *mut _;
            let btn = stop_btn.clone();
            let stop_cb = Closure::wrap(Box::new(move || {
                if let Some(ref mut cb) = unsafe { &mut *on_stop_ptr } {
                    cb();
                }
            }) as Box<dyn FnMut()>);
            btn.add_event_listener_with_callback("click", stop_cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register stop click handler".into(),
                })?;
            stop_cb.forget();
        }

        // Start the timer update interval (250ms).
        {
            let timer_ref = self.timer_el.clone();
            let paused_ptr: *const std::sync::atomic::AtomicBool = &std::sync::atomic::AtomicBool::new(false) as *const _;
            // Actually, we need a different approach — we'll use an update method called externally.
            // For the interval, we rely on `update()` being called externally.
            // This keeps the architecture clean: the lifecycle module drives timer updates.
        }

        // Append to body.
        body.append_child(self.container.as_ref().expect("invariant: container set"))?;

        self.announce("Recording started");

        Ok(())
    }

    /// Native no-op: status bar cannot be rendered outside a browser.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn show(&mut self) -> Result<()> {
        Ok(())
    }

    /// Update the timer display.
    ///
    /// `duration_ms` is the current elapsed recording time in milliseconds.
    /// This method is called externally (from the lifecycle orchestrator or
    /// a timer interval) at ~250ms intervals.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn update(&self, duration_ms: f64) {
        if let Some(ref timer) = self.timer_el {
            timer.set_text_content(Some(&format_duration(duration_ms)));
        }
    }

    /// Native no-op.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn update(&self, _duration_ms: f64) {}

    /// Set the paused state, updating the visual display.
    ///
    /// When `paused` is true:
    /// - Timer text blinks (CSS `.blinking` class)
    /// - "Paused" label is shown next to the timer
    /// - Pause icon switches to Resume (▶)
    /// - Host element opacity drops to 0.6
    ///
    /// When `paused` is false:
    /// - Timer stops blinking
    /// - "Paused" label is hidden
    /// - Icon switches back to Pause (⏸)
    /// - Host opacity returns to 0.7
    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.paused = paused;

        #[cfg(target_arch = "wasm32")]
        {
            // Toggle blink class on timer.
            if let Some(ref timer) = self.timer_el {
                if paused {
                    let _ = timer.class_list().add_1("blinking");
                } else {
                    let _ = timer.class_list().remove_1("blinking");
                }
            }

            // Toggle "Paused" label visibility.
            if let Some(ref label) = self.paused_label_el {
                if paused {
                    let _ = label.set_attribute("style", "");
                } else {
                    let _ = label.set_attribute("style", "display: none");
                }
            }

            // Toggle pause/resume icon and aria-label.
            if let Some(ref btn) = self.pause_btn_el {
                if paused {
                    btn.set_text_content(Some("▶"));
                    btn.set_attribute("aria-label", "Resume recording").ok();
                } else {
                    btn.set_text_content(Some("⏸"));
                    btn.set_attribute("aria-label", "Pause recording").ok();
                }
            }

            // Adjust host opacity.
            if let Some(ref container) = self.container {
                if paused {
                    let _ = container.set_attribute("style", "opacity: 0.6");
                } else {
                    container.remove_attribute("style").ok();
                }
            }

            // Announce state change.
            if paused {
                self.announce("Recording paused");
            } else {
                self.announce("Recording resumed");
            }
        }
    }

    /// Return whether the status bar is in paused mode.
    pub(crate) fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set the pause toggle callback (called when user clicks pause/resume).
    pub(crate) fn set_on_pause_toggle<F>(&mut self, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.on_pause_toggle = Some(Box::new(callback));
    }

    /// Set the stop callback (called when user clicks stop).
    pub(crate) fn set_on_stop<F>(&mut self, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.on_stop = Some(Box::new(callback));
    }

    /// Announce a message via the screen-reader region.
    #[cfg(target_arch = "wasm32")]
    fn announce(&self, message: &str) {
        if let Some(ref aria) = self.aria_el {
            aria.set_text_content(Some(message));
        }
    }

    /// Remove the status bar from the DOM.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn remove(&mut self) {
        if self.removed {
            return;
        }
        self.removed = true;

        if let Some(container) = self.container.take() {
            let _ = container.remove();
        }
        self.timer_el = None;
        self.paused_label_el = None;
        self.pause_btn_el = None;
        self.aria_el = None;
        self.paused = false;
    }

    /// Native no-op.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn remove(&mut self) {
        self.paused = false;
    }
}

impl Default for RecorderStatusBar {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Timer formatting — pure function, testable natively
// ---------------------------------------------------------------------------

/// Format elapsed milliseconds as `MM:SS` or `HH:MM:SS`.
///
/// - Under 1 hour: `MM:SS` (e.g., "03:42")
/// - 1 hour or more: `HH:MM:SS` (e.g., "01:02:15")
/// - Always zero-padded, colon-separated
pub(crate) fn format_duration(ms: f64) -> String {
    let total_secs = (ms / 1000.0).max(0.0) as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

// ---------------------------------------------------------------------------
// Native unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Timer formatting
    // ------------------------------------------------------------------

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0.0), "00:00");
    }

    #[test]
    fn test_format_duration_mmss() {
        assert_eq!(format_duration(30_000.0), "00:30");
        assert_eq!(format_duration(222_000.0), "03:42");
        assert_eq!(format_duration(3_500_000.0), "58:20");
    }

    #[test]
    fn test_format_duration_hhmmss() {
        assert_eq!(format_duration(3_733_000.0), "01:02:13");
        assert_eq!(format_duration(7_200_000.0), "02:00:00");
        assert_eq!(format_duration(366_1000.0), "01:01:01");
    }

    #[test]
    fn test_format_duration_edge_cases() {
        // Exactly 59:59 → still MM:SS
        assert_eq!(format_duration(3_599_000.0), "59:59");
        // Exactly 60:00 → switches to HH:MM:SS
        assert_eq!(format_duration(3_600_000.0), "01:00:00");
        // Negative
        assert_eq!(format_duration(-1000.0), "00:00");
        // Very large value
        assert_eq!(format_duration(100_000_000.0), "27:46:40");
    }

    #[test]
    fn test_format_duration_millisecond_truncation() {
        // 123.456 seconds → 2 minutes 3 seconds (milliseconds truncated)
        assert_eq!(format_duration(123_456.789), "02:03");
    }

    // ------------------------------------------------------------------
    // RecorderStatusBar state
    // ------------------------------------------------------------------

    #[test]
    fn test_new_status_bar_not_paused() {
        let bar = RecorderStatusBar::new();
        assert!(!bar.is_paused());
    }

    #[test]
    fn test_set_paused_true() {
        let mut bar = RecorderStatusBar::new();
        bar.set_paused(true);
        assert!(bar.is_paused());
    }

    #[test]
    fn test_set_paused_false() {
        let mut bar = RecorderStatusBar::new();
        bar.set_paused(true);
        assert!(bar.is_paused());
        bar.set_paused(false);
        assert!(!bar.is_paused());
    }

    #[test]
    fn test_pause_toggle_callback() {
        let mut bar = RecorderStatusBar::new();
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&called);
        bar.set_on_pause_toggle(move || c.set(true));
        if let Some(ref mut cb) = bar.on_pause_toggle {
            cb();
        }
        assert!(called.get());
    }

    #[test]
    fn test_stop_callback() {
        let mut bar = RecorderStatusBar::new();
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&called);
        bar.set_on_stop(move || c.set(true));
        if let Some(ref mut cb) = bar.on_stop {
            cb();
        }
        assert!(called.get());
    }

    #[test]
    fn test_new_default_not_paused() {
        let bar = RecorderStatusBar::default();
        assert!(!bar.is_paused());
    }

    #[test]
    fn test_show_and_remove_cycles() {
        let mut bar = RecorderStatusBar::new();
        // show is a no-op on native.
        assert!(bar.show().is_ok());
        bar.set_paused(true);
        assert!(bar.is_paused());
        bar.remove();
        assert!(!bar.is_paused());
    }

    #[test]
    fn test_update_no_panic() {
        let bar = RecorderStatusBar::new();
        // update is a no-op on native — should not panic.
        bar.update(42_000.0);
        bar.update(3_600_000.0);
    }
}
