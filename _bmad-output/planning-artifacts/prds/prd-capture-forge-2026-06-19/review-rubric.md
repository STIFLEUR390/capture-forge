# PRD Quality Review — CaptureForge

## Overall verdict

A high-substance PRD with clear strategic coherence and honest scope management. Recorder Core (P0) is tightly specified with verifiable acceptance criteria that an engineering team can build from. The three gaps that reduce downstream usefulness are: (1) Editor (P1) and AI (P2) sub-products lack any acceptance criteria, (2) all user stories use the generic "As a user" protagonist despite strong persona work that could name them, and (3) no glossary exists for domain nouns that are reused across sections. None of these block P0 implementation, but they weaken story creation for later phases and make the PRD harder to source-extract from.

## Decision-readiness — strong

Decisions are stated as decisions throughout. MP4 deferral is explicit (§1 "MP4 export is treated as priority P1"), Firefox support is deferred (§12 "No Firefox-specific work in V0.1 or V0.5"), and the "Explicitly deferred to V0.2 / V0.3" list (§6.1) names what is cut and why. Trade-offs name the thing given up, not just the thing chosen — IndexedDB fallback is deferred because "OPFS is reliably available on Chrome 120+; fallback adds test surface without user-facing value at launch" (§6.1). Counter-metrics (§3) surface the negative consequences of success (binary bloat, memory exhaustion, permission creep, privacy risk). Open Questions (§19, OQ-01 through OQ-07) are genuinely open: each has a resolution condition and a trigger event, and none answer themselves in the next sentence.

### Findings
- **[high]** Missing `[NOTE FOR PM]` callouts at deferred decisions (§ 6.1, § 14) — The PRD surfaces scope tensions well through tables and prose, but never uses the `[NOTE FOR PM]` convention that downstream tools (story creation, sprint planning) rely on to spot unresolved judgment calls. *Fix:* Add `[NOTE FOR PM]` at the four or five places where a scope trade-off was a close call (IndexedDB deferral, VP8-only decision, Region selection shunting to P1).
- **[low]** Open Question ownership all marked `[TBD]` (§ 19) — Every OQ owner is `[TBD]`. For OQ-01 (codec strategy) and OQ-04 (chunk size), ownership should be assigned (e.g., "engineering lead after first benchmarks") so resolution path is clear. OQ-07 (re-recording threshold) is inherently cross-functional but could name a driver.

## Substance over theater — strong

No furniture across any sub-dimension. The three primary personas (Alex, Marie, Karim) each drive specific features that are traceable through the PRD — Marie's long-tutorial needs map to pause/resume and camera PiP; Karim's QA workflow drives blur and MP4 export; Alex's developer use case drives keyboard shortcuts and GIF export. This is well-earned. NFRs (§10) carry specific, measurable targets (≥25 FPS, <100ms desync, <500MB RAM, <16ms annotation latency) with measurement methods — not boilerplate. The vision statement ("Record like a pro, without giving up your privacy") is product-specific: it names the category and the differentiator, and the five Core Tenets (§1) are binding commitments (zero telemetry, zero account, no artificial limits, local-first, open source) that could not swap into a generic PRD. No innovation theater or differentiation-section-for-its-own-sake is present.

### Findings
*(No substantive findings. Dimension is strong.)*

## Strategic coherence — strong

The PRD has a clear thesis: a privacy-first, open-source screen recorder that grows bottom-up from a Dev/QA beachhead through community-driven adoption. Feature prioritization follows from this thesis — P0 maximizes capture reliability and crash resilience (the "no data loss" promise that matters to technical users), P1 adds editing polish, P2 adds optional AI. This is not a backlog with headings. The six-phase roadmap (§18) names themes, not just features ("Reliable capture, resilient recovery"; "Closing the UX gaps"). Success Metrics (§3) validate the thesis: session completion rate and recovery success rate directly measure the reliability promise; export success rate measures the no-data-loss promise. Counter-metrics (§3) are present and product-specific (WASM binary size bloat, memory exhaustion, permission creep).

### Findings
*(No substantive findings. Dimension is strong.)*

## Done-ness clarity — adequate

