# AudienceLens Architecture

**Status:** Draft (post-brainstorming session 2026-06-19)
**Applies to:** CaptureForge Recorder Core — downstream of session-structured capture
**Dependencies:** `docs/prd.md`, `docs/architect.md`, `Oxichrome.md`

---

## 1. Module Objective

Define the contract between a **captured session** and its **derived publications**. The `AudienceLens` type formalizes how a raw multi-track session is transformed, filtered, and rendered into audience-specific outputs — without ever mutating the source session.

### Role in the Source → Derivation Chain

```
Session Source (immutable)
    │  OPFS: tracks, manifest, chunks, health signals
    ▼
AudienceLens (declarative engine)
    │  visibility rules → transforms → outputs
    ▼
Publication (one per lens)
    │  Sales / Dev / QA / Docs + custom
```

The source never changes. Lenses never write back. Publications are cached derivations, recomputed on invalidation.

---

## 2. Core Type — `AudienceLens`

```rust
/// A declarative transformation engine applied to a captured session source.
/// Each lens produces one or more derived publications without mutating the
/// source tracks or manifest.
pub struct AudienceLens {
    pub id: LensId,
    pub name: String,
    pub kind: LensKind,

    /// Visibility rules: which source layers to include/expose
    pub visibility: VisibilityRules,

    /// Transform pipeline: ordered transformations applied to visible layers
    pub transforms: Vec<Box<dyn LensTransform>>,

    /// Output targets: formats and destinations for the derived publication
    pub outputs: Vec<OutputTarget>,

    /// Integrity contract declared at install time
    pub capabilities: CapabilitySet,
}
```

### Supporting Types

```rust
pub struct LensId(pub Uuid);

pub enum LensKind {
    /// Built-in, shipped with the extension
    BuiltIn,
    /// Installed from community marketplace, sandboxed
    Community { publisher: String, version: SemVer },
    /// User-composed from existing capabilities in the advanced UI
    Custom { author: String },
}

pub struct VisibilityRules {
    /// Which tracks are visible (video_primary, audio_mic, audio_system,
    /// camera_pip, dom_snapshots, overlays, cursor, etc.)
    pub include_tracks: TrackSet,

    /// Selectors for DOM context depth (none / selectors_only / full_tree)
    pub dom_depth: DomDepth,

    /// Whether to mask sensitive data (credentials, tokens, PII)
    pub auto_mask: MaskPolicy,

    /// Whether to expose raw timing / debug info
    pub show_technical_overlay: bool,
}

pub enum DomDepth {
    None,
    SelectorsOnly,
    FullTree,
}

pub enum MaskPolicy {
    /// No masking
    None,
    /// Auto-detect and blur (default for QA/Sales)
    Auto,
    /// Strict mode — mask everything pattern-matching a deny list
    Strict { patterns: Vec<GlobPattern> },
}
```

### Transform Trait

```rust
/// A single transformation step in the lens pipeline.
/// Transforms are *declarative*: they describe what to do, not how.
/// The runtime maps them to concrete implementations (DOM filtering,
/// narrative zoom, transcript summarization, etc.).
#[async_trait]
pub trait LensTransform: Send + Sync {
    fn id(&self) -> &str;
    fn input_kind(&self) -> TrackKind;
    fn output_kind(&self) -> TrackKind;

    /// Apply the transformation to a single track segment.
    /// Returns None if the transformation is not applicable
    /// (graceful degradation).
    async fn apply(
        &self,
        input: TrackSegment,
        context: &LensContext,
    ) -> Result<Option<TrackSegment>, TransformError>;
}

pub enum TransformError {
    /// Transient — retry may succeed
    Transient { retryable: bool, detail: String },
    /// Permanent — this transform cannot produce output for this input
    Permanent { detail: String },
    /// Resource limit exceeded (memory, time)
    ResourceExhausted { resource: ResourceKind, limit: u64 },
}
```

### Output Target

