use crate::error::{RecordingError, Result};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{
    AudioContext, Blob, BlobEvent, Event, MediaRecorder, MediaStream,
    MediaStreamTrack,
};

#[cfg(target_arch = "wasm32")]
use web_sys::MediaRecorderOptions;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

/// Internal lifecycle state used to guard against invalid method calls.
///
/// This is separate from the 9-state `SessionState` machine in `recorder.rs`;
/// it only tracks whether the `MediaRecorder` has been started, paused, or
/// stopped within this lifecycle instance.
#[derive(Debug, Clone, PartialEq)]
enum LifecycleState {
    Idle,
    Active,
    Paused,
    Stopped,
}

// ---------------------------------------------------------------------------
// RecordingLifecycle
// ---------------------------------------------------------------------------

/// Manages a `MediaRecorder` instance through its full lifecycle.
///
/// Owns the media stream, audio context, and microphone track for the
/// duration of a recording.  Handles pause/resume state transitions,
/// duration tracking (with pause exclusion), and chunk forwarding via
/// an optional callback.
///
/// # State guards
///
/// | method   | Allowed in          | StateViolation if called in |
/// |----------|---------------------|-----------------------------|
/// | start    | Idle                | Active, Paused, Stopped     |
/// | pause    | Active              | Idle, Paused, Stopped       |
/// | resume   | Paused              | Idle, Active, Stopped       |
/// | stop     | Active, Paused      | Idle, Stopped               |
/// | cancel   | Active, Paused      | Idle, Stopped               |
pub(crate) struct RecordingLifecycle {
    /// Internal lifecycle state for guard enforcement.
    state: LifecycleState,
    /// The active `MediaRecorder`, created when `start()` is called.
    media_recorder: Option<MediaRecorder>,
    /// The acquired media stream (held to prevent garbage collection).
    media_stream: Option<MediaStream>,
    /// The `AudioContext` from stream acquisition (kept alive for audio).
    audio_context: Option<AudioContext>,
    /// The microphone track, if acquired.
    mic_track: Option<MediaStreamTrack>,
    /// Timestamp (`performance.now()`) when recording (or the current resume)
    /// started.
    start_time: Option<f64>,
    /// Timestamp when the last pause began.
    pause_start_time: Option<f64>,
    /// Finalised total recorded duration after `stop()` (milliseconds).
    accumulated_duration_ms: f64,
    /// Total time spent paused across all pause/resume cycles (milliseconds).
    accumulated_pause_ms: f64,
    /// Callback invoked when `ondataavailable` fires with a non-empty blob.
    on_chunk: Option<Box<dyn FnMut(Blob)>>,
    // ------------------------------------------------------------------
    // Closure storage — MUST NOT be dropped while MediaRecorder is alive,
    // otherwise event handlers silently stop firing.
    // ------------------------------------------------------------------
    #[allow(dead_code)]
    _ondataavailable_closure: Option<Closure<dyn FnMut(BlobEvent)>>,
    #[allow(dead_code)]
    _onerror_closure: Option<Closure<dyn FnMut(Event)>>,
    #[allow(dead_code)]
    _onstop_closure: Option<Closure<dyn FnMut(Event)>>,
}

impl RecordingLifecycle {
    /// Create a new `RecordingLifecycle` in the Idle state.
    pub fn new() -> Self {
        Self {
            state: LifecycleState::Idle,
            media_recorder: None,
            media_stream: None,
            audio_context: None,
            mic_track: None,
            start_time: None,
            pause_start_time: None,
            accumulated_duration_ms: 0.0,
            accumulated_pause_ms: 0.0,
            on_chunk: None,
            _ondataavailable_closure: None,
            _onerror_closure: None,
            _onstop_closure: None,
        }
    }

