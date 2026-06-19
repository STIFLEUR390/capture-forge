# Spine Pair Review — CaptureForge

## Overall verdict

The spine pair is fundamentally sound and usable: all required DESIGN.md sections are present in canonical order, all EXPERIENCE.md required defaults are present, and the three Key Flows each have named protagonists, numbered steps, climax beats, and failure paths. Two high-severity issues prevent clean downstream consumption — the DESIGN.md prose token references use logical names (`{colors.background}`) that cannot be resolved against the YAML frontmatter (which only has `-light`/`-dark` suffixed keys), and the decision-log claims an i18n decision was recorded in EXPERIENCE.md Foundation when it was not, creating a broken cross-reference for anyone relying on the log as an index.

## 1. Flow coverage — adequate

Checked the EXPERIENCE.md sources frontmatter (PRD, ux-designer.md, product-brief.md) + decision log. Extracted 10 REC stories from the PRD (REC-01 through REC-10) plus features from ux-designer.md and product-brief.md. Three Key Flows present, each with named protagonist (Alex, Marie, Karim), numbered steps, a climax beat, and a failure path.

### Findings

- **[high]** **REC-06 (Cancel recording) has no dedicated Key Flow.** Escape-during-countdown is described as behavior under Component Patterns (Countdown overlay) and Interaction Primitives (`Escape` / `Alt+Shift+X`), but no flow demonstrates cancellation as the primary action with a protagonist. *Fix:* Add a flow where a protagonist starts recording, realises the wrong tab is selected, cancels via Escape, and returns to Idle. Or annotate an existing flow step with the cancel path more visibly.

- **[medium]** **UJ/requirement IDs not mapped.** EXPERIENCE.md does not annotate flows or component patterns with REC-IDs from the PRD. A consumer reading the flows cannot trace back to specific PRD stories without manual cross-referencing. *Fix:* Add REC-ID footnotes to each flow that exercises specific stories (e.g., `[REC-01][REC-02][REC-07]`).

- **[low]** **REC-10 (Delete recordings) is exercised only implicitly.** Karim clicks "Delete" in Flow 3 step 8 but it's a closing note, not a featured interaction. No flow centers on managing or deleting recordings. *Fix:* Minor — acceptable for V0.1 scope.

## 2. Token completeness — adequate

Extracted every token from DESIGN.md YAML frontmatter and every `{path.to.token}` reference in prose.

### Findings

- **[high]** **Prose references use logical names that don't exist in YAML.** Eight prose `{path.to.token}` references resolve to no frontmatter key: `{colors.background}`, `{colors.primary}`, `{colors.primary-foreground}`, `{colors.foreground}`, `{colors.muted}`, `{colors.muted-foreground}`, `{colors.destructive}`, `{colors.countdown-fill}`, `{colors.border}`. The YAML frontmatter only defines `-light`/`-dark` suffixed variants (e.g., `background-light`, `primary-dark`). A naive token resolver would fail. The components YAML section is correct because it uses the explicit `'{colors.primary-light}' / '{colors.primary-dark}'` dual-value convention. *Fix:* Either (a) add logical-name alias keys to the YAML that document the light/dark resolution, or (b) change all prose references to use the explicit `-light`/`-dark` suffixed names with context about which applies in which mode.

- **[medium]** **`recording-dot` and `recording-dot-glow` violate the all-tokens-paired claim.** The YAML comment reads "All tokens exist in light and dark variants" but `recording-dot` and `recording-dot-glow` are single values. The `integrity-*` tokens are also single values, though defensible (semantic badges). *Fix:* Either add `recording-dot-dark` / `recording-dot-glow-dark` variants, or update the comment to call out theme-independent exceptions.

- **[low]** **`spacing.component-gap` defined in YAML but never referenced** anywhere in prose or EXPERIENCE.md. Not a defect — it may be used at implementation time — but a leaner frontmatter could drop it or add a usage note.

## 3. Component coverage — adequate

Extracted every component name used across both files. Cross-referenced each against DESIGN.md (visual spec) and EXPERIENCE.md Component Patterns (behavioral spec).

### Findings

- **[medium]** **Integrity badge has visual spec but no behavioral spec.** DESIGN.md Components (prose) and YAML frontmatter (`integrity-badge-clean`, `integrity-badge-partial`, `integrity-badge-incomplete`) define the visual. EXPERIENCE.md Flow 2 failure path references the `Partial` badge. But EXPERIENCE.md Component Patterns has no Integrity badge entry — no behavioral rules, no state mapping, no interaction. *Fix:* Add an Integrity badge section to EXPERIENCE.md Component Patterns covering: when it appears (export/concat result, crash recovery), what each variant means, click behavior (none? tooltip?), dismissal.

- **[medium]** **`popup-button-secondary` is vestigial.** Defined in DESIGN.md YAML frontmatter but never referenced in DESIGN.md prose, EXPERIENCE.md Component Patterns, or any flow. The popup UI described across both spines has only a Start button (primary). No secondary button exists in any surface. *Fix:* Remove `popup-button-secondary` from YAML, or add a behavioral spec and surface context if planned for V0.2.

- **[low]** **YAML component name `timer-display` vs prose "Timer" — minor inconsistency.** Frontmatter key is `timer-display`; prose Components section titles it "Timer"; EXPERIENCE.md Component Patterns uses "Timer display." All clearly refer to the same component, but a strict name resolver would need aliasing.