```rust
pub struct OutputTarget {
    pub format: OutputFormat,
    pub quality: QualityProfile,
    pub destination: OutputDestination,
}

pub enum OutputFormat {
    Video { container: VideoContainer, codec: Codec },
    Markdown { template: String },
    InteractiveReplay,
    QAReport,
    ChangelogVideo,
    // Extensible — plugins register new variants
    Custom(String),
}

pub enum OutputDestination {
    /// Export to user's Downloads folder (triggered via chrome.downloads)
    Download { filename: String },
    /// Save as a derived track within the same session bundle
    SessionDerived { track_label: String },
    /// Copy to clipboard / share URL
    Clipboard,
}
```

---

## 3. Capabilities & Security

### Declared Capabilities — `CapabilitySet`

Every lens (especially community ones) must declare what it needs:

```rust
pub struct CapabilitySet {
    /// Which track types the lens reads
    pub reads_tracks: Vec<TrackKind>,

    /// Whether it can access raw DOM snapshots
    pub reads_dom: bool,

    /// Whether it needs network access
    pub network_access: NetworkAccess,

    /// Whether it can read the full transcript
    pub reads_transcript: bool,

    /// Maximum memory it may allocate (0 = no limit)
    pub max_memory_bytes: u64,

    /// Maximum wall-clock time per transform call
    pub max_transform_ms: u64,
}

pub enum NetworkAccess {
    None,
    /// Only to pre-approved CDN for model weights
    CdnOnly { allowlist: Vec<String> },
    /// Full — subject to user confirmation at install
    Full,
}
```

### Enforcement Layers

| Phase | Mechanism | What it catches |
|-------|-----------|-----------------|
| **Install** | Static validation of `CapabilitySet` vs declared `VisibilityRules` | Lying about what it reads |
| **Load** | Sandbox instantiation (dedicated `WebAssembly` module or `iframe` with `sandbox` attribute) | Lateral data access |
| **Transform** | Runtime capability proxy — lens receives `TrackSegment` *views*, never raw `Arc<Vec<u8>>` | Buffer over-reads |
| **Output** | Output size cap, content-type validation | ZIP bombs, format smuggling |

### Sandboxing Strategy

For community lenses, each transform runs in an isolated context:

- **Rust-native lenses:** Separate `wasm` module with no `import` access to Chrome APIs. All data passed as serialized segments (zero-copy views via `SharedArrayBuffer` where supported).
- **JS-plugin lenses:** Sandboxed `<iframe>` with `sandbox="allow-scripts"` attribute. No `allow-same-origin`. No DOM access to the extension's own pages.
- **Custom user lenses:** Run in the same process as the editor UI (trusted by definition) but still subject to capability validation at save time.

---

## 4. Built-In Lenses

### Sales Lens

| Property | Value |
|----------|-------|
| `visibility.include_tracks` | video_primary, audio_mic, camera_pip, overlays |
| `visibility.dom_depth` | None |
| `visibility.auto_mask` | Auto |
| `visibility.show_technical_overlay` | false |
| `transforms` | Narrative zoom, transition smoothing, CTA emphasis |
| `outputs` | Video (MP4/WebM), Markdown summary |

### Dev Lens

| Property | Value |
|----------|-------|
| `visibility.include_tracks` | video_primary, audio_mic, audio_system, dom_snapshots, overlays |
| `visibility.dom_depth` | SelectorsOnly |
| `visibility.auto_mask` | None |
| `visibility.show_technical_overlay` | true |
| `transforms` | DOM selector injection, timing track, log overlay |
| `outputs` | Video (with burnt-in tech overlay), Markdown (with code snippets) |

### QA Lens

| Property | Value |
|----------|-------|
| `visibility.include_tracks` | video_primary, audio_mic, dom_snapshots, cursor |
| `visibility.dom_depth` | FullTree |
| `visibility.auto_mask` | Strict (credentials, tokens) |
| `visibility.show_technical_overlay` | true |
| `transforms` | Step detection, checkpoint markers, assertion highlighting |
| `outputs` | QAReport (custom), Markdown (trace), Video (with burnt-in checkpoints) |

### Docs Lens

| Property | Value |
|----------|-------|
| `visibility.include_tracks` | video_primary, dom_snapshots, overlays |
| `visibility.dom_depth` | SelectorsOnly |
| `visibility.auto_mask` | Auto |
| `visibility.show_technical_overlay` | false |
| `transforms` | Chapter auto-detection, screenshot extraction, transcript → section |
| `outputs` | Markdown/MDX tutorial, screenshot gallery, step-by-step |

