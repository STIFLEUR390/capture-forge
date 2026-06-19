---
stepsCompleted: [1]
inputDocuments: []
workflowType: 'research-extension'
lastStep: 1
research_type: 'technical'
research_topic: 'Technical Feasibility Analysis: WebCodecs, OPFS, Chrome APIs, WASM for Capture Forge'
research_goals: >
  Validate the technical feasibility of the "source session + lenses + optional
  local AI" architecture for Capture Forge — a modular, cross-browser, privacy-first
  screen recording extension built in Rust + WASM via oxichrome.
user_name: 'Herold'
date: '2026-06-19'
web_research_enabled: true
source_verification: true
---

# Technical Feasibility Analysis: Browser APIs for Capture Forge

**Date:** 2026-06-19
**Author:** Herold
**Type:** Technical analysis

---

## Research Overview

Deep-dive technical analysis to validate whether Capture Forge's architecture ("source session + lenses + optional local AI") is feasible in-browser using current Web APIs, Chrome extension APIs, and WebAssembly. Covers six capability domains:

1. Screen capture APIs (Chrome-specific + cross-browser)
2. MediaRecorder and WebCodecs for encoding
3. OPFS (Origin Private File System) for local storage
4. WebAssembly in Manifest V3 extensions
5. WebGPU for accelerated processing
6. Local AI inference (Whisper WASM, ONNX, etc.)

---

## 1. Screen Capture APIs — Current State

### 1.1 Three Capture Paths

| API | Scope | User Gesture Required | Service Worker Access | Cross-Browser |
|-----|-------|----------------------|----------------------|---------------|
| **`chrome.tabCapture`** | Browser tab (audio + video) | Yes (via extension action) | No (offscreen doc only) | Chromium only |
| **`chrome.desktopCapture`** | Tab/window/screen picker | Yes (tab ID required in MV3) | No (offscreen doc only) | Chromium only |
| **`getDisplayMedia()`** | Tab/window/screen picker | Yes | No (offscreen doc only) | Cross-browser (Chrome, Firefox, Safari 18.4+, Edge) |

### 1.2 Architecture Constraint: Service Worker Cannot Handle Media

**Critical finding:** In Manifest V3, the background service worker has **no DOM access** and cannot:
- Call `getUserMedia()` / `getDisplayMedia()` directly
- Hold active `MediaStream` objects
- Run `MediaRecorder` or `AudioContext`
- Use Web Audio API for mixing

**Solution: Offscreen Documents** (Chrome 109+, MV3 only) — a hidden, DOM-enabled page. The recommended architecture is:

```
Popup (user gesture)
    → Background Service Worker (orchestration, state, file saving)
        → Offscreen Document (capture, media mixing, MediaRecorder)
            → Background Service Worker (OPFS save or download)
```

**Limitations of Offscreen Documents:**
- Only `chrome.runtime` API available (no `tabCapture`, `storage`, `downloads` directly)
- Single instance per extension (no parallel captures)
- URL must be a static HTML file bundled with the extension
- No `offscreen` API in Firefox — Firefox uses `windows.create()` with hidden windows or background pages (MV2-compatible approach)

### 1.3 Audio Capture Architecture

Screen recording requires **three separate audio sources** that must be mixed:

| Source | Capture Method | Notes |
|--------|---------------|-------|
| Tab audio (system + remote participants) | `tabCapture` → `getUserMedia` with `chromeMediaSource: 'tab'` | Captured via stream ID token; no picker after initial gesture |
| Microphone (local user) | `getUserMedia({ audio: true })` | Separate call from offscreen document; user gesture required |
| Mixing | Web Audio API (`AudioContext` → `createMediaStreamDestination()`) | MediaRecorder can only encode one audio track per stream |

**Warning:** `tabCapture` redirects tab audio away from speakers to the capture stream. To hear audio during recording, route to `AudioContext.destination`.

### 1.4 Firefox Cross-Browser Path

Firefox does not support `chrome.offscreen`, `chrome.tabCapture`, or `chrome.desktopCapture`. The cross-browser alternative is:

- **`getDisplayMedia()`** + **`MediaRecorder`** from a **popup** or **background page** (if Firefox retains persistent background pages in MV3)
- Firefox's Manifest V3 still allows `background.scripts` (non-persistent) and `background.page` (persistent-eventing), unlike Chrome's strict service worker model
- **Firefox limitation:** No `tabCapture` equivalent — tab audio capture without a picker dialog is not possible
- **Current status:** Firefox's MV3 implementation maintains more flexibility than Chrome's (supports Event Pages, `background.page`, more CSP flexibility), but the `offscreen` API gap needs to be worked around

### 1.5 Feasibility Verdict: CAPTURE

| Requirement | Status |
|-------------|--------|
| Screen + tab recording in Chrome | ✅ Feasible via offscreen document pattern |
| Tab audio capture | ✅ Feasible via `tabCapture` stream ID |
| Mic + system audio mixing | ✅ Feasible via Web Audio API |
| Firefox support | ⚠️ Feasible via `getDisplayMedia()`; no tab audio capture |
| Service worker persistence | ⚠️ 300-second kill limit is a hard constraint for long recordings |
| Multi-tab recording | ❌ Each session is bound to one tab; closing tab ends capture |

> **Key architectural decision:** The offscreen document approach works but means audio mixing and MediaRecorder run in a hidden DOM context separate from the WASM module. WASM processing (AI, lenses) would need to operate on encoded chunks or in a separate worker, not on the live MediaStream directly.

---

## 2. MediaRecorder + WebCodecs — Encoding Strategy

### 2.1 MediaRecorder (P0 Path)

The simplest encoding path uses `MediaRecorder` in the offscreen document:

```javascript
// MIME type support varies; always check
const mimeType = 'video/webm; codecs=vp8,opus';
if (!MediaRecorder.isTypeSupported(mimeType)) {
    // Fallback to browser-default
    recorder = new MediaRecorder(stream);
}
recorder.ondataavailable = e => chunks.push(e.data);
```

**Limitations:**
- Only outputs containerized formats (WebM) — no raw frame access
- No control over encoding parameters beyond the MIME type string
- VP8/VP9/Opus are the only universally-supported codec combination
- No H.264 output in service worker context (H.264 requires proprietary licensing)
- `ondataavailable` delivers chunks at an interval you control via `timeslice` (ms), allowing incremental save to OPFS

### 2.2 WebCodecs (P1 Path for Editor)

WebCodecs provides low-level access to the browser's built-in codecs for **raw frame encoding/decoding**:

| Operation | WebCodecs Class | Support Level |
|-----------|----------------|---------------|
| Encode raw frames to H.264 | `VideoEncoder` | ✅ 99.72% encode support (H.264 Baseline) |
| Encode raw frames to VP9 | `VideoEncoder` | ✅ 99.99% encode support |
| Encode raw frames to AV1 | `VideoEncoder` | ⚠️ ~88% encode; 24% on Safari |
| Decode video to raw frames | `VideoDecoder` | ✅ Universal for VP9/H.264 |
| Encode audio | `AudioEncoder` | ✅ Opus 96%, AAC 90% |
| Process raw frames (filters, lenses) | `VideoFrame` + Canvas2D/WebGL | ✅ Full access to pixel data |

**2026 Codec support (1M+ device study):**

| Encoding Strategy | Coverage |
|---|---|
| VP9 Profile 0 | **99.99%** |
| H.264 Baseline | **99.72%** |
| AV1 Profile 0, 8-bit | ~88% |
| AV1 + HEVC | 98.16% |
| AV1 + VP9 | **99.91%** |
| AV1 + AVC | **99.94%** |

**Key insight for "lenses" architecture:**
WebCodecs + `VideoFrame` enables reading raw pixel data from a recorded stream, applying transformations (blur, zoom, highlights, overlays) as WASM compute shaders or Canvas2D operations, then re-encoding. This is the technical foundation for the "lens" concept: each lens is a `VideoFrame` → pixel manipulation → re-encode pipeline.

### 2.3 Recommended Encoding Strategy for Capture Forge

| Phase | Primary Output | Fallback | Rationale |
|-------|---------------|----------|-----------|
| P0 (recording) | WebM VP8/Opus via MediaRecorder | Browser default mimeType | Simplest path; universal compatibility |
| P1 (editor export) | WebM VP9 via WebCodecs | H.264 Baseline (Chrome only) | VP9 is universal; AV1 for users who need it |
| P2 (lenses) | Raw `VideoFrame` processing | Canvas2D intermediate | Lenses operate on decoded frames, re-encode via WebCodecs |