Recorder Core (P0) is the strength here. The ten acceptance criteria (§6.3, REC-A1 through REC-A10) carry measurable targets (≥25 FPS, desync <100ms, <2s start latency, <3s WebM export), reference browser conditions, and named constraints ("Desktop with GPU... Fallback to 720p accepted"; "<3 frames lost at resume boundary accepted"). An engineer knows what "done" means for each P0 feature. No vague language like "reasonable performance" or "user-friendly" appears in the ACs.

### Findings
- **[high]** Editor (P1) sub-product has zero acceptance criteria (§ 7) — §7 lists 11 user stories (ED-01 through ED-11) and an `EditorSession` struct, but there is no equivalent of REC-A1–A10 for any editor feature. What "trim done" means (frame accuracy? instant preview? re-export correctness?) is entirely implicit. Downstream story creation for P1 has no target to build from. *Fix:* Add acceptance criteria for each editor feature, at minimum: trim frame accuracy (±1 frame), annotation replay FPS, mute silence-threshold, crop aspect-ratio preservation, export-after-edit quality parity with source.
- **[medium]** AI sub-product (P2) has no acceptance criteria (§ 8) — While P2 is explicitly optional and feature-gated, the PRD includes detailed specifications (model size, RAM, thread count, output formats) that imply a quality target but never state a pass/fail condition. For a PRD that feeds story creation, these should at minimum have one acceptance criterion per AI feature. *Fix:* Add one verifiable condition per P2 feature (e.g., "STT word error rate <15% on clean speech"; "DOM capture roundtrip preserves element structure").
- **[low]** REC-A8 "Manual restore proposed" is underspecified (§ 6.3) — The criterion says "If OPFS chunks found at startup, offer 'Restore'" but does not specify the interaction (dialog? toast? notification badge?). The UI States table (§6.4) clarifies this as a toast, but the AC itself should carry the constraint. *Fix:* Recast REC-A8 to include the UI mechanism: "Show a toast with 'Restore' action when OPFS chunks are found at startup."

## Scope honesty — strong

Non-Goals (§14) is a substantive section with 14 entries, each carrying a rationale that explains why the feature is out of scope rather than asserting it. The "Explicitly deferred to V0.2 / V0.3" sublist in §6.1 serves the same function for P0-scoped omissions. De-scoping is never silent: MP4 export, Firefox, storage manager, keyboard shortcut UI, setup wizard, IndexedDB fallback are all named as deferred with justification. The Assumptions table (§19, ASSUMPTION-01 through ASSUMPTION-07) identifies 7 key premises and assesses the risk of each being wrong — this is genuine, not pro-forma. The open-items density (7 OQs + 7 assumptions) is appropriate for a PRD scoped to P0 build.

### Findings
- **[high]** Assumptions are only in the appendix table, not tagged inline in the document body (§ 19) — ASSUMPTION-01 (Oxichrome stability) should appear as `[ASSUMPTION-01: Oxichrome v0.2 stable enough for production use]` at the point in §5 where Oxichrome is named as the framework dependency. Same for ASSUMPTION-05 (OPFS reliability) at the point in §6 where OPFS is named as sole storage path. Without inline tags, a section reader cannot see what reasoning depends on an unverified premise. *Fix:* Add `[ASSUMPTION-N: ...]` inline markers at the dependency-declaration points in §5 and §6, and verify they roundtrip with the §19 index.
- **[medium]** Missing `[NON-GOAL for MVP]` inline markers (§ 6.1, § 14) — The Non-Goals section and the deferred-items list carry the content but use prose tables rather than inline markers. A downstream tool scanning for `[NON-GOAL for MVP]` finds nothing. *Fix:* Add `[NON-GOAL for MVP]` to the first sentence of each deferred item in §6.1 and to the most impactful items in §14 (MP4 export, Firefox, storage manager).

## Downstream usability — adequate

ID continuity is clean across all sections — REC-01 through REC-14, ED-01 through ED-11, REC-A1 through REC-A10, NFR-PERF/REL/SEC/A11Y/I18N sequences, OQ-01 through OQ-07, ASSUMPTION-01 through ASSUMPTION-07. All contiguous, no gaps or duplicates. Each section makes sense when read independently: cross-references use section numbers and IDs rather than "see above." The message protocol enum (§6.5), storage layout (§6.6), and chunk lifecycle (§6.6) are precise enough for direct code translation.