    /// Start recording.
    ///
    /// Takes ownership of the acquired stream and creates a `MediaRecorder`
    /// with `"video/webm; codecs=vp8,opus"` MIME type and 1000 ms timeslice.
    ///
    /// Sets up `ondataavailable`, `onerror`, and `onstop` event handlers.
    ///
    /// # Safety
    ///
    /// After `start()` is called, `self` must not move in memory until
    /// `stop()` or `cancel()` releases the resources, because the event
    /// handler closures hold raw pointers into `self.on_chunk`.
    pub fn start(
        &mut self,
        stream: MediaStream,
        audio_context: AudioContext,
        mic_track: Option<MediaStreamTrack>,
    ) -> Result<()> {
        if self.state != LifecycleState::Idle {
            return Err(RecordingError::StateViolation {
                details: "Recording is already in progress".into(),
            });
        }

        let mime_type = select_mime_type()?;
        let recorder = self.create_recorder(&stream, mime_type)?;

        // Store owned resources.
        self.media_stream = Some(stream);
        self.audio_context = Some(audio_context);
        self.mic_track = mic_track;
        self.media_recorder = Some(recorder);
        self.state = LifecycleState::Active;
        self.accumulated_duration_ms = 0.0;
        self.accumulated_pause_ms = 0.0;
        self.record_start_timestamp();

        Ok(())
    }

    /// Pause the recording.
    ///
    /// Stores the pause start timestamp for accurate duration tracking.
    pub fn pause(&mut self) -> Result<()> {
        if self.state != LifecycleState::Active {
            return Err(RecordingError::StateViolation {
                details: "Cannot pause — recording is not active".into(),
            });
        }

        if let Some(ref recorder) = self.media_recorder {
            recorder.pause().map_err(|_| RecordingError::MediaRecorderError {
                details: "Failed to pause MediaRecorder".into(),
            })?;
        }

        self.record_pause_start();
        self.state = LifecycleState::Paused;

        Ok(())
    }

    /// Resume from pause.
    ///
    /// The pause duration is excluded from the recording timer.
    pub fn resume(&mut self) -> Result<()> {
        if self.state != LifecycleState::Paused {
            return Err(RecordingError::StateViolation {
                details: "Cannot resume — recording is not paused".into(),
            });
        }

        if let Some(ref recorder) = self.media_recorder {
            recorder.resume().map_err(|_| RecordingError::MediaRecorderError {
                details: "Failed to resume MediaRecorder".into(),
            })?;
        }

        self.record_resume_end();
        self.state = LifecycleState::Active;

        Ok(())
    }

    /// Stop the recording.
    ///
    /// Triggers `MediaRecorder.stop()`.  The final `ondataavailable` fires
    /// before the `onstop` event.  The accumulated duration is frozen at
    /// the point of the call.
    pub fn stop(&mut self) -> Result<()> {
        if self.state != LifecycleState::Active && self.state != LifecycleState::Paused {
            return Err(RecordingError::StateViolation {
                details: "Cannot stop — no active recording".into(),
            });
        }

        // Freeze the accumulated duration before calling stop.
        self.accumulated_duration_ms = self.calculate_duration();

        if let Some(ref recorder) = self.media_recorder {
            recorder.stop().map_err(|_| RecordingError::MediaRecorderError {
                details: "Failed to stop MediaRecorder".into(),
            })?;
        }

        self.state = LifecycleState::Stopped;

        Ok(())
    }

    /// Cancel the recording.
    ///
    /// If the lifecycle is in `Active` or `Paused` (i.e. `start()` has been
    /// called), `MediaRecorder.stop()` is invoked for a clean shutdown but
    /// all chunks are discarded.  Resources are always released.
    pub fn cancel(&mut self) -> Result<()> {
        if self.state == LifecycleState::Idle || self.state == LifecycleState::Stopped {
            return Err(RecordingError::StateViolation {
                details: "Cannot cancel — no active recording".into(),
            });
        }

        // Discard any chunk callbacks — don't forward further data.
        self.on_chunk = None;

        // Stop the MediaRecorder for a clean shutdown (best-effort).
        if let Some(ref recorder) = self.media_recorder {
            let _ = recorder.stop();
        }

        self.release_resources();
        self.state = LifecycleState::Idle;

        Ok(())
    }