---

## 5. States & Transitions

### Lens Lifecycle

```
Draft ──► Validated ──► Active ──► Disabled
               │                      │
               ▼                      ▼
           Rejected               Uninstalled
```

| State | Meaning |
|-------|---------|
| `Draft` | Being composed in the advanced editor, not yet saved |
| `Validated` | Capability set checked against declared rules, sandbox configured |
| `Active` | Available in the Audience Lens selector on the Session Source page |
| `Disabled` | User turned it off; no data access |
| `Rejected` | Static validation failed at install/save — not loaded |
| `Uninstalled` | Removed from the lens registry |

### Derived Publication States

```
Pending ──► Rendering ──► Ready
                │
                ▼
            Failed ──► Partial
```

| State | Meaning |
|-------|---------|
| `Pending` | Publication requested, not yet started |
| `Rendering` | Transform pipeline is executing |
| `Ready` | Fully rendered, available for export/preview |
| `Failed` | Permanent error — publication not available |
| `Partial` | Some transforms succeeded, some degraded — report attached |

---

## 6. Error Contracts

### Lens Application Errors

| Condition | Error | UX |
|-----------|-------|-----|
| Lens `capabilities` mismatch with runtime environment | `CapabilityViolation` | "This lens requires DOM access, which is not available." |
| Transform timeouts | `TransformError::ResourceExhausted` | "Frame enhancement timed out — continuing without it." |
| Output target unavailable (e.g., download path) | `OutputUnavailable` | "Could not write to Downloads folder." |
| Source track missing for required transform | `TransformError::Permanent` | "DOM snapshot track not found — skipping DOM transforms." |

### Degradation Rules

1. **A failed transform does not fail the entire lens.** The renderer skips the failing step and marks the publication as `Partial`.
2. **A failed output target does not fail other targets.** If MP4 export fails but Markdown succeeds, the Markdown output is `Ready`.
3. **The user is never shown a raw error object.** Each failure is mapped to a human-readable degradation notice attached to the specific publication.

---

## 7. Validation Criteria

### For a Built-In Lens

- [ ] Declares `CapabilitySet` consistent with its `VisibilityRules`
- [ ] All `transforms` resolve to known implementations
- [ ] At least one `outputs` entry produces a format a user can consume immediately
- [ ] Degradation path documented for each transform

### For a Community Lens (Marketplace)

- [ ] `CapabilitySet` statically validated against declared manifest
- [ ] Sandbox instantiation verified in test environment
- [ ] No network access beyond declared `NetworkAccess`
- [ ] Output size capped at 2× the source session size
- [ ] Lens name + publisher signature verified

### For a Custom Lens (User-Composed)

- [ ] Every referenced capability is available in the current runtime
- [ ] Transform order forms a DAG (no cycles)
- [ ] At least one output target is `Download { .. }` or `SessionDerived { .. }`

---

## 8. Integration Points

| Component | Role |
|-----------|------|
| `recording.rs` | Produces raw tracks — the lens never reads here |
| `storage.rs` (OPFS) | Stores session manifest + track segments — lens reads *through* capability proxy |
| `editor.rs` | Triggers lens re-render when timeline is modified — source itself never changes |
| `dom_capture.rs` | Produces DOM snapshot track (optional) — gated by lens `reads_dom` |
| `ai.rs` | Feature-gated transforms (transcript → chapters, auto-masking) — invoked by lens pipeline |
| `MessageRouter` | Distributes lens render requests across modules — enables parallel rendering of independent outputs |

---

## 9. Open Questions (from brainstorming)

1. **Re-render invalidation:** When the user edits the timeline, do we invalidate all derived publications, or only those whose source time range changed?
2. **Shared lens state:** Can two lenses share a computed intermediate (e.g., transcript → chapters is the same for Docs and QA)? If so, how is caching governed by `CapabilitySet`?
3. **Plugin ABI stability:** What is the minimum stable API surface for community lens authors? `LensTransform` trait? Something higher-level?
4. **Version skew between lens and session format:** How does a lens declare which session manifest version it targets?