**Key recommendation for modularity:** Record to WebM chunks via MediaRecorder (P0), store chunks in OPFS. For playback/editing (P1), decode via WebCodecs into `VideoFrame` objects. For lenses (P2), pipe frames through WASM pixel processors.

---

## 3. OPFS — Origin Private File System (Storage Layer)

### 3.1 Why OPFS for Capture Forge

OPFS is the correct storage backend for a local-first, private screen recorder:

| Capability | IndexedDB | OPFS | Verdict |
|-----------|-----------|------|---------|
| Large file writes (>100MB) | ❌ Structured clone tax | ✅ Direct buffer-to-file | OPFS wins |
| Random access (seek within file) | ❌ Read → modify → write whole blob | ✅ Byte-level reads/writes via `SyncAccessHandle` | OPFS wins |
| Append-only logging | ❌ Transaction overhead per write | ✅ Hold open handle and append | OPFS wins |
| Streaming writes (chunks from MediaRecorder) | ❌ Batch commit overhead | ✅ Direct file append | OPFS wins |
| Crash durability | ⚠️ Transaction journal | ✅ Explicit `flush()` maps to `fsync` | OPFS wins |
| Web Worker access | ✅ | ✅ Sync access only in Workers | Tie (OPFS requires Worker for sync API) |

### 3.2 Architecture: Streaming MediaRecorder Chunks → OPFS

Each `ondataavailable` event delivers a chunk as a `Blob`. For a 1-hour recording at 1080p VP8 (~2-4 Mbps), that's approximately 900MB–1.8GB. OPFS handles this well:

```javascript
// Offscreen document: receive chunk and pass to background worker
// Background worker (dedicated): write chunks to OPFS
async function appendChunk(fileHandle, chunk) {
    const syncHandle = await fileHandle.createSyncAccessHandle();
    const buffer = await chunk.arrayBuffer();
    const fileSize = syncHandle.getSize();
    syncHandle.write(new Uint8Array(buffer), { at: fileSize });
    syncHandle.flush(); // crash-safe
    syncHandle.close();
}
```

**Performance:** Writing a 50MB payload to OPFS from a Worker is "a tight, synchronous loop" vs. IndexedDB which would "likely freeze for a few hundred milliseconds."

### 3.3 Crash Recovery Pattern (P0 Requirement)

The PRD calls for crash recovery. OPFS enables this naturally:

1. Create a recording session file at start → write metadata header (session ID, timestamp, recording params)
2. Append each MediaRecorder chunk as it arrives → `flush()` after each chunk
3. On crash → on next extension start, scan OPFS for incomplete sessions
4. Recover by concatenating all chunks → produce playable WebM (may lose last chunk before crash)

### 3.4 Storage Quotas

OPFS data is subject to the browser's storage quota. In Chrome:
- **~60% of available disk** for extensions and web apps combined
- Extensions and their origin share the quota per-profile
- OPFS data can be evicted by the browser if the device runs low on space
- `navigator.storage.estimate()` provides remaining quota

**Mitigation:** Warn users at 80% quota. Support explicit export/cleanup. The P0 "export WebM" feature frees OPFS space.

### 3.5 Feasibility Verdict: STORAGE

| Requirement | Status |
|-------------|--------|
| Write large recording files (GB+) | ✅ OPFS handles this well from Workers |
| Read chunks for playback | ✅ Random access via `SyncAccessHandle` |
| Crash recovery | ✅ Flush-after-chunk pattern works |
| Quota management | ⚠️ Browser may evict; need user warnings |
| Firefox support | ✅ OPFS supported in Firefox 110+ |

---

## 4. WebAssembly in Manifest V3 — The Critical Constraint

### 4.1 Current Status

Chrome's Manifest V3 supports WebAssembly with specific CSP requirements, but **several pitfalls** exist:

**The `wasm-unsafe-eval` requirement:**
```json
{
    "content_security_policy": {
        "extension_pages": "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"
    }
}
```

This directive permits `WebAssembly.instantiate()` and `WebAssembly.compile()` but **still blocks**:
- `eval()` — strictly blocked
- `new Function()` — strictly blocked
- **Dynamic JS stubs required for WASM interop** — the JIT bridge between WASM and JS can be blocked