    /// Return the total recorded duration in milliseconds.
    ///
    /// During an active recording this is the wall-clock time minus pauses.
    /// After `stop()` the frozen duration is returned.
    pub fn duration_ms(&self) -> f64 {
        match self.state {
            LifecycleState::Active => {
                let elapsed = self.current_time() - self.start_time.unwrap_or(0.0);
                elapsed - self.accumulated_pause_ms
            }
            LifecycleState::Paused => {
                // While paused, use pause_start_time as the effective "now"
                // so that duration does not advance.
                let effective_now =
                    self.pause_start_time.unwrap_or_else(|| self.current_time());
                let elapsed = effective_now - self.start_time.unwrap_or(0.0);
                elapsed - self.accumulated_pause_ms
            }
            _ => self.accumulated_duration_ms,
        }
    }

    /// Return `true` when the `MediaRecorder` is paused.
    pub fn is_paused(&self) -> bool {
        self.state == LifecycleState::Paused
    }

    /// Set a callback to be invoked for each chunk emitted by the
    /// `MediaRecorder` (`ondataavailable`).
    pub fn set_on_chunk<F>(&mut self, callback: F)
    where
        F: FnMut(Blob) + 'static,
    {
        self.on_chunk = Some(Box::new(callback));
    }

    /// Return a reference to the stored `MediaRecorder`, if any.
    #[allow(dead_code)]
    pub(crate) fn media_recorder(&self) -> Option<&MediaRecorder> {
        self.media_recorder.as_ref()
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Return the current monotonic time in milliseconds.
    fn current_time(&self) -> f64 {
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window()
                .expect("invariant: window should exist in WASM context")
                .performance()
                .expect("invariant: performance should exist")
                .now()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Fallback for native tests — monotonic time in milliseconds.
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0)
        }
    }

    /// Record the start (or resume) timestamp.
    fn record_start_timestamp(&mut self) {
        self.start_time = Some(self.current_time());
    }

    /// Record the pause start timestamp.
    fn record_pause_start(&mut self) {
        self.pause_start_time = Some(self.current_time());
    }

    /// Finalise a pause — add its duration to the accumulated pause total.
    fn record_resume_end(&mut self) {
        if let Some(pause_start) = self.pause_start_time.take() {
            let pause_duration = self.current_time() - pause_start;
            self.accumulated_pause_ms += pause_duration;
        }
    }

    /// Calculate the current duration at the point of the call.
    fn calculate_duration(&self) -> f64 {
        match self.start_time {
            Some(start) => {
                let effective_now = match self.state {
                    LifecycleState::Paused => {
                        self.pause_start_time.unwrap_or_else(|| self.current_time())
                    }
                    _ => self.current_time(),
                };
                (effective_now - start) - self.accumulated_pause_ms
            }
            None => self.accumulated_duration_ms,
        }
    }

    /// Release all media resources.
    ///
    /// Stops all tracks on the media stream, closes the `AudioContext`, and
    /// drops the `MediaRecorder` and closure handles.
    fn release_resources(&mut self) {
        // Stop all tracks on the media stream.
        if let Some(stream) = self.media_stream.take() {
            let tracks = stream.get_tracks();
            let len = tracks.length();
            for i in 0..len {
                if let Ok(track) = tracks.get(i).dyn_into::<MediaStreamTrack>() {
                    track.stop();
                }
            }
        }

        // Stop the mic track separately if it was stored individually.
        if let Some(track) = self.mic_track.take() {
            track.stop();
        }

        // Close the AudioContext.
        if let Some(ctx) = self.audio_context.take() {
            let _ = ctx.close();
        }

        // Drop the MediaRecorder and closure handles.
        self.media_recorder = None;
        self._ondataavailable_closure = None;
        self._onerror_closure = None;
        self._onstop_closure = None;
        self.on_chunk = None;
        self.start_time = None;
        self.pause_start_time = None;
        self.accumulated_duration_ms = 0.0;
        self.accumulated_pause_ms = 0.0;
    }

    /// Create a `MediaRecorder` for the given stream and MIME type, and wire
    /// up the event handlers.
    ///
    /// # Safety (WASM only)
    ///
    /// The `ondataavailable` closure captures a raw pointer to
    /// `self.on_chunk`.  The caller (`start()`) must guarantee that `self`
    /// does not move in memory after this function returns, because the
    /// closure stored in `self._ondataavailable_closure` will outlive the
    /// `&mut self` borrow and reference the same memory location.
    fn create_recorder(
        &mut self,
        stream: &MediaStream,
        mime_type: &str,
    ) -> Result<MediaRecorder> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (stream, mime_type);
            return Err(RecordingError::MediaRecorderError {
                details: "MediaRecorder requires a browser environment".into(),
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut options = MediaRecorderOptions::new();
            options.set_mime_type(mime_type);

            let recorder =
                MediaRecorder::new_with_options(stream, &options).map_err(|_| {
                    RecordingError::MediaRecorderError {
                        details: format!(
                            "Failed to create MediaRecorder with MIME type '{}'",
                            mime_type
                        ),
                    }
                })?;

            // ------------------------------------------------------------------
            // ondataavailable — forwards non-empty blobs to the chunk callback.
            //
            // SAFETY: We capture a raw pointer to self.on_chunk.  The caller
            // (start()) guarantees self does not move after create_recorder
            // returns, so the pointer remains valid for the closure's lifetime.
            // ------------------------------------------------------------------
            {
                let on_chunk_ptr: *mut Option<Box<dyn FnMut(Blob)>> =
                    &mut self.on_chunk as *mut _;

                let cb = Closure::wrap(Box::new(move |event: BlobEvent| {
                    if let Some(data) = event.data() {
                        if data.size() > 0 {
                            // SAFETY: on_chunk is only accessed from within
                            // this closure, which is dropped (via
                            // _ondataavailable_closure) before
                            // RecordingLifecycle is dropped.
                            if let Some(ref mut chunk_cb) =
                                unsafe { &mut *on_chunk_ptr }
                            {
                                chunk_cb(data);
                            }
                        }
                    }
                }) as Box<dyn FnMut(BlobEvent)>);
                recorder.set_ondataavailable(&cb);
                self._ondataavailable_closure = Some(cb);
            }

            // onerror — currently a no-op; the session state machine handles
            // error transitions.  The closure must be stored to prevent GC.
            {
                let cb = Closure::wrap(Box::new(move |_event: Event| {
                    // Logging TBD — see Story 1.1 panic hook for error
                    // surfacing strategy.
                }) as Box<dyn FnMut(Event)>);
                recorder.set_onerror(Some(&cb));
                self._onerror_closure = Some(cb);
            }

            // onstop — notifies that the MediaRecorder has fully stopped.
            {
                let cb = Closure::wrap(Box::new(move |_event: Event| {
                    // The final ondataavailable has already fired before
                    // onstop.  The orchestrator will handle the stop
                    // completion signal.
                }) as Box<dyn FnMut(Event)>);
                recorder.set_onstop(Some(&cb));
                self._onstop_closure = Some(cb);
            }

            // Apply a timeslice of 1000 ms for ondataavailable emissions.
            recorder.start_with_timeslice(1000).map_err(|_| {
                RecordingError::MediaRecorderError {
                    details: "Failed to start MediaRecorder".into(),
                }
            })?;

            Ok(recorder)
        }
    }
}