## 4. State coverage — adequate

Walked every surface from EXPERIENCE.md Foundation and IA. Expected nine states: Idle, Starting, Countdown, Recording, Paused, Stopping, Preview, Error, CrashRecovery.

### Findings

- **[low]** **Setup / first-install state not covered.** The PRD lists "Setup wizard minimal" in Sequence 1 roadmap. The ux-designer.md has a Setup component in its component tree. But EXPERIENCE.md has no Setup surface, no first-run state, and no permission-granting flow (permissions are assumed via Chrome native dialogs). The decision log rejects a formal onboarding wizard, but the Spine should still note how first-run and permission-granting work (or state that they're handled by Chrome natively). *Fix:* Add a note to EXPERIENCE.md Foundation or State Patterns about the first-install experience.

## 5. Visual reference coverage — spine-only (acknowledged)

No `mockups/`, `wireframes/`, or `imports/` files exist. The `imports/` directory is present but empty. No inline links to visual references in either spine.

The decision log records "Fast Path (No Visual Mockups)" — intentional. However, neither spine's frontmatter explicitly declares "spine-only" or "fast-path" status. The statuses are "draft" (both), which is ambiguous. *Fix:* Add a `status: spine-only` or `status: draft-fast-path` annotation to both frontmatter sections so consumers immediately know no visual references exist.

No findings of severity — this is a known and documented fast-path choice.

## 6. Bloat & overspecification — strong

### Findings

- **[low]** **DESIGN.md Components prose section restates YAML frontmatter.** The 12 prose component specs (Popup container, Mode selector, etc.) largely repeat values already in the YAML `components:` block. E.g., "Popup container" prose lists `background: {colors.background}`, `padding: {spacing.popup-padding}`, `radius: {rounded.lg}`, `width: 280px` — all derivable from the YAML. The prose would be stronger if it focused on design rationale and left exact values to the YAML schema. *Fix:* Reduce each sub-section to design intent (e.g., "The popup container must feel light — thin radius, no shadow.") and move exhaustive value lists to YAML.

- **[low]** **Flow 2 breaks fourth wall with scope note: "Wait — V0.1 doesn't have trim."** This is informative but breaks the key flow narrative convention of showing the user's experience, not editorializing about scope. *Fix:* Restructure as a decision note after the flow rather than a narrative aside.

## 7. Inheritance discipline — adequate

Sources frontmatter checked: `{planning_artifacts}/prds/prd-capture-forge-2026-06-19/prd.md` resolves (found on disk at `_bmad-output/planning-artifacts/prds/…`), `docs/ux-designer.md` resolves, `docs/product-brief.md` resolves. The `{planning_artifacts}` template variable resolution is implicit (not formally documented) but follows BMAD convention.

### Findings

- **[high]** **Decision log claims English/French i18n decision is "Recorded in: EXPERIENCE.md — Foundation section" but it is not there.** No mention of i18n, English-only V0.1, or French V0.2 exists anywhere in EXPERIENCE.md. Anyone reading the decision log as an index will be misdirected. The information does appear in ux-designer.md (section 6) and the PRD, but the decision log's cross-reference is broken. *Fix:* Add an i18n note to EXPERIENCE.md Foundation, or correct the decision log to reference the PRD section.

- **[low]** **Glossary absent across all documents.** No formal glossary exists in any source or spine. Terms are used consistently (Popup, Recording toolbar, Preview page, etc.) but a consumer looking for a single source of term definitions won't find one.

## 8. Shape fit — strong

DESIGN.md sections all present in canonical order: Brand & Style, Colors, Typography, Layout & Spacing, Elevation & Depth, Shapes, Components, Do's and Don'ts. No sections invented, none omitted.

EXPERIENCE.md required defaults all present: Foundation, IA, Voice and Tone, Component Patterns, State Patterns, Interaction Primitives, Accessibility Floor, Key Flows. Required-when-applicable (Inspiration & Anti-patterns) present. "Responsive & Platform" is absent — defensible for a fixed-surface browser extension (popup 280px, toolbar full-viewport, preview full-viewport — no responsive breakpoints needed), but not explicitly justified. *Fix:* Add a one-line justification in Foundation or a new "Platform Notes" subsection explaining why responsive is absent.

## Mechanical notes

- `popup-button-secondary` in DESIGN.md YAML is defined but orphaned — no visual or behavioral spec references it.
- `spacing.component-gap` in YAML is defined but unreferenced.
- DESIGN.md frontmatter `status: draft` and EXPERIENCE.md `status: draft` are consistent.
- Both spines use documents consistently — no broken internal anchor references.

## Finding counts by severity

| Severity | Count | Categories affected |
|----------|-------|-------------------|
| **Critical** | 0 | — |
| **High** | 3 | Token completeness (logical-name resolution gap), Inheritance discipline (decision-log broken ref), Flow coverage (missing Cancel flow) |
| **Medium** | 4 | Component coverage (Integrity badge missing behavioral spec; `popup-button-secondary` vestigial), Token completeness (`recording-dot` no light/dark pair), State coverage (first-install not covered) |
| **Low** | 5 | Bloat (prose restates YAML, fourth-wall break), Inheritance (UJ IDs not mapped, no glossary), Shape fit (responsive not justified), Token completeness (unreferenced token), State coverage (delete not featured) |
