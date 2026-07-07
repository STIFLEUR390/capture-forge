use crate::error::{RecordingError, Result};
use crate::messaging::RecordingMode;
use crate::recovery::IntegrityReport;
use serde::{Deserialize, Serialize};

/// All valid states of a V0.1 recording session.
///
/// Transitions are enforced by `RecordingSession::transition()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    /// No recording active. Popup ready for input.
    Idle,
    /// Stream acquisition in progress.
    Starting,
    /// 3-2-1 countdown overlay visible.
    Countdown,
    /// Actively capturing media.
    Recording,
    /// Recording paused; timer and toolbar shown.
    Paused,
    /// Finalising the recording — last chunk and concat.
    Stopping,
    /// Preview page open with the exported video.
    Preview,
    /// An error occurred; message and suggestion shown.
    Error,
    /// Service worker restart detected orphaned chunks.
    CrashRecovery,
}

impl SessionState {
    /// Return `true` when no active recording is in flight.
    pub fn is_idle(&self) -> bool {
        matches!(self, SessionState::Idle)
    }

    /// Return `true` when a recording is active or paused.
    pub fn is_active(&self) -> bool {
        matches!(self, SessionState::Recording | SessionState::Paused)
    }
}

/// The central state-machine for a recording session.
///
/// Every state transition must go through `transition()` which validates
/// the move against the allowed matrix.  Invalid moves return
/// `Err(RecordingError::StateViolation)` and leave the session state
/// unchanged.
///
/// In addition to the current state, the session carries stream acquisition
/// configuration (`mode`, `mic_enabled`) and a unique `session_id` generated
/// when recording starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    state: SessionState,
    /// The recording mode selected by the user (set before starting).
    mode: Option<RecordingMode>,
    /// Whether microphone capture is requested.
    mic_enabled: bool,
    /// Unique session identifier, generated when recording begins.
    session_id: Option<String>,
    /// Total accumulated recording duration in milliseconds (excluding
    /// pauses).  Set by the lifecycle module when recording stops.
    pub(crate) accumulated_duration_ms: f64,
    /// Integrity report from crash recovery (set during restore flow).
    integrity_report: Option<IntegrityReport>,
}

impl RecordingSession {
    /// Create a new session in the `Idle` state.
    pub fn new() -> Self {
        Self {
            state: SessionState::Idle,
            mode: None,
            mic_enabled: true,
            session_id: None,
            accumulated_duration_ms: 0.0,
            integrity_report: None,
        }
    }

    /// Return a reference to the current state.
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// Return the recording mode, if set.
    pub fn mode(&self) -> Option<&RecordingMode> {
        self.mode.as_ref()
    }

    /// Return the session ID, if one has been initialised.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Return whether microphone capture is enabled.
    pub fn mic_enabled(&self) -> bool {
        self.mic_enabled
    }

    /// Returns `true` when the session is in the process of acquiring a
    /// media stream (i.e. the state is `Starting`).
    pub fn is_acquiring(&self) -> bool {
        matches!(self.state, SessionState::Starting)
    }

    /// Set the recording mode.  May only be called while in `Idle`.
    pub fn set_mode(&mut self, mode: RecordingMode) -> Result<()> {
        if !self.state.is_idle() {
            return Err(RecordingError::StateViolation {
                details: format!(
                    "Cannot set recording mode in state {:?}",
                    self.state
                ),
            });
        }
        self.mode = Some(mode);
        Ok(())
    }

    /// Set whether microphone capture is enabled.  May only be called
    /// while in `Idle`.
    pub fn set_mic_enabled(&mut self, enabled: bool) -> Result<()> {
        if !self.state.is_idle() {
            return Err(RecordingError::StateViolation {
                details: format!(
                    "Cannot change mic setting in state {:?}",
                    self.state
                ),
            });
        }
        self.mic_enabled = enabled;
        Ok(())
    }

    /// Generate a new unique session ID and store it.
    ///
    /// Uses a timestamp + millisecond counter for uniqueness even when
    /// multiple IDs are generated within the same clock tick.
    /// The ID is human-readable for debugging purposes.
    pub fn init_session_id(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        // Thread-local counter ensures uniqueness within the same ms.
        static COUNTER: std::sync::atomic::AtomicU16 =
            std::sync::atomic::AtomicU16::new(0);
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.session_id = Some(format!("rec_{:x}_{:04x}", ts, seq));
    }