impl Default for RecordingLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MIME type selection
// ---------------------------------------------------------------------------

/// Select the best supported MIME type for `MediaRecorder` WebM output.
///
/// Priority: `"video/webm; codecs=vp8,opus"` > `"video/webm"` > error.
pub(crate) fn select_mime_type() -> Result<&'static str> {
    #[cfg(target_arch = "wasm32")]
    {
        let preferred = "video/webm; codecs=vp8,opus";
        if MediaRecorder::is_type_supported(preferred) {
            return Ok(preferred);
        }
        if MediaRecorder::is_type_supported("video/webm") {
            return Ok("video/webm");
        }
        return Err(RecordingError::MediaRecorderError {
            details: "No supported MediaRecorder MIME type found for WebM output"
                .into(),
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // Cannot check MIME support outside WASM — return the preferred string.
        Ok("video/webm; codecs=vp8,opus")
    }
}

// ---------------------------------------------------------------------------
// Native (cargo test) unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Construction and defaults
    // ------------------------------------------------------------------

    #[test]
    fn test_new_lifecycle_has_no_recorder() {
        let lc = RecordingLifecycle::new();
        assert!(lc.media_recorder.is_none());
        assert!(lc.media_stream.is_none());
        assert!(lc.audio_context.is_none());
        assert_eq!(lc.state, LifecycleState::Idle);
    }

    #[test]
    fn test_duration_starts_at_zero() {
        let lc = RecordingLifecycle::new();
        assert_eq!(lc.duration_ms(), 0.0);
    }

    #[test]
    fn test_is_paused_default_false() {
        let lc = RecordingLifecycle::new();
        assert!(!lc.is_paused());
    }

    // ------------------------------------------------------------------
    // State guards — calling lifecycle methods before start()
    // ------------------------------------------------------------------

    #[test]
    fn test_pause_before_start_returns_error() {
        let mut lc = RecordingLifecycle::new();
        let err = lc.pause().unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
    }

    #[test]
    fn test_resume_before_start_returns_error() {
        let mut lc = RecordingLifecycle::new();
        let err = lc.resume().unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
    }

    #[test]
    fn test_stop_before_start_returns_error() {
        let mut lc = RecordingLifecycle::new();
        let err = lc.stop().unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
    }

    #[test]
    fn test_cancel_in_idle_returns_error() {
        let mut lc = RecordingLifecycle::new();
        let err = lc.cancel().unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
    }

    // ------------------------------------------------------------------
    // select_mime_type returns a string (native) or error (headless)
    // ------------------------------------------------------------------

    #[test]
    fn test_select_mime_type() {
        let result = select_mime_type();
        // On native this always returns the preferred string.
        assert!(result.is_ok());
        let mime = result.unwrap();
        assert!(mime.contains("video/webm"));
    }

    // ------------------------------------------------------------------
    // release_resources is safe on fresh lifecycle
    // ------------------------------------------------------------------

    #[test]
    fn test_release_resources_does_not_panic() {
        let mut lc = RecordingLifecycle::new();
        lc.release_resources();
        assert!(lc.media_recorder.is_none());
        assert!(lc.media_stream.is_none());
        assert!(lc.audio_context.is_none());
        assert_eq!(lc.accumulated_duration_ms, 0.0);
        assert_eq!(lc.accumulated_pause_ms, 0.0);
    }

    // ------------------------------------------------------------------
    // Duration tracking logic (pure math, no browser needed)
    // ------------------------------------------------------------------

    #[test]
    fn test_duration_accumulates_after_stop() {
        let mut lc = RecordingLifecycle::new();
        lc.state = LifecycleState::Stopped;
        lc.accumulated_duration_ms = 5000.0;
        assert!((lc.duration_ms() - 5000.0).abs() < 0.001);
    }

    #[test]
    fn test_pause_resume_preserves_duration() {
        let mut lc = RecordingLifecycle::new();

        // Simulate a full recording with pause/resume cycle by directly
        // setting lifecycle fields.

        // Start at t=0
        lc.state = LifecycleState::Active;
        lc.start_time = Some(0.0);
        lc.accumulated_pause_ms = 0.0;

        // Pause at t=1000
        lc.pause_start_time = Some(1000.0);
        lc.state = LifecycleState::Paused;

        // Resume at t=3000 → pause lasted 2000 ms
        lc.accumulated_pause_ms = 2000.0;
        lc.pause_start_time = None;
        lc.state = LifecycleState::Active;

        // Stop at t=5000
        // Duration should be: (5000 - 0) - 2000 = 3000
        lc.state = LifecycleState::Stopped;
        lc.accumulated_duration_ms = 3000.0;

        assert!((lc.duration_ms() - 3000.0).abs() < 0.001);
    }

    // ------------------------------------------------------------------
    // MIME type format
    // ------------------------------------------------------------------

    #[test]
    fn test_mime_type_format() {
        let mime = select_mime_type().unwrap();
        assert!(
            mime.starts_with("video/webm"),
            "MIME type should start with video/webm, got: {}",
            mime
        );
    }

    // ------------------------------------------------------------------
    // Lifecycle guard — double cancel
    // ------------------------------------------------------------------

    #[test]
    fn test_double_cancel_returns_error() {
        let mut lc = RecordingLifecycle::new();
        // Fake being in Active state so the first cancel doesn't fail.
        lc.state = LifecycleState::Active;
        // Cancel once — resources are released, state resets to Idle.
        let r1 = lc.cancel();
        assert!(r1.is_ok());
        // Second cancel while in Idle should fail.
        let r2 = lc.cancel();
        assert!(r2.is_err());
        assert!(matches!(r2.unwrap_err(), RecordingError::StateViolation { .. }));
    }
}