**The Emscripten trap:** The default Emscripten build uses `-s DYNAMIC_EXECUTION=1` which generates `eval()` and `Function()` calls for Embind wrapper generation. Must set `-s DYNAMIC_EXECUTION=0` to work in MV3.

### 4.2 oxichrome Path

Capture Forge uses **wasm-bindgen** (via oxichrome), not Emscripten. wasm-bindgen does not rely on `eval()` or `new Function()` — it generates static JS bindings. This means:

✅ **wasm-bindgen is inherently MV3-compatible** — no `DYNAMIC_EXECUTION` issues
✅ **wasm-pack build --target web** generates clean JS that works with `'wasm-unsafe-eval'`
✅ **Oxichrome handles the manifest CSP** via its proc-macro attributes

### 4.3 Service Worker Lifecycle in MV3

**Critical lifecycle constraints (corrected):**

| Constraint | Detail | Mechanism |
|-----------|--------|-----------|
| Idle timeout | **~30 seconds** after last event | Worker terminates if no event (alarm, message, action) fires within this window |
| Single operation limit | **~5 minutes** for a long-running task | Chrome may terminate a request that exceeds ~5 min of continuous execution |
| Event-based reset | **Any event resets the idle timer** | `chrome.alarms.create()` with short period, `chrome.runtime.onMessage`, etc. |
| Offscreen document independence | Offscreen doc can **outlive** the service worker | Worker restarts can reconnect to existing offscreen doc |

**This is NOT a fixed "kill at 300s"** — the worker is terminated after ~30s of inactivity, not an absolute deadline. The 5-minute limit applies to a single continuous operation (e.g., one long WASM call without yielding). The correct pattern is:

1. **Keep the worker alive with events** — Use `chrome.alarms.create({ periodInMinutes: 4 })` as a heartbeat during active recording
2. **Design for reconnection** — The offscreen document outlives SW restarts; new SW instance reconnects via `chrome.runtime.connect`
3. **Chunk long operations into <5 min segments** — Save intermediate state to OPFS, resume in next chunk
4. **Use offscreen document for stable state** — The offscreen doc persists independently of the SW lifecycle

**Mitigation strategies:**
1. **Heartbeat alarm** — `chrome.alarms.create('capture-heartbeat', { periodInMinutes: 4 })` — the alarm event fires, resets the 30s idle timer, and the SW stays alive during active capture
2. **Offscreen doc reconnection** — On SW restart (e.g., after update), `clients.matchAll()` finds the existing offscreen doc and re-establishes the message channel
3. **OPFS as source of truth** — Recording state stored in OPFS, not in SW memory. SW crash = read last known state from OPFS on restart
  4. WASM linear memory state is not easily serialized — partial work is lost

**Mitigation strategies:**
1. **Break work into <300s chunks** — save intermediate state to OPFS/IndexedDB every N minutes
2. **Keep service worker alive with periodic chrome alarms** — `chrome.alarms.create()` with a short period (<5 min) can keep the worker alive during active recording
3. **Offscreen document can outlive the service worker** — design for reconnection: offscreen doc stores state, new service worker instance reconnects and picks up

### 4.4 WASM in Content Scripts

Content scripts have a **dual-CSP enforcement** problem — "Shadow CSP":
1. The manifest-defined CSP is checked first
2. An immutable baseline policy hardcoded in the browser core is then applied

This means WASM in content scripts is **not recommended** — the Shadow CSP may override the manifest CSP. WASM should run in:
- The service worker (with `wasm-unsafe-eval`)
- An offscreen document
- An extension page (popup, options)

### 4.5 Firefox WASM in Extensions

Firefox's MV3 is more permissive with CSP:
- `wasm-unsafe-eval` is supported
- `background.page` (persistent background page) is still allowed, avoiding the 300-second kill issue
- Firefox supports WASM in extensions since Firefox 95+

### 4.6 Feasibility Verdict: WASM

| Requirement | Status |
|-------------|--------|
| Rust WASM in background service worker | ✅ Feasible via `'wasm-unsafe-eval'` |
| wasm-bindgen compatibility | ✅ No `eval()` dependency |
| Long-running WASM processing | ⚠️ Must chunk work or keep SW alive with alarms |
| WASM in offscreen documents | ✅ Most flexible environment |
| WASM in content scripts | ❌ Shadow CSP blocks this |
| Firefox WASM extension support | ⚠️ Depends on MV3 implementation status |