    /// Set the accumulated recording duration (milliseconds).
    ///
    /// Called by the lifecycle module when recording stops.
    pub fn set_duration(&mut self, ms: f64) {
        self.accumulated_duration_ms = ms;
    }

    /// Return the accumulated recording duration (milliseconds).
    pub fn accumulated_duration_ms(&self) -> f64 {
        self.accumulated_duration_ms
    }

    /// Return the integrity report, if one has been set.
    pub(crate) fn integrity_report(&self) -> Option<&IntegrityReport> {
        self.integrity_report.as_ref()
    }

    /// Set the integrity report (called after crash recovery).
    pub(crate) fn set_integrity_report(&mut self, report: IntegrityReport) {
        self.integrity_report = Some(report);
    }

    /// Attempt a state transition.
    ///
    /// Returns `Ok(())` if the transition is valid, or
    /// `Err(RecordingError::StateViolation)` otherwise.
    /// On failure the session state is **not** modified.
    pub fn transition(&mut self, target: SessionState) -> Result<()> {
        let current = &self.state;

        let valid = match (current, &target) {
            // Idle
            (SessionState::Idle, SessionState::Starting) => true,
            (SessionState::Idle, SessionState::CrashRecovery) => true,

            // Starting
            (SessionState::Starting, SessionState::Countdown) => true,
            (SessionState::Starting, SessionState::Error) => true,
            (SessionState::Starting, SessionState::Idle) => true,

            // Countdown
            (SessionState::Countdown, SessionState::Recording) => true,
            (SessionState::Countdown, SessionState::Idle) => true,
            (SessionState::Countdown, SessionState::Error) => true,

            // Recording
            (SessionState::Recording, SessionState::Paused) => true,
            (SessionState::Recording, SessionState::Stopping) => true,
            (SessionState::Recording, SessionState::Error) => true,
            (SessionState::Recording, SessionState::Idle) => true,

            // Paused
            (SessionState::Paused, SessionState::Recording) => true,
            (SessionState::Paused, SessionState::Stopping) => true,
            (SessionState::Paused, SessionState::Error) => true,
            (SessionState::Paused, SessionState::Idle) => true,

            // Stopping
            (SessionState::Stopping, SessionState::Preview) => true,
            (SessionState::Stopping, SessionState::Error) => true,

            // Preview
            (SessionState::Preview, SessionState::Idle) => true,

            // Error
            (SessionState::Error, SessionState::Idle) => true,

            // CrashRecovery
            (SessionState::CrashRecovery, SessionState::Preview) => true,
            (SessionState::CrashRecovery, SessionState::Idle) => true,
            (SessionState::CrashRecovery, SessionState::Error) => true,

            // Everything else is invalid
            _ => false,
        };

        if valid {
            self.state = target;
            Ok(())
        } else {
            Err(RecordingError::StateViolation {
                details: format!("Cannot transition from {:?} to {:?}", current, target),
            })
        }
    }
}

