---
name: CaptureForge
description: Privacy-first open-source screen recorder browser extension. Rust/Leptos/WASM — no inherited UI system; all tokens defined from scratch.
status: final
colors:
  # System-theme pairings. All tokens exist in light and dark variants.
  background-light: '#FFFFFF'
  background-dark: '#1A1B1E'
  foreground-light: '#1A1B1E'
  foreground-dark: '#E4E5E7'
  muted-light: '#F4F4F5'
  muted-dark: '#27282B'
  muted-foreground-light: '#71717A'
  muted-foreground-dark: '#A0A1A7'
  primary-light: '#2563EB'
  primary-dark: '#60A5FA'
  primary-foreground-light: '#FFFFFF'
  primary-foreground-dark: '#0F172A'
  accent-light: '#F59E0B'
  accent-dark: '#FCD34D'
  accent-foreground-light: '#1A1A00'
  accent-foreground-dark: '#1A1A00'
  destructive-light: '#EF4444'
  destructive-dark: '#F87171'
  border-light: '#E4E4E7'
  border-dark: '#3F3F46'
  ring-light: '#2563EB'
  ring-dark: '#60A5FA'
  surface-overlay: 'rgba(0, 0, 0, 0.5)'
  recording-dot-light: '#EF4444'
  recording-dot-dark: '#FCA5A5'
  recording-dot-glow-light: 'rgba(239, 68, 68, 0.6)'
  recording-dot-glow-dark: 'rgba(252, 165, 165, 0.6)'
  countdown-fill-light: '#2563EB'
  countdown-fill-dark: '#60A5FA'
  integrity-clean: '#22C55E'
  integrity-partial: '#F59E0B'
  integrity-incomplete: '#EF4444'
typography:
  # V0.1: system font stack. No custom fonts — WASM binary size constraints.
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Helvetica Neue', sans-serif"
    fontSize: 13px
    fontWeight: '400'
    lineHeight: '1.4'
  body-sm:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Helvetica Neue', sans-serif"
    fontSize: 11px
    fontWeight: '400'
    lineHeight: '1.3'
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Helvetica Neue', sans-serif"
    fontSize: 12px
    fontWeight: '500'
    lineHeight: '1.3'
    letterSpacing: '0.02em'
  timer:
    fontFamily: "'SF Mono', 'Cascadia Code', 'JetBrains Mono', 'Fira Code', Consolas, monospace"
    fontSize: 20px
    fontWeight: '600'
    lineHeight: '1.2'
    letterSpacing: '0.05em'
  countdown:
    fontFamily: "'SF Mono', 'Cascadia Code', 'JetBrains Mono', 'Fira Code', Consolas, monospace"
    fontSize: 72px
    fontWeight: '700'
    lineHeight: '1'
rounded:
  sm: 4px
  md: 6px
  lg: 10px
  full: 9999px
spacing:
  unit: 4px  # 4-based scale: 4, 8, 12, 16, 20, 24, 32
  popup-padding: 16px
  overlay-padding: 12px
components:
  popup-container:
    background: '{colors.background-light}' / '{colors.background-dark}'
    padding: '{spacing.popup-padding}'
    radius: '{rounded.lg}'
    width: 280px
  popup-button-primary:
    background: '{colors.primary-light}' / '{colors.primary-dark}'
    foreground: '{colors.primary-foreground-light}' / '{colors.primary-foreground-dark}'
    radius: '{rounded.md}'
    height: 36px
    font: '{typography.label}'
  recorder-toolbar:
    background: '{colors.background-light}' / '{colors.background-dark}'
    radius: '{rounded.lg}'
    shadow: '0 2px 8px rgba(0,0,0,0.15)'
    padding: '{spacing.overlay-padding}'
    height: 44px
  timer-display:
    foreground: '{colors.recording-dot-light}' / '{colors.recording-dot-dark}'
    font: '{typography.timer}'
  countdown-number:
    foreground: '{colors.countdown-fill-light}' / '{colors.countdown-fill-dark}'
    font: '{typography.countdown}'
  toast:
    background: '{colors.background-light}' / '{colors.background-dark}'
    foreground: '{colors.foreground-light}' / '{colors.foreground-dark}'
    radius: '{rounded.md}'
    border: '1px solid {colors.border-light}' / '1px solid {colors.border-dark}'
    shadow: '0 4px 12px rgba(0,0,0,0.2)'
  integrity-badge-clean:
    background: '{colors.integrity-clean}'
    foreground: '#FFFFFF'
    radius: '{rounded.full}'
  integrity-badge-partial:
    background: '{colors.integrity-partial}'
    foreground: '{colors.accent-foreground-light}'
    radius: '{rounded.full}'
  integrity-badge-incomplete:
    background: '{colors.integrity-incomplete}'
    foreground: '#FFFFFF'
    radius: '{rounded.full}'
