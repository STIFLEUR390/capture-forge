# Architecture — CaptureForge

**Stack** : Rust + Oxichrome + Leptos + web-sys
**Cible** : Chrome 120+ (référence), Firefox 125+ (P1)
**Build** : `cargo oxichrome build`
**Source référence** : `screenity/` (clone local v4.5.3)

---

## 1. 3 sous-produits indépendants

```
┌──────────────────────────────────────────────────────────────────┐
│  Recorder Core (P0, V0.1)                                        │
│  ├── capture écran/onglet                                        │
│  ├── micro + pause/resume/stop/cancel                            │
│  ├── export WEBM (concaténation chunks)                         │
│  ├── stockage OPFS (1 track mixé)                               │
│  └── recovery basique (proposer restauration si chunks trouvés)  │
├──────────────────────────────────────────────────────────────────┤
│  Editor (P1, V0.5)                                               │
│  ├── lecteur vidéo + trim début/fin + mute + crop               │
│  ├── annotations canvas (pen, text, shapes, blur)               │
│  └── export après édition                                        │
├──────────────────────────────────────────────────────────────────┤
│  AI / Enrichment (P2, V2.0+, optionnel — feature-gated)         │
│  ├── sherpa-onnx (STT local, ~20MB)                             │
│  ├── aisdk (génération doc cloud)                               │
│  └── DOM capture (snapshots HTML)                               │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Stack technique

```
┌──────────────────────────────────────────────┐
│  Oxichrome v0.2                               │
│  ├── #[extension], #[background], #[popup]    │
│  └── #[on(runtime::on_message)]               │
├──────────────────────────────────────────────┤
│  Leptos v0.7 (CSR)                            │
│  ├── RwSignal, Effect, spawn_local            │
│  └── view!, For, Show                         │
├──────────────────────────────────────────────┤
│  web-sys v0.3                                 │
│  ├── MediaRecorder, MediaStream, AudioContext │
│  ├── CanvasRenderingContext2d                 │
│  ├── FileSystemFileHandle (OPFS)             │
│  └── Worker, Blob, Url                        │
├──────────────────────────────────────────────┤
│  opfs v0.2 + indexed_db_futures v0.6         │
├──────────────────────────────────────────────┤
│  JS shims (minimaux)                          │
│  ├── js/ffmpeg.js       (FFmpeg WASM)        │
│  ├── js/mediapipe.js    (MediaPipe)          │
│  └── js/chrome_shim.js  (tabCapture etc.)    │
├──────────────────────────────────────────────┤
│  Optionnel (feature-gated)                    │
│  ├── sherpa-onnx v1.13  (STT local)          │
│  └── aisdk v0.2          (LLM cloud)         │
└──────────────────────────────────────────────┘
```

---

## 3. Structure du workspace

```
capture-forge/
├── Cargo.toml                     # cdylib
├── oxichrome.config.toml
│
├── src/
│   ├── lib.rs                     # #[extension] entry
│   │
│   ├── background.rs              # #[background] service worker
│   │   ├── mod.rs                 # init, heartbeat simple
│   │   ├── listeners.rs           # commands, runtime, tabs events
│   │   └── messaging.rs           # message router (~20 handlers P0)
│   │
│   ├── recorder.rs                # RECORDER CORE (P0)
│   │   ├── mod.rs                 # RecordingSession state machine
│   │   ├── lifecycle.rs           # start/stop/pause/resume/cancel
│   │   ├── chunk.rs               # Chunk accumulation + OPFS write
│   │   └── stream.rs              # Stream acquisition (tab/desktop)
│   │
│   ├── storage.rs                 # STORAGE (P0)
│   │   ├── mod.rs                 # Storage trait
│   │   ├── opfs.rs                # OPFS writer (1 track)
│   │   └── indexdb.rs             # IndexedDB fallback
│   │
│   ├── export.rs                  # EXPORT (P0)
│   │   └── webm.rs                # WEBM concaténation
│   │
│   ├── content_script.rs          # OVERLAY + ANNOTATIONS (P1)
│   │   ├── mod.rs                 # Shadow DOM injection
│   │   ├── overlay.rs             # Toolbar flottante
│   │   ├── canvas.rs              # Annotation engine
│   │   │   ├── tools.rs           # Pen, highlighter, text, shapes
│   │   │   └── history.rs         # Undo/Redo
│   │   ├── camera.rs              # Camera PiP (P1)
│   │   └── countdown.rs           # 3-2-1 countdown
│   │
│   ├── editor.rs                  # EDITOR (P1)
│   │   ├── mod.rs                 # Editor state (non-destructive)
│   │   ├── player.rs              # Video player
│   │   ├── operations.rs          # Trim, mute, crop
│   │   └── export.rs              # Export after edit
│   │
│   ├── camera_page.rs             # Camera-only page (P1)
│   ├── region_page.rs             # Region selection (P1)
│   ├── popup.rs                   # Mode selection
│   ├── setup.rs                   # First-run
│   └── permissions.rs             # Permissions UI
│
├── src/ai/                        # AI (P2, optional)
│   ├── mod.rs                     # Feature-gated dispatch
│   ├── transcription.rs           # sherpa-onnx
│   ├── captions.rs                # SRT/VTT
│   ├── docgen.rs                  # aisdk
│   └── dom_capture.rs             # HTML snapshots
│
├── static/                        # Icons, fonts, locales
├── js/                            # JS shims
│   ├── ffmpeg.js
│   ├── mediapipe.js
│   └── chrome_shim.js
└── models/                        # ONNX models (sherpa-onnx)
    └── zipformer-en/              # Modèle par défaut unique