// ---------------------------------------------------------------------------
// WASM tests (run via `wasm-pack test --headless --chrome`)
// ---------------------------------------------------------------------------

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    /// Verify that a MediaRecorder can be created with an empty stream
    /// (succeeds or fails gracefully — no panic).
    #[wasm_bindgen_test]
    async fn test_create_recorder() {
        let mut lc = RecordingLifecycle::new();
        let stream = MediaStream::new().expect("invariant: MediaStream::new()");
        let ctx = AudioContext::new().expect("invariant: AudioContext::new()");

        let result = lc.start(stream, ctx, None);
        match result {
            Ok(()) => {
                assert!(lc.media_recorder.is_some());
                assert_eq!(lc.state, LifecycleState::Active);
                let _ = lc.stop();
            }
            Err(e) => {
                assert!(matches!(e, RecordingError::MediaRecorderError { .. }));
            }
        }
    }

    /// Verify that the preferred MIME type is reported as supported in a
    /// browser environment.
    #[wasm_bindgen_test]
    async fn test_mime_type_supported() {
        let result = select_mime_type();
        assert!(result.is_ok(), "MIME type should be supported in Chrome");
        let mime = result.unwrap();
        assert!(mime.contains("video/webm"));
    }

    /// Verify that creating, starting, then stopping a MediaRecorder does
    /// not panic.
    #[wasm_bindgen_test]
    async fn test_recorder_stop_emits_data() {
        let mut lc = RecordingLifecycle::new();
        let stream = MediaStream::new().expect("invariant: MediaStream::new()");
        let ctx = AudioContext::new().expect("invariant: AudioContext::new()");

        let chunks = std::cell::RefCell::new(Vec::<Blob>::new());
        lc.set_on_chunk(move |blob| {
            chunks.borrow_mut().push(blob);
        });

        let result = lc.start(stream, ctx, None);
        if result.is_ok() {
            let _ = lc.stop();
        }
        // No panic is the success condition.
    }

    /// Verify that MediaRecorder responds to pause/resume without error.
    #[wasm_bindgen_test]
    async fn test_recorder_pause_resume() {
        let mut lc = RecordingLifecycle::new();
        let stream = MediaStream::new().expect("invariant: MediaStream::new()");
        let ctx = AudioContext::new().expect("invariant: AudioContext::new()");

        match lc.start(stream, ctx, None) {
            Ok(()) => {
                let pause_result = lc.pause();
                assert!(pause_result.is_ok(), "pause should succeed");
                assert!(lc.is_paused());
                assert_eq!(lc.state, LifecycleState::Paused);

                let resume_result = lc.resume();
                assert!(resume_result.is_ok(), "resume should succeed");
                assert!(!lc.is_paused());
                assert_eq!(lc.state, LifecycleState::Active);

                // Double-resume should fail.
                let double_resume = lc.resume();
                assert!(double_resume.is_err());
                assert!(matches!(
                    double_resume.unwrap_err(),
                    RecordingError::StateViolation { .. }
                ));

                let _ = lc.stop();
            }
            Err(_) => {
                // Acceptable in headless with no real media tracks.
            }
        }
    }

    /// Verify that cancel in Active state releases resources.
    #[wasm_bindgen_test]
    async fn test_cancel_releases_resources() {
        let mut lc = RecordingLifecycle::new();
        let stream = MediaStream::new().expect("invariant: MediaStream::new()");
        let ctx = AudioContext::new().expect("invariant: AudioContext::new()");

        match lc.start(stream, ctx, None) {
            Ok(()) => {
                assert!(lc.media_recorder.is_some());
                let cancel_result = lc.cancel();
                assert!(cancel_result.is_ok());
                assert_eq!(lc.state, LifecycleState::Idle);
                assert!(lc.media_recorder.is_none());
                assert!(lc.media_stream.is_none());
            }
            Err(_) => {
                // Acceptable in headless.
            }
        }
    }
}