---

## 5. WebGPU for Accelerated Video Processing

### 5.1 Current State (June 2026)

WebGPU is now **supported across all major browsers** (Chrome, Edge, Firefox, Safari) as of late 2025.

| Capability | Chrome | Firefox | Safari |
|-----------|--------|---------|--------|
| 3D graphics | ✅ | ✅ | ✅ |
| Compute shaders | ✅ | ✅ | ⚠️ No (as of June 2025) |
| Video frame as texture input | ✅ | ✅ | ✅ |
| WGSL shaders | ✅ | ✅ | ✅ |

### 5.2 Relevance for Capture Forge

WebGPU compute shaders can accelerate:

| Operation | Speedup vs CPU | Use Case |
|-----------|---------------|----------|
| Pixel-level video filters | 10-100x | Blur, color grading, background replacement |
| Image scaling/resizing | 10-50x | Thumbnails, resolution changes |
| ML inference (via WebNN backend) | 5-100x | On-device AI (if WebNN available) |
| Video compositing | 5-20x | Lens overlays, PIP, mockups |

**Key quote from research:** "WebGPU delivers 10–100x speedups for data processing, ML inference, and simulations."

**The Rust path:** `wgpu` (Rust WebGPU implementation) compiles to WASM and provides full WebGPU access from Rust. This means lens processing can be written in Rust and compiled to WASM, using WebGPU compute shaders for acceleration when available — falling back to CPU-based Canvas2D when WebGPU compute is not available (Safari).

### 5.3 Feasibility Verdict: WEBGPU

| Requirement | Status |
|-------------|--------|
| GPU-accelerated video filters | ✅ Via WebGPU compute shaders + wgpu |
| Universal fallback | ✅ Canvas2D for unsupported browsers |
| WASM + WebGPU from Rust | ✅ Via wgpu crate compiled to WASM |
| Safari compute shaders | ⚠️ Not yet available — fallback to CPU/Canvas2D |

---

## 6. Local AI Inference via WASM

### 6.1 Whisper WASM for Captioning

`whisper.cpp` has been compiled to WASM and runs in the browser:

| Model Size | RAM Usage | Quality | Speed (on M1) |
|-----------|-----------|---------|---------------|
| Tiny (~39MB) | ~300MB | Good (short clips) | >1x real-time |
| Base (~74MB) | ~500MB | Better | ~1x real-time |
| Small (~244MB) | ~1.5GB | Best | 0.3x real-time |

**Current packages:**
- `@timur00kh/whisper.wasm` — TypeScript wrapper for whisper.cpp → WASM
- `whisper.rn` — React Native port
- AssemblyAI's offline Whisper guide — browser-based WASM inference

**For Capture Forge:** Whisper tiny or base model compiled to WASM, pre-loaded as an optional "lens" module. The model binary (~39–74MB) would be bundled with the extension or fetched on first use with user consent.

### 6.2 ONNX Runtime Web

ONNX Runtime Web enables running ONNX models in-browser via WASM or WebGPU:
- Execution providers: WASM (CPU), WebGPU, WebGL
- Supports a wide range of ML models beyond Whisper
- Can be used for: background removal, scene detection, content moderation

### 6.3 Chrome Built-in AI (Prompt API)

Chrome has an experimental built-in AI API (Prompt API, behind a flag):
- Runs a small Gemini model locally in-browser
- Available for: summarization, rewriting, classification
- **Currently behind a flag** — not available to extensions in production
- No ETA for general availability

### 6.4 Local AI Architecture for Capture Forge

```
Recording completes (or during pause)
    → Audio track extracted
    → Passed to Whisper WASM module (tiny or base model)
    → Transcription generated locally in Worker (WASM + CPU)
    → Captions styled and stored as sidecar metadata
    → No data leaves the device
```

**Phased approach:**
- **P0:** No AI — all processing local by default
- **P1:** Basic captions via Whisper Tiny WASM (optional download)
- **P2:** Summarization, chapter detection, noise removal
- **Edge condition:** Large models (>74MB) increase extension size significantly — offer as optional download

### 6.5 Feasibility Verdict: LOCAL AI

