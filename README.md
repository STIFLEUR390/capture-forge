# Capture Forge 🔥

**L'enregistreur d'écran open-source le plus complet — écrit en Rust/WASM.**

> Extension Chrome & Firefox (P1). Zéro télémétrie, zéro compte, zéro limite.
> *"Record like a pro, without giving up your privacy."*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.85%2B-dea584)
![Status](https://img.shields.io/badge/Status-Development-blue)
![Version](https://img.shields.io/badge/Version-0.1.0--alpha-blueviolet)

---

## Vision

**Capture Forge** est une extension navigateur nouvelle génération pour l'enregistrement d'écran et l'édition vidéo, entièrement écrite en **Rust** et compilée en **WebAssembly** via [Oxichrome](https://crates.io/crates/oxichrome).

| Sous-produit | Version | Statut |
|---|---|---|
| 🎬 **Recorder Core** | V0.1 (P0) | `En développement` |
| ✂️ **Editor + Overlay** | V0.5 (P1) | `Planifié` |
| 🤖 **AI & Enrichment** | V2.0+ (P2) | `Planifié` |

### Pourquoi Capture Forge ?

| Critère | Capture Forge | Loom | Screencastify | OBS |
|---|---|---|---|---|
| Gratuit | ✅ | Freemium (5min) | Freemium | ✅ |
| Compte requis | ❌ | ✅ | ✅ | ❌ |
| Limite durée | ❌ | 5min (free) | 30min (free) | ❌ |
| Watermark | ❌ | ✅ | ✅ | ❌ |
| Éditeur intégré | ✅ | ✅ (cloud) | ✅ | ❌ |
| Firefox | ✅ | ❌ | ❌ | ✅ |
| Rust/WASM | ✅ | ❌ | ❌ | ❌ |
| 100% local / privé | ✅ | ❌ | ❌ | ✅ |

---

## Fonctionnalités

### Recorder Core (V0.1) — en cours

- [x] Machine à états : 9 états, transitions exhaustives
- [x] Capture écran complet (`getDisplayMedia`)
- [x] Capture onglet spécifique (`tabCapture`)
- [x] Capture micro + mixage audio (AudioContext)
- [x] Pause / Reprise avec chronométrage précis
- [x] Stop / Annuler
- [x] Écriture de chunks avec en-tête (magic + index + timestamp + taille + XXH3)
- [x] Gestion d'erreurs robuste (`thiserror`, `Result<T>` partout)
- [ ] Compteur 3-2-1 avec animation
- [ ] Barre d'état (timer, contrôles)
- [ ] Export WebM par concaténation de chunks
- [ ] Stockage OPFS avec cycle de vie formel
- [ ] Récupération de crash
- [ ] Page de prévisualisation (lecture, téléchargement, suppression)
- [ ] Heartbeat keepalive (offscreen doc ↔ SW)

### Editor + Overlay (V0.5 — P1)

- [ ] Lecteur vidéo (play/pause/seek/volume/fullscreen)
- [ ] Trim non-destructif (début/fin)
- [ ] Mute piste audio
- [ ] Crop simple
- [ ] Barre d'annotation flottante (injection shadow DOM)
- [ ] Outils canvas : stylo, surligneur, texte, formes, flèche, blur
- [ ] Historique Undo/Redo
- [ ] Export MP4 (FFmpeg WASM)
- [ ] Export GIF (FFmpeg WASM)

### AI & Enrichment (V2.0+ — P2)

- [ ] Transcription STT locale (sherpa-onnx/WASM)
- [ ] Export SRT / VTT
- [ ] Intégration LLM cloud optionnelle (aisdk)
- [ ] Capture DOM avec filtres de confidentialité

---

## Architecture

```
Popup/UI (Leptos CSR)
    │ ExtensionMessage (serde, via background router)
    ▼
background.rs (service worker)
    │ dispatche vers les modules core
    ▼
recorder.rs ──→ storage.rs ──→ export.rs
    │              │              │
    │         OPFS (chunks)   WebM blob
    │              │
    ▼              ▼
Offscreen doc    RecoveryManager (triple vérification)
(keepalive,
MediaRecorder)
```

### Stack technique

| Couche | Technologie |
|--------|------------|
| Framework extension | [Oxichrome](https://crates.io/crates/oxichrome) v0.2 |
| UI | Leptos v0.7 (CSR) |
| Langage | Rust (wasm32-unknown-unknown) |
| APIs navigateur | web-sys (MediaRecorder, MediaStream, AudioContext, OPFS, Canvas) |
| Sérialisation | serde + thiserror |
| Intégrité chunks | xxhash-rust (XXH3) |

### Machine à états

9 états : `Idle` → `Starting` → `Countdown` → `Recording` ↔ `Paused` → `Stopping` → `Preview` → `Idle`

Toutes les transitions invalides lèvent `RecordingError::StateViolation` — l'état de la session reste inchangé.

---

## Démarrage rapide

### Prérequis

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
cargo install cargo-oxichrome  # Optionnel : régénération manifeste/shims
```

### Build & Test

```bash
# Validation compilation (pas de target wasm32)
cargo check

# Tests unitaires (host natif)
cargo test

# Tests WASM (Chrome headless requis)
wasm-pack test --headless --chrome

# Build WASM
wasm-pack build --target web

# Pipeline oxichrome complet
cargo oxichrome build            # Debug
cargo oxichrome build --release  # Optimisé + wasm-opt -Oz
```

### Charger l'extension

1. Accéder à `chrome://extensions/`
2. Activer le **Mode développeur**
3. Cliquer sur **Charger l'extension non empaquetée**
4. Sélectionner `dist/chromium/`

---

## Structure du projet

```
capture-forge/
├── Cargo.toml              # cdylib Rust
├── src/
│   ├── lib.rs              # Point d'entrée, module declarations, panic hook
│   ├── error.rs            # RecordingError enum (thiserror)
│   ├── recorder.rs         # SessionState (9 états), RecordingSession
│   ├── messaging.rs        # ExtensionMessage (11 variants)
│   ├── stream.rs           # Acquisition écran/onglet/micro
│   ├── lifecycle.rs        # Cycle de vie enregistrement
│   ├── chunk.rs            # Écriture chunks avec en-tête
│   └── ...                 # P1+ modules (feature-gated)
├── docs/                   # Documentation produit et architecture
├── _bmad-output/           # Artéfacts BMAD : PRD, UX, architecture, stories
├── dist/chromium/          # Build output : manifest, background.js, WASM
└── tests/                  # Tests E2E (Playwright)
```

---

## License

MIT — voir [LICENSE](LICENSE).

---

## Feuille de route

| Jalon | Contenu | Cible |
|-------|---------|-------|
| V0.1-alpha | Recorder Core (capture, pause, export WEBM, stockage OPFS) | Actuel |
| V0.2-alpha | Stockage résilient + UX popup + i18n français | Prochain |
| V0.5-beta | Éditeur + Overlay + Firefox | P1 |
| V1.0 | Caméra PiP + sélection région + 18 langues | P1 |
| V2.0+ | AI : STT local, LLM, capture DOM | P2 |

---

## Contribution

Ce projet utilise la méthodologie **BMAD** pour la planification et le développement.
Les stories et le suivi se trouvent dans `_bmad-output/`.

---

*Fait avec 🔥 et 🦀*
