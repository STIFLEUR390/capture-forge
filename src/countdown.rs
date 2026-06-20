use crate::error::Result;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    Document, Element, HtmlElement, HtmlSpanElement, KeyboardEvent, Node, ShadowRoot,
    ShadowRootInit, ShadowRootMode,
};

// ---------------------------------------------------------------------------
// CSS — inline in the shadow root (no external stylesheets in V0.1)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
const COUNTDOWN_CSS: &str = r#"
:host {
    all: initial;
    display: flex;
    align-items: center;
    justify-content: center;
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    z-index: 2147483647;
    background: rgba(0, 0, 0, 0.6);
    font-family: 'SF Mono', 'Cascadia Code', 'JetBrains Mono', 'Fira Code', Consolas, monospace;
    -webkit-font-smoothing: antialiased;
}

.countdown-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
}

.number {
    font-size: 72px;
    font-weight: 700;
    line-height: 1;
    color: #2563EB;
    opacity: 0;
    transform: scale(0.5);
    transition: opacity 0.2s ease-out, transform 0.2s ease-out;
    user-select: none;
}

.number.visible {
    opacity: 1;
    transform: scale(1);
}

.number.fade-out {
    opacity: 0;
    transform: scale(1.2);
}

@media (prefers-color-scheme: dark) {
    .number { color: #60A5FA; }
}

@keyframes countdown-scale {
    0%   { opacity: 0; transform: scale(0.5); }
    30%  { opacity: 1; transform: scale(1.1); }
    60%  { opacity: 1; transform: scale(1); }
    100% { opacity: 0; transform: scale(1.2); }
}

@media (prefers-reduced-motion: reduce) {
    .number.visible {
        opacity: 1;
        transform: scale(1);
        transition: opacity 0.3s ease-out;
    }
    .number.fade-out {
        opacity: 0;
        transform: scale(1);
        transition: opacity 0.4s ease-out;
    }
}

.ring-svg {
    width: 120px;
    height: 120px;
    transform: rotate(-90deg);
}

.ring-bg {
    fill: none;
    stroke: rgba(255, 255, 255, 0.15);
    stroke-width: 4;
}

.ring-fill {
    fill: none;
    stroke: #2563EB;
    stroke-width: 4;
    stroke-linecap: round;
    stroke-dasharray: 282.74;
    stroke-dashoffset: 282.74;
    transition: stroke-dashoffset 1s linear;
}

@media (prefers-color-scheme: dark) {
    .ring-fill { stroke: #60A5FA; }
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

// The circumference of a circle with r=45: 2 * pi * 45 ≈ 282.74
#[cfg(target_arch = "wasm32")]
const RING_CIRCUMFERENCE: f64 = 282.74;

/// Manages the 3-2-1 countdown overlay rendered via shadow DOM.
///
/// The overlay is a full-viewport semi-transparent surface that displays
/// the countdown number and a filling circle ring.  It registers an Escape
/// keydown handler on the document that fires the `on_cancel` callback.
///
/// # State guards
///
/// | method | Allowed when        | Behaviour |
/// |--------|---------------------|-----------|
/// | show   | new() / after remove | Injects shadow DOM and starts interval |
/// | tick   | animation running   | Advances to next number (3→2→1→done) |
/// | remove | visible             | Cleans up DOM elements and handlers |
/// | reset  | any                 | Resets internal state for reuse |
pub(crate) struct CountdownOverlay {
    /// Current number being displayed (3, 2, or 1).
    current: u32,
    /// Set to true after all three numbers have been displayed.
    completed: bool,
    /// Callback invoked when Escape is pressed during the countdown.
    on_cancel: Option<Box<dyn FnMut()>>,
    /// Callback invoked when the countdown sequence finishes naturally.
    on_complete: Option<Box<dyn FnMut()>>,
    /// The `<div>` container attached to the document body.
    #[cfg(target_arch = "wasm32")]
    container: Option<Element>,
    /// The shadow root where all elements are rendered.
    #[cfg(target_arch = "wasm32")]
    shadow: Option<ShadowRoot>,
    /// The `<span>` displaying the current number.
    #[cfg(target_arch = "wasm32")]
    number_el: Option<HtmlSpanElement>,
    /// The `<circle>` whose stroke-dashoffset drives the ring fill.
    #[cfg(target_arch = "wasm32")]
    ring_el: Option<Element>,
    /// The `<div aria-live="assertive">` for screen reader announcements.
    #[cfg(target_arch = "wasm32")]
    aria_el: Option<HtmlSpanElement>,
    /// The keydown closure — must be kept alive while the handler is registered.
    #[cfg(target_arch = "wasm32")]
    _keydown_closure: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    /// The interval handle for countdown tick timing.
    #[cfg(target_arch = "wasm32")]
    _interval_handle: Option<i32>,
    /// Re-entrancy guard to prevent multiple `remove()` calls.
    #[cfg(target_arch = "wasm32")]
    removed: bool,
}

impl CountdownOverlay {
    /// Create a new `CountdownOverlay` in an unrendered state.
    pub(crate) fn new() -> Self {
        Self {
            current: 3,
            completed: false,
            on_cancel: None,
            on_complete: None,
            #[cfg(target_arch = "wasm32")]
            container: None,
            #[cfg(target_arch = "wasm32")]
            shadow: None,
            #[cfg(target_arch = "wasm32")]
            number_el: None,
            #[cfg(target_arch = "wasm32")]
            ring_el: None,
            #[cfg(target_arch = "wasm32")]
            aria_el: None,
            #[cfg(target_arch = "wasm32")]
            _keydown_closure: None,
            #[cfg(target_arch = "wasm32")]
            _interval_handle: None,
            #[cfg(target_arch = "wasm32")]
            removed: false,
        }
    }

    /// Render the countdown overlay by injecting shadow DOM into the active
    /// tab's document body.
    ///
    /// On native (non-WASM) this is a no-op that returns `Ok(())`.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn show(&mut self) -> Result<()> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| crate::error::RecordingError::Unknown {
                details: "Cannot access document for countdown overlay".into(),
            })?;

        let body = document.body().ok_or_else(|| {
            crate::error::RecordingError::Unknown {
                details: "No document body for countdown overlay".into(),
            }
        })?;

        // Create container and attach shadow root.
        let container = document.create_element("div")?;
        container.set_attribute("data-capture-forge", "countdown")?;
        let shadow_init = ShadowRootInit::new(ShadowRootMode::Open);
        let shadow = container.attach_shadow(&shadow_init)?;

        // Inject inline CSS.
        let style = document.create_element("style")?;
        style.set_text_content(Some(COUNTDOWN_CSS));
        shadow.append_child(&style)?;

        // --- Build the countdown DOM tree ---
        let container_div = document.create_element("div")?;
        container_div.set_attribute("class", "countdown-container")?;

        // SVG ring.
        let svg = document.create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")?;
        svg.set_attribute("class", "ring-svg")?;
        svg.set_attribute("viewBox", "0 0 100 100")?;

        let ring_bg = document.create_element_ns(Some("http://www.w3.org/2000/svg"), "circle")?;
        ring_bg.set_attribute("class", "ring-bg")?;
        ring_bg.set_attribute("cx", "50")?;
        ring_bg.set_attribute("cy", "50")?;
        ring_bg.set_attribute("r", "45")?;
        svg.append_child(&ring_bg)?;

        let ring_fill = document.create_element_ns(Some("http://www.w3.org/2000/svg"), "circle")?;
        ring_fill.set_attribute("class", "ring-fill")?;
        ring_fill.set_attribute("cx", "50")?;
        ring_fill.set_attribute("cy", "50")?;
        ring_fill.set_attribute("r", "45")?;
        svg.append_child(&ring_fill)?;

        container_div.append_child(&svg)?;

        // Number display.
        let number_el = document.create_element("span")?;
        number_el.set_attribute("class", "number")?;
        number_el.set_text_content(Some(&self.current.to_string()));
        container_div.append_child(&number_el)?;

        // Screen-reader-only announcement region.
        let aria_el = document.create_element("span")?;
        aria_el.set_attribute("class", "sr-only")?;
        aria_el.set_attribute("aria-live", "assertive")?;
        aria_el.set_attribute("aria-atomic", "true")?;
        aria_el.set_text_content(Some(&self.current.to_string()));
        container_div.append_child(&aria_el)?;

        shadow.append_child(&container_div)?;

        // Store references for later updates and cleanup.
        self.container = Some(container);
        self.shadow = Some(shadow);
        self.number_el = Some(number_el.unchecked_into::<HtmlSpanElement>());
        self.ring_el = Some(ring_fill);
        self.aria_el = Some(aria_el.unchecked_into::<HtmlSpanElement>());

        // Trigger the "visible" state after a microtask so the CSS transition fires.
        {
            let number = self.number_el.clone();
            let cb = Closure::once(move || {
                if let Some(el) = number {
                    el.class_list().add_1("visible").ok();
                }
            });
            let _ = web_sys::window()
                .and_then(|w| w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 10));
            cb.forget();
        }

        // Register the Escape keydown handler on the document.
        {
            let container_ptr = self.container.as_ref().map(|c| c.clone() as Element);
            let on_cancel_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_cancel as *mut _;

            let cb = Closure::wrap(Box::new(move |event: KeyboardEvent| {
                if event.key() == "Escape" {
                    event.prevent_default();
                    event.stop_propagation();
                    // Remove the overlay immediately.
                    if let Some(ref c) = container_ptr {
                        let _ = c.remove();
                    }
                    // Fire the cancel callback.
                    if let Some(ref mut cb) = unsafe { &mut *on_cancel_ptr } {
                        cb();
                    }
                }
            }) as Box<dyn FnMut(KeyboardEvent)>);
            document.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())
                .map_err(|_| crate::error::RecordingError::Unknown {
                    details: "Failed to register countdown Escape handler".into(),
                })?;
            self._keydown_closure = Some(cb);
        }

        // Start the countdown interval.
        self.start_interval(&document);

        // Append to body.
        body.append_child(self.container.as_ref().expect("invariant: container set"))?;

        Ok(())
    }

    /// Native no-op: countdown DOM cannot be rendered outside a browser.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn show(&mut self) -> Result<()> {
        // On native, state is tracked for unit testing.
        self.current = 3;
        self.completed = false;
        Ok(())
    }

    /// Start the 1-second interval that drives number and ring updates.
    #[cfg(target_arch = "wasm32")]
    fn start_interval(&mut self, document: &Document) {
        // We use a counter approach: each tick advances the state.
        // Total ticks: 4 (number 3 visible → tick1: show 2, tick2: show 1, tick3: complete)
        let elapsed = std::cell::Cell::new(0u32);
        let number_ref = self.number_el.clone();
        let ring_ref = self.ring_el.clone();
        let aria_ref = self.aria_el.clone();
        let on_complete_ptr: *mut Option<Box<dyn FnMut()>> = &mut self.on_complete as *mut _;

        let cb = Closure::wrap(Box::new(move || {
            let tick = elapsed.get();
            elapsed.set(tick + 1);

            match tick {
                0 => {
                    // Show "2": fade out current, update text, fade in.
                    if let Some(ref num) = number_ref {
                        num.class_list().remove_1("visible").ok();
                        num.class_list().add_1("fade-out").ok();
                    }
                }
                1 => {
                    // Mid-point: update number text to "2", reset animation classes.
                    if let Some(ref num) = number_ref {
                        num.set_text_content(Some("2"));
                        num.class_list().remove_1("fade-out").ok();
                        // Small delay for the class removal to take effect.
                    }
                    if let Some(ref aria) = aria_ref {
                        aria.set_text_content(Some("2"));
                    }
                    // Reset ring.
                    if let Some(ref ring) = ring_ref {
                        ring.set_attribute("stroke-dashoffset", &RING_CIRCUMFERENCE.to_string()).ok();
                    }
                }
                2 => {
                    // Show "2" visible.
                    if let Some(ref num) = number_ref {
                        num.class_list().add_1("visible").ok();
                    }
                }
                3 => {
                    // Show "1": fade out "2".
                    if let Some(ref num) = number_ref {
                        num.class_list().remove_1("visible").ok();
                        num.class_list().add_1("fade-out").ok();
                    }
                }
                4 => {
                    // Mid-point: update text to "1".
                    if let Some(ref num) = number_ref {
                        num.set_text_content(Some("1"));
                        num.class_list().remove_1("fade-out").ok();
                    }
                    if let Some(ref aria) = aria_ref {
                        aria.set_text_content(Some("1"));
                    }
                    // Reset ring.
                    if let Some(ref ring) = ring_ref {
                        ring.set_attribute("stroke-dashoffset", &RING_CIRCUMFERENCE.to_string()).ok();
                    }
                }
                5 => {
                    // Show "1" visible.
                    if let Some(ref num) = number_ref {
                        num.class_list().add_1("visible").ok();
                    }
                }
                6 => {
                    // Complete: fade out "1".
                    if let Some(ref num) = number_ref {
                        num.class_list().remove_1("visible").ok();
                        num.class_list().add_1("fade-out").ok();
                    }
                }
                7 => {
                    // Countdown complete: fire callback.
                    if let Some(ref aria) = aria_ref {
                        aria.set_text_content(Some("Recording started"));
                    }

                    // Clear the interval.
                    // on_complete callback handles removal + state transition.
                    if let Some(ref mut cb) = unsafe { &mut *on_complete_ptr } {
                        cb();
                    }
                }
                _ => {}
            }

            // Advance ring fill on the ticks where numbers are shown.
            if tick <= 1 || tick == 3 || tick == 5 {
                if let Some(ref ring) = ring_ref {
                    let offset = RING_CIRCUMFERENCE - (RING_CIRCUMFERENCE * (tick as f64 + 1.0) / 3.0);
                    ring.set_attribute("stroke-dashoffset", &offset.to_string()).ok();
                }
            }
        }) as Box<dyn FnMut()>);

        let handle = web_sys::window()
            .and_then(|w| {
                w.set_interval_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    1000,
                ).ok()
            });
        self._interval_handle = handle;
        cb.forget();
    }

    /// Advance the countdown to the next number (3→2→1→done).
    ///
    /// On native, this simulates a countdown tick for testing state transitions.
    /// Returns `true` when the sequence is complete (all three numbers shown).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn tick(&mut self) -> bool {
        if self.completed {
            return true;
        }
        match self.current {
            3 => self.current = 2,
            2 => self.current = 1,
            1 => {
                self.completed = true;
                return true;
            }
            _ => unreachable!("countdown number out of range"),
        }
        false
    }

    /// Return the current countdown number (3, 2, or 1).
    pub(crate) fn current(&self) -> u32 {
        self.current
    }

    /// Return whether the countdown sequence has finished.
    pub(crate) fn is_complete(&self) -> bool {
        self.completed
    }

    /// Set the callback to fire when the user presses Escape.
    pub(crate) fn set_on_cancel<F>(&mut self, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.on_cancel = Some(Box::new(callback));
    }

    /// Set the callback to fire when the countdown sequence completes.
    pub(crate) fn set_on_complete<F>(&mut self, callback: F)
    where
        F: FnMut() + 'static,
    {
        self.on_complete = Some(Box::new(callback));
    }

    /// Remove the countdown overlay from the DOM and clean up handlers.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn remove(&mut self) {
        if self.removed {
            return;
        }
        self.removed = true;

        // Clear the interval.
        if let Some(handle) = self._interval_handle.take() {
            if let Some(w) = web_sys::window() {
                w.clear_interval_with_handle(handle);
            }
        }

        // Remove the keydown listener.
        if let Some(closure) = self._keydown_closure.take() {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                let _ = doc.remove_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
            }
        }

        // Remove the container from the DOM.
        if let Some(container) = self.container.take() {
            let _ = container.remove();
        }

        self.shadow = None;
        self.number_el = None;
        self.ring_el = None;
        self.aria_el = None;
    }

    /// Native no-op: nothing to clean up.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn remove(&mut self) {
        // State is tracked for testing.
        self.current = 3;
        self.completed = false;
    }

    /// Reset the countdown to its initial state for reuse.
    pub(crate) fn reset(&mut self) {
        self.current = 3;
        self.completed = false;
    }
}