| Requirement | Status |
|-------------|--------|
| Whisper captions offline | ✅ whisper.cpp WASM works in browser |
| Model size acceptable | ⚠️ Tiny (39MB) is fine; Small (244MB) needs opt-in download |
| Real-time transcription | ✅ Tiny model achieves >1x real-time on modern hardware |
| Browser AI API | ⚠️ Chrome Prompt API is experimental/flagged |
| No external API calls | ✅ All processing stays on-device |

---

## 7. Architecture Synthesis

### 7.1 Proposed Technical Architecture for Capture Forge

```
┌─────────────────────────────────────────────────────┐
│               Captured Forge Extension               │
├──────────────────────────────────────────────────────┤
│ Service Worker (Rust WASM → oxichrome)              │
│   • Extension lifecycle management                   │
│   • State orchestration                              │
│   • Module/lens registry                             │
│   • OPFS file management                             │
│   • Download/export                                  │
│   • Chrome alarms for keepalive                      │
├──────────────────────────────────────────────────────┤
│ Offscreen Document (created on demand)              │
│   • MediaRecorder host (WebM VP8/Opus)              │
│   • Audio mixing (Web Audio API)                     │
│   • Lens processing (Canvas2D or WebGPU)             │
│   • Chunk streaming to Service Worker → OPFS         │
├──────────────────────────────────────────────────────┤
│ Worker Threads (Dedicated Workers)                  │
│   • OPFS sync file access (SyncAccessHandle)        │
│   • WASM AI inference (Whisper, ONNX)               │
│   • Heavy lens processing (off main thread)         │
├──────────────────────────────────────────────────────┤
│ Storage (OPFS)                                       │
│   • WebM chunks (incremental append)                │
│   • Session metadata + crash recovery                │
│   • Lens configs and user preferences                │
│   • Downloaded AI models (Whisper .bin files)       │
├──────────────────────────────────────────────────────┤
│ Popup (Leptos UI via oxichrome)                     │
│   • Record/stop controls                            │
│   • Lens selector (source → selected lens)          │
│   • Session management                               │
│   • Settings and preferences                         │
├──────────────────────────────────────────────────────┤
│ Content Scripts (minimal)                           │
│   • Only if needed for DOM interactions             │
│   • No WASM (Shadow CSP blocks it)                  │
│   • Communicates via chrome.runtime                 │
└──────────────────────────────────────────────────────┘
```

### 7.2 Data Flow

```
User clicks record in Popup
    → Popup sends start message to SW
    → SW creates Offscreen Document
        → Offscreen calls tabCapture.getMediaStreamId() or getDisplayMedia()
        → Offscreen mixes tab audio + mic via Web Audio API
        → Offscreen starts MediaRecorder with timeslice (e.g., 1000ms)
    → Ondataavailable fires every 1s
        → Chunk sent to SW via runtime messaging
        → SW forwards to Dedicated Worker
            → Worker writes chunk to OPFS via SyncAccessHandle
            → Worker calls flush() → crash-safe
    → User clicks stop
        → SW closes Offscreen Document
        → Worker finalizes OPFS file
        → Session metadata written
    → User can now:
        a. Export WebM (chrome.downloads)
        b. Apply lenses (re-decode via WebCodecs → process → re-encode)
        c. Generate AI captions (optional: download Whisper model → transcribe)
```

### 7.3 Modular "Lens" Architecture

Each lens is a WASM module implementing:

```rust
pub trait Lens {
    fn name(&self) -> &str;
    fn process_frame(&self, frame: &mut VideoFrame) -> Result<(), LensError>;
    fn required_permissions(&self) -> Vec<Permission>;
    fn approximate_cost(&self) -> ComputeCost; // to show user before enabling
}
```

| Lens | WASM Size | Compute | P0/P1/P2 |
|------|-----------|---------|----------|
| Auto-zoom on clicks | ~50KB | CPU/Canvas2D | P0 (core UX) |
| Cursor spotlight | ~30KB | CPU/Canvas2D | P0 (core UX) |
| Blur sensitive content | ~100KB | WebGPU (preferred) or Canvas2D | P0 |
| Camera PIP | ~50KB | Canvas2D | P1 |
| Screen mockup frame | ~50KB | Canvas2D | P1 |
| Color grading | ~100KB | WebGPU compute shader | P1 |
| AI captions (Whisper) | ~39MB + model | CPU (WASM) | P2 |
| Background removal | ~500KB + ONNX | WebGPU compute | P2 |
| AI chapter detection | ~100KB | CPU (WASM) | P2 |

