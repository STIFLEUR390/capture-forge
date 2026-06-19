# Product Brief — CaptureForge

**Statut** : Restructuration (3 sous-produits : Recorder Core / Editor / AI)

---

## 1. Vision

**CaptureForge** est l'enregistreur d'écran open-source le plus complet,
écrit en Rust/WASM. Extension Chrome & Firefox, zéro télémétrie, zéro compte, zéro limite.

> *"Record like a pro, without giving up your privacy."*

---

## 2. Positionnement

**Pour** développeurs, formateurs, testeurs QA
**qui** ont besoin d'enregistrer leur écran et d'éditer localement
**sans** limite, watermark, ou compte obligatoire,
**CaptureForge** est une extension open-source
**qui** offre l'enregistrement écran/onglet, annotations en direct,
un éditeur vidéo, et l'export multi-format — 100% local et privé.

### Matrice concurrentielle

| Critère | CaptureForge | Loom | Screencastify | OBS |
|---------|-------------------|------|---------------|-----|
| Gratuit | ✅ | Freemium (5min) | Freemium | ✅ |
| Compte requis | ❌ | ✅ | ✅ | ❌ |
| Limite durée | ❌ | 5min (free) | 30min (free) | ❌ |
| Watermark | ❌ | ✅ | ✅ | ❌ |
| Éditeur intégré | ✅ | ✅ (cloud) | ✅ | ❌ |
| Firefox | ✅ | ❌ | ❌ | ✅ |
| Rust/WASM | ✅ | ❌ | ❌ | ❌ |

---

## 3. Public cible

- **Alex, développeur** : code reviews, démos techniques, GIF pour PRs
- **Marie, formatrice** : tutoriels longs (30min+), sous-titres
- **Karim, QA** : bug reports avec annotations, export MP4 pour Jira

---

## 4. Features par sous-produit

### Recorder Core (P0, V0.1)

| Feature | Détail |
|---------|--------|
| Screen recording | desktopCapture |
| Tab recording | tabCapture |
| Microphone | AudioContext simple |
| Pause / Resume | |
| Stop / Cancel | |
| Countdown | 3-5s |
| Export WEBM | Concaténation chunks |
| Stockage OPFS | cloud-chunks/<sessionId>/ |
| Crash recovery basique | Chunks persistés |

### Editor (P1, V0.5)

| Feature | Détail |
|---------|--------|
| Video player | Plyr-like (web-sys) |
| Trim début/fin | |
| Mute | |
| Crop simple | |
| Export après édition | |

### Overlay + Annotations (P1, V0.5)

| Feature | Détail |
|---------|--------|
| Toolbar flottante | |
| Pen / Highlighter | web-sys Canvas |
| Text / Shapes / Arrow | |
| Blur zone | |
| Undo/Redo | |

### AI/Enrichment (P2, V2.0+ — optionnel)

| Feature | Moteur |
|---------|--------|
| Transcription STT | sherpa-onnx (local) |
| Sous-titres SRT/VTT | sherpa-onnx |
| Génération tutoriel | aisdk (cloud, clé API) |
| Résumé automatique | aisdk |
| DOM capture | Content script |
| Smart search | aisdk |

---

## 5. Métriques de succès

| Métrique | Cible V0.1 | Cible V1.0 |
|----------|-----------|------------|
| ⭐ GitHub | 500 | 5000 |
| 🐛 Issues résolues | 10/semaine | 20/semaine |
| 🌍 Langues | 2 (EN, FR) | 18 |
| ⚡ FPS recording | 25 (1080p) | 30 (1080p) |
| 🦊 Firefox | ❌ | ✅ |
| 📦 Downloads | 100/sem | 1000/sem |

---

## 6. Roadmap

```
Recorder Core (V0.1)        Editor + Overlay (V0.5)        V1.0            V2.0+
├── Screen/Tab record       ├── Toolbar flottante          ├── Firefox      ├── sherpa-onnx
├── Micro                   ├── Canvas annotations         ├── Camera PiP   ├── aisdk doc gen
├── Pause/Resume/Stop       ├── Video player               ├── 18 langues   ├── DOM capture
├── WEBM export             ├── Trim/Mute/Crop             ├── Export MP4   └── Smart search
├── OPFS storage            └── Export édité               └── Export GIF
├── Crash recovery
└── Setup

Q3 2026                    Q4 2026                        Q1 2027          Q2 2027+
```