impl Default for CountdownOverlay {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Native unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_overlay_starts_at_three() {
        let overlay = CountdownOverlay::new();
        assert_eq!(overlay.current(), 3);
        assert!(!overlay.is_complete());
    }

    #[test]
    fn test_countdown_sequence() {
        let mut overlay = CountdownOverlay::new();
        assert_eq!(overlay.current(), 3);

        // Tick 1: 3 → 2
        assert!(!overlay.tick());
        assert_eq!(overlay.current(), 2);
        assert!(!overlay.is_complete());

        // Tick 2: 2 → 1
        assert!(!overlay.tick());
        assert_eq!(overlay.current(), 1);
        assert!(!overlay.is_complete());

        // Tick 3: 1 → done
        assert!(overlay.tick());
        assert!(overlay.is_complete());
    }

    #[test]
    fn test_countdown_complete_returns_true() {
        let mut overlay = CountdownOverlay::new();
        overlay.tick();
        overlay.tick();
        assert!(overlay.tick()); // completes
        assert!(overlay.is_complete());
    }

    #[test]
    fn test_countdown_tick_past_complete() {
        let mut overlay = CountdownOverlay::new();
        overlay.tick();
        overlay.tick();
        overlay.tick(); // complete
        assert!(overlay.is_complete());
        // Further ticks return true without changing state.
        assert!(overlay.tick());
        assert!(overlay.is_complete());
    }

