# UX Designer — CaptureForge

**Design d'expérience utilisateur pour l'extension browser**

---

## 1. Parcours utilisateur (Recorder Core — V0.1)

```
Installation → Setup → [Popup] → Countdown → Recording → Stop → Preview → Export

                    Popup                     Recording Overlay
               ┌─────────────────┐      ┌───────────────────────────┐
               │ ○ CaptureForge     │      │ 00:03:42                  │
               │                 │      │ [⏸] [⏹]                  │
               │ [Full] [Tab]    │      │ (toolbar simple)          │
               │                 │      └───────────────────────────┘
               │ [✓] Micro       │
               │ [1080p] [30fps] │      Preview
               │                 │      ┌───────────────────────┐
               │ [Start Record]  │      │ ▶ Play     [Edit]     │
               │ [Alt+Shift+G]  │      │           [Download]  │
               └─────────────────┘      │           [Delete]   │
                                         └───────────────────────┘
```

Pas de région, pas de caméra-only, pas de configuration complexe dans le MVP.

---

## 2. Component Tree Leptos

```
App
├── Popup
│   ├── ModeSelector        (Full Screen | Tab)
│   ├── MicToggle
│   └── StartButton
│
├── RecorderOverlay
│   ├── Timer
│   ├── PauseButton
│   ├── StopButton
│   └── CountdownOverlay
│
├── PreviewPage
│   ├── VideoPlayer
│   └── Actions (Download / Edit / Delete)
│
├── Setup
├── Permissions
├── EditorPage          (P1)
│   ├── Player
│   ├── TrimSlider
│   ├── MuteToggle
│   ├── CropTool
│   └── ExportButton
│
└── ContentScript       (P1 — annotations)
    ├── Toolbar
    ├── Canvas
    └── CameraPiP
```

---

## 3. États et transitions (Recorder Core)

| État | Rendu |
|------|-------|
| `Idle` | Popup prête |
| `Starting` | Spinner "Préparation..." |
| `Countdown` | 3 → 2 → 1 animé |
| `Recording` | Timer + toolbar (pause + stop) |
| `Paused` | Timer clignote "Paused" |
| `Stopping` | Finalisation... |
| `Preview` | Player + actions |
| `Error` | Message + suggestion |
| `CrashRecovery` | "Recording récupérable — Restaurer ?" |

---

## 4. Raccourcis clavier

| Touche | Action | Priorité |
|--------|--------|----------|
| `Alt+Shift+G` | Start recording | P0 |
| `Alt+Shift+X` | Cancel recording | P0 |
| `Alt+Shift+M` | Pause/Resume | P0 |
| `Ctrl+Shift+S` | Stop recording | P0 |
| `Escape` | Annuler / Fermer | P0 |
| `Space` | Play/Pause preview | P0 |

---

## 5. Micro-interactions

- **Countdown** : cercle qui se remplit, 3→2→1→start
- **Stop** : animation de fermeture
- **Pause** : timer clignote, toolbar transparente
- **Export** : spinner + "Exportation..."
- **Crash recovery** : toast "Un enregistrement précédent a été trouvé"

---

## 6. i18n (18 langues — P1)

Structure JSON dans `static/locales/{lang}.json` :

```json
{
  "app": { "name": "CaptureForge" },
  "popup": {
    "start": "Start Recording",
    "fullScreen": "Full Screen",
    "tab": "Tab",
    "microphone": "Microphone"
  },
  "recorder": {
    "paused": "Paused",
    "stop": "Stop",
    "cancel": "Cancel"
  },
  "editor": {
    "trim": "Trim",
    "mute": "Mute",
    "crop": "Crop",
    "export": "Export"
  },
  "ai": {
    "transcribe": "Transcribe",
    "generateDoc": "Generate Tutorial",
    "modelDownload": "Download speech model (20 MB)",
    "modelReady": "Speech model ready"
  }
}
```

Langues : `en`, `fr`, `de`, `es`, `ca`, `hi`, `id`, `it`, `ko`,
`pl`, `pt_BR`, `pt_PT`, `ru`, `ta`, `tr`, `uk`, `zh_CN`, `zh_TW`

---

## 7. Accessibilité

- `aria-label` sur tous les boutons
- Navigation Tab/Enter/Escape
- Contrastes ≥ 4.5:1 (WCAG AA)
- Animations respectant `prefers-reduced-motion`