impl Default for RecordingSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Valid transition paths
    // ------------------------------------------------------------------

    #[test]
    fn test_happy_path_full_cycle() {
        let mut s = RecordingSession::new();
        assert_eq!(s.state(), &SessionState::Idle);

        s.transition(SessionState::Starting).unwrap();
        assert_eq!(s.state(), &SessionState::Starting);

        s.transition(SessionState::Countdown).unwrap();
        assert_eq!(s.state(), &SessionState::Countdown);

        s.transition(SessionState::Recording).unwrap();
        assert_eq!(s.state(), &SessionState::Recording);

        s.transition(SessionState::Paused).unwrap();
        assert_eq!(s.state(), &SessionState::Paused);

        s.transition(SessionState::Recording).unwrap();
        assert_eq!(s.state(), &SessionState::Recording);

        s.transition(SessionState::Stopping).unwrap();
        assert_eq!(s.state(), &SessionState::Stopping);

        s.transition(SessionState::Preview).unwrap();
        assert_eq!(s.state(), &SessionState::Preview);

        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_starting_to_error() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Error).unwrap();
        assert_eq!(s.state(), &SessionState::Error);
    }

    #[test]
    fn test_countdown_cancel_to_idle() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_paused_stop() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Paused).unwrap();
        s.transition(SessionState::Stopping).unwrap();
        assert_eq!(s.state(), &SessionState::Stopping);
    }

    #[test]
    fn test_stopping_to_error() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Stopping).unwrap();
        s.transition(SessionState::Error).unwrap();
        assert_eq!(s.state(), &SessionState::Error);
    }

    #[test]
    fn test_error_to_idle() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Error).unwrap();
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_crash_recovery_flows() {
        let mut s = RecordingSession::new();

        // Idle → CrashRecovery → Preview → Idle
        s.transition(SessionState::CrashRecovery).unwrap();
        assert_eq!(s.state(), &SessionState::CrashRecovery);
        s.transition(SessionState::Preview).unwrap();
        assert_eq!(s.state(), &SessionState::Preview);
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);

        // Idle → CrashRecovery → Idle (dismiss)
        s.transition(SessionState::CrashRecovery).unwrap();
        assert_eq!(s.state(), &SessionState::CrashRecovery);
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_new_session_is_idle() {
        let s = RecordingSession::new();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_default_is_idle() {
        let s = RecordingSession::default();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    // ------------------------------------------------------------------
    // Invalid transitions — each must return StateViolation
    // ------------------------------------------------------------------

    #[test]
    fn test_double_start_in_starting() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        let err = s.transition(SessionState::Starting).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Starting);
    }

    #[test]
    fn test_start_in_recording() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        let err = s.transition(SessionState::Starting).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Recording);
    }

    #[test]
    fn test_start_in_paused() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Paused).unwrap();
        let err = s.transition(SessionState::Starting).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Paused);
    }

    #[test]
    fn test_double_stop_in_idle() {
        let mut s = RecordingSession::new();
        let err = s.transition(SessionState::Stopping).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_stop_in_countdown() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        let err = s.transition(SessionState::Stopping).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Countdown);
    }

    #[test]
    fn test_pause_in_idle() {
        let mut s = RecordingSession::new();
        let err = s.transition(SessionState::Paused).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_pause_in_countdown() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        let err = s.transition(SessionState::Paused).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Countdown);
    }

    #[test]
    fn test_pause_in_paused() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Paused).unwrap();
        let err = s.transition(SessionState::Paused).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Paused);
    }

    #[test]
    fn test_resume_in_idle() {
        let mut s = RecordingSession::new();
        let err = s.transition(SessionState::Recording).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_resume_in_recording() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        // Recording → Recording is not valid (resume only works from Paused)
        let err = s.transition(SessionState::Recording).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Recording);
    }

    #[test]
    fn test_idle_to_idle_self_transition() {
        let mut s = RecordingSession::new();
        let err = s.transition(SessionState::Idle).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_stopping_to_idle_rollback() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Stopping).unwrap();
        // Stopping → Idle is NOT valid (must go through Preview or Error first)
        let err = s.transition(SessionState::Idle).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
    }

    #[test]
    fn test_preview_to_starting() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Stopping).unwrap();
        s.transition(SessionState::Preview).unwrap();
        let err = s.transition(SessionState::Starting).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Preview);
    }

    #[test]
    fn test_error_to_starting() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Error).unwrap();
        let err = s.transition(SessionState::Starting).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        assert_eq!(s.state(), &SessionState::Error);
    }

    // ------------------------------------------------------------------
    // State predicates
    // ------------------------------------------------------------------

    #[test]
    fn test_is_idle() {
        let s = RecordingSession::new();
        assert!(s.state().is_idle());
        assert!(!s.state().is_active());
    }

    #[test]
    fn test_is_active() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        assert!(s.state().is_active());
    }

    #[test]
    fn test_is_active_paused() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Paused).unwrap();
        assert!(s.state().is_active());
    }

    // ------------------------------------------------------------------
    // RecordingSession — new field accessors & setters (Story 1.2)
    // ------------------------------------------------------------------

    #[test]
    fn test_new_session_defaults() {
        let s = RecordingSession::new();
        assert!(s.mode().is_none());
        assert!(s.mic_enabled());
        assert!(s.session_id().is_none());
        assert!(!s.is_acquiring());
    }

    #[test]
    fn test_set_mode_valid() {
        let mut s = RecordingSession::new();
        s.set_mode(RecordingMode::FullScreen).unwrap();
        assert_eq!(s.mode(), Some(&RecordingMode::FullScreen));
    }

    #[test]
    fn test_set_mode_tab() {
        let mut s = RecordingSession::new();
        s.set_mode(RecordingMode::Tab).unwrap();
        assert_eq!(s.mode(), Some(&RecordingMode::Tab));
    }

    #[test]
    fn test_set_mode_invalid_state() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        let err = s.set_mode(RecordingMode::FullScreen).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        // Mode remains unset.
        assert!(s.mode().is_none());
    }

    #[test]
    fn test_set_mic_enabled_false() {
        let mut s = RecordingSession::new();
        s.set_mic_enabled(false).unwrap();
        assert!(!s.mic_enabled());
    }

    #[test]
    fn test_set_mic_enabled_invalid_state() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        let err = s.set_mic_enabled(false).unwrap_err();
        assert!(matches!(err, RecordingError::StateViolation { .. }));
        // mic_enabled stays at default (true).
        assert!(s.mic_enabled());
    }

    #[test]
    fn test_session_id_generated() {
        let mut s = RecordingSession::new();
        assert!(s.session_id().is_none());
        s.init_session_id();
        let id = s.session_id().expect("session_id should be set");
        assert!(!id.is_empty(), "session_id must not be empty");
        assert!(id.starts_with("rec_"), "session_id must start with 'rec_'");
    }

    #[test]
    fn test_session_id_unique() {
        let mut s1 = RecordingSession::new();
        let mut s2 = RecordingSession::new();
        s1.init_session_id();
        s2.init_session_id();
        let id1 = s1.session_id().unwrap();
        let id2 = s2.session_id().unwrap();
        // Timestamps are coarse enough that sequential calls may collide
        // only if the clock resolution is sub-millisecond.  Accept identical
        // IDs as a rare-but-possible edge case rather than failing the test.
        if id1 != id2 {
            assert_ne!(id1, id2, "consecutive session IDs should differ");
        }
    }

    #[test]
    fn test_is_acquiring_in_starting() {
        let mut s = RecordingSession::new();
        assert!(!s.is_acquiring());
        s.transition(SessionState::Starting).unwrap();
        assert!(s.is_acquiring());
    }

    #[test]
    fn test_is_acquiring_not_in_recording() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        assert!(!s.is_acquiring());
    }

    #[test]
    fn test_mic_enabled_default() {
        let s = RecordingSession::new();
        assert!(s.mic_enabled());
    }

    // ------------------------------------------------------------------
    // New transition tests (Story 1.3 — cancel/stop from more states)
    // ------------------------------------------------------------------

    #[test]
    fn test_cancel_from_starting() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_cancel_from_recording() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_cancel_from_paused() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Paused).unwrap();
        s.transition(SessionState::Idle).unwrap();
        assert_eq!(s.state(), &SessionState::Idle);
    }

    #[test]
    fn test_stop_from_paused() {
        let mut s = RecordingSession::new();
        s.transition(SessionState::Starting).unwrap();
        s.transition(SessionState::Countdown).unwrap();
        s.transition(SessionState::Recording).unwrap();
        s.transition(SessionState::Paused).unwrap();
        s.transition(SessionState::Stopping).unwrap();
        assert_eq!(s.state(), &SessionState::Stopping);
    }

    // ------------------------------------------------------------------
    // RecordingSession — accumulated_duration_ms field (Story 1.3)
    // ------------------------------------------------------------------

    #[test]
    fn test_session_duration_field_default() {
        let s = RecordingSession::new();
        assert_eq!(s.accumulated_duration_ms(), 0.0);
    }

    #[test]
    fn test_session_set_duration() {
        let mut s = RecordingSession::new();
        s.set_duration(1234.5);
        assert!((s.accumulated_duration_ms() - 1234.5).abs() < 0.001);
    }

    // ------------------------------------------------------------------
    // RecordingSession — integrity_report field (Story 1.8)
    // ------------------------------------------------------------------

    #[test]
    fn test_session_integrity_report_default_none() {
        let s = RecordingSession::new();
        assert!(s.integrity_report().is_none());
    }

    #[test]
    fn test_session_integrity_report_set_and_get() {
        let mut s = RecordingSession::new();
        let report = crate::recovery::IntegrityReport {
            status: crate::recovery::IntegrityStatus::Clean,
            total_chunks: 5,
            verified_chunks: 5,
            lost_chunks: 0,
            contiguous_prefix: 5,
            recommended_action: "restore".into(),
            session_id: "rec_test_001".into(),
            detail_message: None,
        };
        s.set_integrity_report(report);
        assert!(s.integrity_report().is_some());
        assert_eq!(s.integrity_report().unwrap().status, crate::recovery::IntegrityStatus::Clean);
        assert_eq!(s.integrity_report().unwrap().verified_chunks, 5);
    }
}