```

### Cargo.toml

```toml
[package]
name = "capture-forge"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
oxichrome = "0.2"
leptos = { version = "0.7", features = ["csr"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
web-sys = { version = "0.3", features = [
    "CanvasRenderingContext2d", "MediaRecorder", "MediaStream",
    "MediaDevices", "AudioContext", "OscillatorNode", "AnalyserNode",
    "File", "Blob", "Url", "Worker", "FileSystemFileHandle",
    "FileSystemWritableFileStream", "FileSystemDirectoryHandle",
    "HtmlCanvasElement", "HtmlVideoElement", "Window", "Document",
    "Navigator", "StorageManager", "console", "Performance",
    "Clipboard", "ClipboardItem",
] }
indexed_db_futures = "0.6"
opfs = "0.2"

# Optionnel — AI
sherpa-onnx = { version = "1.13", optional = true }
aisdk = { version = "0.2", optional = true }

[features]
default = ["recorder", "storage", "export"]
recorder = []
storage = []
export = []
overlay = []        # Content script + canvas (P1)
editor = []         # Video editor (P1)
camera = []         # PiP + MediaPipe (P1)
stt = ["sherpa-onnx"]  # Transcription locale (P2)
llm = ["aisdk"]        # LLM cloud (P2)
dom = []               # DOM capture (P2)

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

---

## 4. Data Flow (Recorder Core)

```
User: Popup → "Start Recording"
    │
    ▼
background.rs
    ├── 1. startRecording()
    ├── 2. openRecorderTab() ou createOffscreen()
    │
    ▼
recorder.rs
    ├── 3. acquireStream() — getDisplayMedia / tabCapture
    ├── 4. startRecorder() — MediaRecorder (codec par défaut: VP8+opus)
    ├── 5. Chunks → OPFS toutes les ~10s
    │
    User: "Stop"
    ▼
    ├── 6. stop() → finalize last chunk
    ├── 7. concat chunks → WEBM blob
    └── 8. open editor.html ou download.html
```

---

## 5. Messages (Recorder Core)

```rust
pub enum ExtensionMessage {
    StartRecording { mode: RecordingMode },
    StopRecording,
    PauseRecording,
    ResumeRecording,
    CancelRecording,
    GetStreamingData,
    ApplyStreamingData { data: String },
    VideoReady { session_id: String },
    RecordingError { code: String, details: String },
}
```

~10 handlers (contre 80+ dans l'original). Le message router reste extensible.

---

## 6. Build Pipeline

```
cargo oxichrome build --release
    │
    │ 1. Compile Rust → wasm32-unknown-unknown
    │ 2. wasm-bindgen → dist/wasm/
    │ 3. Génère manifest.json + shims JS
    │ 4. wasm-opt -Oz
    │
    ▼
dist/chromium/
├── manifest.json
├── background.js
├── popup.html / popup.js
├── recorder.html / recorder.js
├── editor.html / editor.js     (si feature "editor")
├── wasm/
│   ├── capture_forge.js
│   └── capture_forge_bg.wasm
├── icons/ fonts/ backgrounds/ locales/
└── js/
    ├── ffmpeg.js
    ├── mediapipe.js
    └── chrome_shim.js
```

---

## 7. Plan de QA

### Unitaires (Rust)
```bash
cargo test                          # Tous les modules
cargo test --features editor        # Avec l'éditeur
cargo test --features stt           # Avec la transcription
```

### Tests de la state machine
```
Idle → Starting → Recording → Paused → Recording → Stopping → Idle
Idle → Starting → Recording → Stopping → Exporting → Done
Idle → Starting → Recording → Error → Idle
```

### E2E (Playwright)
```bash
npx playwright test tests/e2e/
```

Scénarios :
1. Install → Setup → Record 10s → Stop → WEBM download
2. Record → Pause 3s → Resume → Record 5s → Stop → durée = 15s
3. Record → Kill SW → Restart → Recovery proposée
4. Record → Editor → Trim → Export
5. Record → Annotations → Stop → Export avec annotations
```

### Matrice de compatibilité

| Test | Chrome 120 | Chrome 130 | Edge 120 | Firefox 125 |
|------|-----------|-----------|----------|-------------|
| Screen record | ✅ | ✅ | ✅ | 🔄 P1 |
| Tab record | ✅ | ✅ | ✅ | 🔄 P1 |
| Pause/Resume | ✅ | ✅ | ✅ | 🔄 P1 |
| WEBM export | ✅ | ✅ | ✅ | ✅ |
| OPFS | ✅ | ✅ | ✅ | 🔄 P1 |

---

## 8. Performance targets

| Métrique | Cible | Condition |
|----------|-------|-----------|
| Recording FPS | ≥ 25 FPS en 1080p | Chrome 120+, GPU Intel/AMD/Nvidia. Fallback 720p accepté |
| Audio sync | Désync < 100ms | Casque recommandé |
| Memory recording | < 500MB pour 1h | Chunks OPFS |
| Export WEBM 5min | < 3s | Concat simple, pas ré-encodage |
| Export MP4 5min | < 2min | FFmpeg WASM (P1) |
| WASM load | < 1s | wasm-opt, brotli |
| Canvas annotation | < 16ms | web-sys Canvas 2D direct |

---

## 9. Défis et solutions

| Défi | Solution |
|------|----------|
| Rust-first mais FFmpeg WASM est du JS | JS shim encapsulé derrière un trait Rust |
| tabCapture pas dans web-sys | JS shim `chrome_shim.js` (20 lignes) |
| Taille WASM > 2MB | Feature gates, wasm-opt, brotli |
| MediaPipe = JS uniquement | JS shim, appelé depuis Rust |
| Oxichrome v0.2 immature | Fork si nécessaire, contrib upstream |
| sherpa-onnx modèle 20MB à dl | Download progressif dans OPFS, 1 seul modèle par défaut |