    #[test]
    fn test_reset_restores_state() {
        let mut overlay = CountdownOverlay::new();
        overlay.tick();
        overlay.tick();
        overlay.tick();
        assert!(overlay.is_complete());

        overlay.reset();
        assert_eq!(overlay.current(), 3);
        assert!(!overlay.is_complete());
    }

    #[test]
    fn test_set_on_cancel() {
        let mut overlay = CountdownOverlay::new();
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&called);
        overlay.set_on_cancel(move || c.set(true));
        if let Some(ref mut cb) = overlay.on_cancel {
            cb();
        }
        assert!(called.get());
    }

    #[test]
    fn test_set_on_complete() {
        let mut overlay = CountdownOverlay::new();
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let c = std::rc::Rc::clone(&called);
        overlay.set_on_complete(move || c.set(true));
        if let Some(ref mut cb) = overlay.on_complete {
            cb();
        }
        assert!(called.get());
    }

    #[test]
    fn test_remove_resets_state() {
        let mut overlay = CountdownOverlay::new();
        overlay.tick();
        overlay.tick();
        overlay.remove();
        // After remove, state is reset.
        assert_eq!(overlay.current(), 3);
        assert!(!overlay.is_complete());
    }

    #[test]
    fn test_countdown_default() {
        let overlay = CountdownOverlay::default();
        assert_eq!(overlay.current(), 3);
        assert!(!overlay.is_complete());
    }

    #[test]
    fn test_full_cycle_new_remove_new() {
        let mut overlay = CountdownOverlay::new();
        overlay.tick();
        overlay.tick();
        overlay.tick();
        assert!(overlay.is_complete());
        overlay.remove();
        assert_eq!(overlay.current(), 3);
        assert!(!overlay.is_complete());
    }
}