---

## Brand & Style

CaptureForge is a privacy-first open-source screen recorder. The visual identity follows from the product's position: a **developer tool with consumer-grade polish**. It should feel serious without being corporate, refined without being fussy.

The brand is expressed through:
- **Blue primary** — trusted, technical, neutral across light/dark
- **Amber accent** — recording state, countdown moments, integrity warnings — anything "live" or "at risk"
- **Red recording dot** — the single non-negotiable recording affordance, inherited from camera convention
- **System font stack** — no custom font loading in V0.1 (WASM binary constraints)
- **Monochromatic grays** — the interface is almost all grays; color is reserved for meaning
- **No gradient, no illustration, no mascot** — visual restraint signals "tool, not toy"

[ASSUMPTION: Brand colors chosen as blue/amber. Replace with actual brand colours when decided.]

## Colors

The palette is defined as light/dark pairs. Every surface observes `prefers-color-scheme` and switches without transition (no animation on theme flip).

**Surface hierarchy:**
- `background` — primary surface (popup, preview page, editor)
- `muted` — secondary surface (toolbar background, disabled state)
- `surface-overlay` — scrim behind dialogs (semi-transparent black)

**Semantic color:**
- `primary` — interactive elements (start button, active toggles, links)
- `accent` — recording state, countdown, integrity warnings, any "this matters now" indicator
- `destructive` — cancel recording, delete recording, error states
- `integrity-*` — three-state badge: clean (green), partial (amber), incomplete (red)
- `recording-dot` — the live recording indicator (red, with glow)

**Dark mode adaptation:** The light-to-dark mapping is hue-preserving but luminance-adjusted. Blue primary light (`#2563EB`) → dark (`#60A5FA`). Amber accent light (`#F59E0B`) → dark (`#FCD34D`). The red recording dot darkens slightly to avoid blooming on dark surfaces.

## Typography

V0.1 uses the **system font stack** exclusively. No `@font-face`, no variable font, no webfont download. Rationale: WASM binary size is already a concern; adding font loading increases complexity and cold-start time.

| Role | Size | Weight | Usage |
|------|------|--------|-------|
| `body` | 13px | 400 | Popup labels, preview descriptions, settings |
| `body-sm` | 11px | 400 | Secondary info, file sizes, timestamps |
| `label` | 12px | 500 (0.02em tracking) | Button labels, toggle labels, section headers |
| `timer` | 20px | 600 (mono, 0.05em tracking) | Recording duration display |
| `countdown` | 72px | 700 (mono) | 3-2-1 countdown number |

Monospace faces (`SF Mono`, `Cascadia Code`, etc.) are used for the timer and countdown, where fixed-width alignment communicates precision.

[ASSUMPTION: System font stack is acceptable for V0.1. If a brand font is desired, add in V0.2 with a custom `@font-face` declaration and preload strategy.]

## Layout & Spacing

All spacing uses a **4px unit scale**: 4, 8, 12, 16, 20, 24, 32, 40. Every component's internal padding and margin snaps to this grid.

**Surface dimensions:**
- Popup: 280px wide (fixed), variable height up to 400px
- Toolbar overlay: full viewport width, anchored to top or bottom edge
- Preview page: full viewport (offscreen document), 16:9 player area
- Countdown overlay: full viewport, centered

**Layout rules:**
- Popup is single-column, vertically stacked
- Toolbar is a single horizontal row with centered controls
- Preview page has a video player region + action bar below
- Never use multi-column layouts in V0.1 surfaces

## Elevation & Depth

| Element | Shadow | Blur | y-offset |
|---------|--------|------|----------|
| Toolbar overlay | `shadow-sm` | 8px | 2px |
| Toast / dialog | `shadow-lg` | 12px | 4px |
| Popup (native) | None (browser default) | — | — |

The extension popup is a native Chrome popup — it cannot receive custom shadows. Only HTML surfaces rendered in offscreen documents or content scripts receive elevation.