### Findings
- **[high]** No glossary (§ missing) — Domain nouns central to the PRD are never defined in one place: "chunk lifecycle," "integrity report," "triple verification," "session manifest," "AudienceLens," "recording mode." These terms are used consistently but a downstream consumer (story creator, architect) must infer definitions by cross-referencing. *Fix:* Add a Glossary section between §4 (Product Principles) and §5 (Product Architecture), defining each domain noun with one sentence.
- **[high]** All user stories use generic "As a user" protagonist (§ 6.2, § 7.2) — The PRD invests in three named personas (Alex, Marie, Karim) in §2, but carries none of them into the user stories that those personas motivate. REC-01 through REC-14 and ED-01 through ED-11 all begin "As a user." An engineer or story creator cannot tell which story serves which persona without manual cross-referencing. *Fix:* Recast each user story to name its primary persona: "As Alex, I can record my entire screen"; "As Marie, I can pause and resume a recording without losing data"; "As Karim, I can blur sensitive areas of the screen."
- **[medium]** No acceptance criteria section for Editor (P1) UJs (§ 7) — Beyond the missing ACs noted in Done-ness clarity, the structural gap means downstream story creation has no criteria section to source-extract from at all. This reduces the PRD's usability for UX and architecture handoff on P1. *Fix:* Add an acceptance criteria table parallel to §6.3 for all P1 features.
- **[low]** Cross-references to external docs without context (§ 5.5, § 9.4) — §5.5 ends with "For full detail see docs/architect.md" and §9.4 references `docs/audience-lens-architecture.md`. For a reader without access to those documents, the workspace structure and lens architecture guidance is incomplete. *Fix:* Add one-sentence summaries of what each external doc contributes at the cross-reference point.

## Shape fit — adequate

The PRD's three-sub-product structure matches the product's phased roadmap, and the independent-build claim ("A bug or delay in one sub-product never blocks the others") is backed by architectural evidence (§5.3, §5.4). The consumer-product shape is well-served by the UJ structure, the UI states table (§6.4), and the privacy model (§11). The PRD is not over-formalized. However, for a consumer product with meaningful UX and three named personas, the rubric's guidance is that UJs with named protagonists are load-bearing — and the PRD uses generic "As a user" throughout, which is a shape-fit miss.

### Findings
- **[high]** Generic "As a user" protagonist contradicts persona investment (§ 2 vs § 6.2) — The PRD did the hard work of defining three specific personas with distinct pain points and feature needs, then abandoned them in the user stories. For a consumer product, this is the dimension where named protagonists matter most. A new reader of REC-01 ("As a user, I can record my entire screen") cannot tell if this serves Alex, Marie, or Karim (it serves all three, but that's a discovery cost). *Fix:* Same as the downstream-usability finding — name the primary persona in each UJ.

## Mechanical notes

- **Glossary drift:** No glossary exists, so drift cannot be assessed. Key nouns appear to be used consistently across sections ("chunk lifecycle" follows the same four-state model in §6.1, §6.6, and §18). No case or plural inconsistencies spotted.
- **ID continuity:** All sequences (REC, ED, REC-A, NFR-PERF, NFR-REL, NFR-SEC, NFR-A11Y, NFR-I18N, OQ, ASSUMPTION) are contiguous and gap-free.
- **Assumptions Index roundtrip:** The 7 assumptions in §19 are indexed with stable IDs, but none appear as inline `[ASSUMPTION: ...]` tags in the document body. The index is complete; inline markers are missing.
- **UJ protagonist naming:** All 25 user stories use "As a user." No named protagonists. Three supporting-case UJs (REC-11–REC-14) also use generic protagonist.
- **Required sections present:** Vision, Target Audience, Success Metrics, Product Principles, Architecture, FRs with UJs, NFRs, Non-Goals, Open Questions & Assumptions, Adoptions Thesis, Roadmap — all present. Missing: Glossary.