### 7.4 Known Constraints and Mitigations

| Constraint | Impact | Mitigation |
|-----------|--------|------------|
| SW 300-sec kill | Long recordings lose bridge | Chrome alarms every 4min to reset; offscreen doc reconnects on wake |
| Single offscreen doc | Cannot record multiple tabs simultaneously | P1 feature — use multiple offscreen docs via reason rotation? (undocumented) |
| No tab audio in Firefox | Firefox recordings lack system audio | `getDisplayMedia` + separate mic only; inform users |
| No `tabCapture` in Firefox | Must use `getDisplayMedia` picker every time | Acceptable trade-off for cross-browser support |
| WASM model size | Whisper models add 39–244MB | Bundle Tiny; request opt-in download for larger models |
| OPFS eviction | Browser may delete storage | Warning at 80% quota; export prompts; user-controlled cleanup |
| No WebGPU compute on Safari | Lenses run slower via Canvas2D | Feature detection; graceful degradation |

---

## 8. Feasibility Summary

| Capability Domain | P0 (Recorder) | P1 (Editor) | P2 (AI) |
|------------------|---------------|-------------|---------|
| Screen capture | ✅ Feasible | ✅ Feasible | ✅ Feasible |
| Audio mixing | ✅ Feasible | ✅ Feasible | ✅ Feasible |
| MediaRecorder encoding | ✅ Feasible | ⚠️ Upgrade to WebCodecs for flexibility | ✅ Feasible |
| OPFS storage | ✅ Feasible + crash recovery | ✅ Feasible | ✅ Feasible |
| WASM in extension | ✅ Feasible (wasm-bindgen) | ✅ Feasible | ✅ Feasible |
| Rust + oxichrome | ✅ Feasible | ✅ Feasible | ✅ Feasible |
| Lens architecture | ✅ Zoom, blur, spotlight | ✅ Mockups, overlays, grading | ✅ AI modules |
| Local AI inference | N/A | N/A | ⚠️ Feasible with Whisper Tiny (opt-in download) |
| WebGPU acceleration | N/A | ✅ Filters via compute shaders | ⚠️ Safari gap |
| Firefox support | ⚠️ `getDisplayMedia` only; no tab audio | ⚠️ No `offscreen` API — needs fallback | ⚠️ Same constraints |
| CrOS/ChromeOS | ✅ Chrome-native APIs | ✅ Same codebase | ✅ Same |

### 8.1 Top Technical Risks (Ranked)

1. **🟡 Service worker idle timeout (~30s) + single-operation limit (~5 min)** — MUST architect for event-driven keepalive (heartbeat alarms every 4 min), chunk long operations, and design reconnection for SW restart. Offscreen doc persists independently.
2. **🔴 No `offscreen` API in Firefox** — Need a Firefox-specific fallback path using `windows.create()` or persistent `background.page` (if Firefox maintains it) for media processing.
3. **🟡 WASM model download size** — 39MB for Whisper Tiny is acceptable but adds friction. Implement as opt-in with progress indicator.
4. **🟡 OPFS eviction under storage pressure** — User education needed. Automatic export-to-download as safety net.
5. **🟢 Safari's lack of WebGPU compute** — Fallback to Canvas2D for lens processing. Acceptable for P1.
6. **🟢 Content script WASM blocked** — Not an issue if all WASM runs in SW/offscreen doc/workers (as designed).

### 8.2 Recommendation

**The architecture is technically feasible.** No blocker found that would prevent Capture Forge from working as designed. The main engineering investment is:

1. **Offscreen document pattern** for capture + mixing (Chrome)
2. **OPFS-backed chunked storage** with crash recovery
3. **Rust WASM modules** for each lens, loaded on demand
4. **Firefox fallback path** using `getDisplayMedia()` + hidden window/popup
5. **Optional Whisper WASM download** for local AI captions (P2)

The modular "source session + lenses" architecture maps cleanly to WebCodecs `VideoFrame` processing and WebAssembly modules, making it both technically sound and aligned with the product differentiation identified in the market research.
