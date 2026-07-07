use serde::{Deserialize, Serialize};

/// Source of media for a recording session.
///
/// Determines which Chrome API is used for stream acquisition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecordingMode {
    /// Entire display, selected via `getDisplayMedia`.
    FullScreen,
    /// A single browser tab, acquired via `chrome.tabCapture`.
    Tab,
}

/// All Inter-Process Communication messages for V0.1.
///
/// UI surfaces (popup, overlay, preview) send these to the background
/// message router, which dispatches them to the appropriate core module.
///
/// Every variant must derive `Serialize + Deserialize` for serde JSON transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtensionMessage {
    /// User requested recording start with the given mode.
    StartRecording {
        mode: RecordingMode,
    },
    /// User requested the recording to stop.
    StopRecording,
    /// User requested the recording to pause.
    PauseRecording,
    /// User requested the recording to resume from pause.
    ResumeRecording,
    /// User cancelled the recording (during Countdown or Recording).
    CancelRecording,
    /// Export pipeline completed; the preview page should open.
    VideoReady {
        session_id: String,
    },
    /// An error occurred in a core module.
    RecordingError {
        /// Stable error-code string, e.g. `"stream_acquisition_failed"`.
        code: String,
        /// Human-readable details for the UI.
        details: String,
    },
    /// Keepalive ping sent from the offscreen document to the service worker.
    KeepalivePing,
    /// Keepalive pong sent from the service worker back to the offscreen doc.
    KeepalivePong,
    /// Countdown sequence completed; session should transition to Recording.
    CountdownComplete,
    /// Request current streaming data from the recording module.
    GetStreamingData,
    /// Apply previously-fetched streaming data.
    ApplyStreamingData {
        data: String,
    },
    /// Preview page has been closed by the user (via Delete or Escape).
    PreviewClosed,
    /// Crash recovery event: orphaned chunks detected.
    RecoveryFound {
        session_id: String,
        chunk_count: u32,
    },
    /// User clicked the Restore button on the recovery toast.
    RestoreRecording {
        session_id: String,
    },
    /// User dismissed the recovery toast (clicked Dismiss or timeout fired).
    DismissRecovery,
}

impl ExtensionMessage {
    /// Return `true` if this message is a keepalive variant.
    pub fn is_keepalive(&self) -> bool {
        matches!(
            self,
            ExtensionMessage::KeepalivePing | ExtensionMessage::KeepalivePong
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: round-trip a message through serde JSON and assert equality.
    fn roundtrip(msg: &ExtensionMessage) {
        let json = serde_json::to_string(msg).expect("serialize");
        let deserialized: ExtensionMessage =
            serde_json::from_str(&json).expect("deserialize");
        // Structural comparison via debug formatting.
        assert_eq!(
            format!("{msg:?}"),
            format!("{deserialized:?}"),
            "Round-trip changed message content"
        );
    }

    #[test]
    fn test_start_recording() {
        roundtrip(&ExtensionMessage::StartRecording {
            mode: RecordingMode::FullScreen,
        });
        roundtrip(&ExtensionMessage::StartRecording {
            mode: RecordingMode::Tab,
        });
    }

    #[test]
    fn test_stop_recording() {
        roundtrip(&ExtensionMessage::StopRecording);
    }

    #[test]
    fn test_pause_recording() {
        roundtrip(&ExtensionMessage::PauseRecording);
    }

    #[test]
    fn test_resume_recording() {
        roundtrip(&ExtensionMessage::ResumeRecording);
    }

    #[test]
    fn test_cancel_recording() {
        roundtrip(&ExtensionMessage::CancelRecording);
    }

    #[test]
    fn test_video_ready() {
        roundtrip(&ExtensionMessage::VideoReady {
            session_id: "abc-123".into(),
        });
    }

    #[test]
    fn test_recording_error() {
        roundtrip(&ExtensionMessage::RecordingError {
            code: "stream_acquisition_failed".into(),
            details: "User cancelled the picker".into(),
        });
    }

    #[test]
    fn test_keepalive_ping() {
        roundtrip(&ExtensionMessage::KeepalivePing);
    }

    #[test]
    fn test_keepalive_pong() {
        roundtrip(&ExtensionMessage::KeepalivePong);
    }

    #[test]
    fn test_countdown_complete() {
        roundtrip(&ExtensionMessage::CountdownComplete);
    }

    #[test]
    fn test_get_streaming_data() {
        roundtrip(&ExtensionMessage::GetStreamingData);
    }

    #[test]
    fn test_apply_streaming_data() {
        roundtrip(&ExtensionMessage::ApplyStreamingData {
            data: "some-stream-data".into(),
        });
    }

    #[test]
    fn test_preview_closed() {
        roundtrip(&ExtensionMessage::PreviewClosed);
    }

    #[test]
    fn test_recovery_found() {
        roundtrip(&ExtensionMessage::RecoveryFound {
            session_id: "rec_abc_123".into(),
            chunk_count: 42,
        });
    }

    #[test]
    fn test_restore_recording() {
        roundtrip(&ExtensionMessage::RestoreRecording {
            session_id: "rec_abc_123".into(),
        });
    }

    #[test]
    fn test_dismiss_recovery() {
        roundtrip(&ExtensionMessage::DismissRecovery);
    }

    #[test]
    fn test_is_keepalive_new_variants_false() {
        assert!(!ExtensionMessage::RecoveryFound {
            session_id: "s".into(),
            chunk_count: 1,
        }
        .is_keepalive());
        assert!(!ExtensionMessage::RestoreRecording {
            session_id: "s".into(),
        }
        .is_keepalive());
        assert!(!ExtensionMessage::DismissRecovery.is_keepalive());
    }

    #[test]
    fn test_recording_mode_serde() {
        let modes = vec![RecordingMode::FullScreen, RecordingMode::Tab];
        for mode in modes {
            let json = serde_json::to_string(&mode).expect("serialize");
            let back: RecordingMode = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn test_is_keepalive() {
        assert!(ExtensionMessage::KeepalivePing.is_keepalive());
        assert!(ExtensionMessage::KeepalivePong.is_keepalive());
        assert!(!ExtensionMessage::StopRecording.is_keepalive());
        assert!(!ExtensionMessage::CountdownComplete.is_keepalive());
        assert!(!ExtensionMessage::StartRecording { mode: RecordingMode::FullScreen }.is_keepalive());
    }
}