Elevation is subtle. No layering beyond "surface / overlay / modal" three-deep.

## Shapes

| Token | Value | Used on |
|-------|-------|---------|
| `rounded/sm` | 4px | Inputs, mic toggle, small badges |
| `rounded/md` | 6px | Buttons, cards, popup container |
| `rounded/lg` | 10px | Toolbar, toast, countdown ring container |
| `rounded/full` | 9999px | Integrity badges, recording dot |

Corners are rounded but not pill-shaped (except badges). The 6px default reads as "tool" rather than "social app."

## Components

All components defined from scratch (no inherited UI system). Tokens reference the YAML color/typography/rounded/spacing tables above.

### Popup container
```
background: {colors.background-light} / {colors.background-dark}
padding: {spacing.popup-padding}
radius: {rounded.lg}
width: 280px
```

### Mode selector
```
Two-button radio group: [Full Screen] [Tab]
Active: {colors.primary-light} / {colors.primary-dark} fill, {colors.primary-foreground-light} / {colors.primary-foreground-dark} text, {rounded.md}
Inactive: {colors.muted-light} / {colors.muted-dark} fill, {colors.muted-foreground-light} / {colors.muted-foreground-dark} text, {rounded.md}
Height: 32px
```

### Mic toggle
```
Icon + label row
Checked: {colors.primary-light} / {colors.primary-dark} text
Unchecked: {colors.muted-foreground-light} / {colors.muted-foreground-dark} text
radius: {rounded.sm}
```

### Start button
```
background: {colors.primary-light} / {colors.primary-dark}
foreground: {colors.primary-foreground-light} / {colors.primary-foreground-dark}
radius: {rounded.md}
height: 36px
label: {typography.label}
Full width of popup
Disabled state: opacity 0.5, cursor not-allowed
```

### Recording toolbar
```
horizontal row, centered
background: {colors.background-light} / {colors.background-dark}
radius: {rounded.lg}
shadow: toolbar shadow
padding: {spacing.overlay-padding}
height: 44px
min-width: 180px
```

### Timer
```
foreground: {colors.recording-dot-light} / {colors.recording-dot-dark}
font: {typography.timer}
Shows MM:SS or HH:MM:SS
```

### Pause / Stop buttons
```
Icon buttons, square 32x32
radius: {rounded.sm}
Pause: {colors.foreground-light} / {colors.foreground-dark} icon
Stop: {colors.destructive-light} / {colors.destructive-dark} icon
Hover: {colors.muted-light} / {colors.muted-dark} background
```

### Countdown overlay
```
Full viewport centered
background: semi-transparent (60% opacity)
Number: {colors.countdown-fill-light} / {colors.countdown-fill-dark}, {typography.countdown}
Circle ring: stroke {colors.countdown-fill-light} / {colors.countdown-fill-dark}, animated fill
```

### Preview video player
```
16:9 aspect ratio container
background: #000000
Controls: browser-native <video> element
```

### Preview actions
```
Horizontal row below player
[Download] {colors.primary-light} / {colors.primary-dark} button, {rounded.md}
[Delete] {colors.destructive-light} / {colors.destructive-dark} outline button, {rounded.md}
```

### Crash recovery toast
```
non-modal, bottom-center positioned
background: {colors.background-light} / {colors.background-dark}
border: 1px solid {colors.border-light} / {colors.border-dark}
radius: {rounded.md}
shadow: toast shadow
Content: "A previous recording was found" + [Restore] action button
Auto-dismiss after 8s, or on Restore/Dismiss
```

### Integrity badge
```
Pill badge, {rounded/full}
padding: 2px 8px
font: {typography.body-sm}
Color: {colors.integrity-clean/partial/incomplete}
```

## Do's and Don'ts

| Do | Don't |
|----|-------|
| Use blue for all interactive elements | Use accent blue for non-interactive decoration |
| Use amber only for recording state / countdown / warnings | Use amber for buttons, backgrounds, or chrome |
| System font stack in V0.1 | Load custom fonts in V0.1 |
| Dark/light via `prefers-color-scheme` | Add a manual theme toggle in V0.1 |
| Red recording dot with pulse animation | Any other shape or color for recording indicator |
| Toast for recovery (non-modal, auto-dismiss) | Modal dialog that blocks interaction |
| Single-column popup layout | Tabs, sidebar, or multi-panel popup |
