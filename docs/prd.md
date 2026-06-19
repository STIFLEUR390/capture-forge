# Product Requirements Document — CaptureForge

**Version** : 0.2
**Statut** : Restructuration

---

## 1. Vision

CaptureForge est une extension browser (Chrome + Firefox) d'enregistrement d'écran
et d'édition vidéo, écrite en Rust et compilée en WebAssembly via Oxichrome.
Trois sous-produits indépendants :

```
Recorder Core  ←  Editor  ←  AI / Enrichment
(P0, V0.1)        (P1, V0.5)    (P2, V2.0+)
```

Chacun peut être développé, testé et livré séparément.

---

## 2. Sous-produit A : Recorder Core

**Scope MVP** : capture écran/onglet + micro + pause/reprise + stop + export WEBM natif
+ stockage OPFS + récupération crash minimale.

**Dépendances Chrome API** : `tabCapture`, `desktopCapture`, `getDisplayMedia`,
`storage`, `unlimitedStorage`, `downloads`, `scripting`

**Dépendances Rust** : `web-sys` (MediaRecorder, MediaStream, AudioContext), `opfs`

**Non-goals** (exclu du MVP) :
- Région sélectionnée (P1)
- Webcam-only (P1)
- Codec ladder complet (P1, un seul codec au début)
- Keepalive avancé (P1, simple garde-fou suffit)
- Recovery complexe (P1, perte acceptable au redémarrage SW)
- Remux MP4 (P1)
- Firefox (P1)

### 2.1 User stories — Recorder Core

| ID | Story | Priorité |
|----|-------|----------|
| REC-01 | En tant qu'utilisateur, je peux enregistrer mon écran entier | P0 |
| REC-02 | En tant qu'utilisateur, je peux enregistrer un onglet spécifique | P0 |
| REC-03 | En tant qu'utilisateur, je peux enregistrer mon micro | P0 |
| REC-04 | En tant qu'utilisateur, je peux mettre en pause/reprendre | P0 |
| REC-05 | En tant qu'utilisateur, je peux arrêter et voir un aperçu | P0 |
| REC-06 | En tant qu'utilisateur, je peux annuler un enregistrement | P0 |
| REC-07 | En tant qu'utilisateur, je vois un timer et un compteur 3-2-1 | P0 |
| REC-08 | En tant qu'utilisateur, je peux exporter en WEBM | P0 |
| REC-09 | En tant qu'utilisateur, mon recording est stocké dans OPFS | P0 |
| REC-10 | En tant qu'utilisateur, je peux supprimer mes enregistrements | P0 |

### 2.2 Messages associés

```
desktop-capture → start-recording → countdown-finished →
get-streaming-data → start-recording-tab → [recording] →
stop-recording-tab → video-ready → stopRecording
```

### 2.3 Critères d'acceptation

| ID | Critère | Cible | Navigateur de réf. | Condition |
|----|---------|-------|-------------------|-----------|
| REC-A1 | Recording framerate | ≥ 25 FPS en 1080p | Chrome 120+ sur x86_64 | Desktop avec GPU Intel/AMD/Nvidia. Fallback à 720p accepté |
| REC-A2 | Audio micro + système | Synchronisé (désync < 100ms) | Chrome 120+ | AudioContext mixer basique. Casque recommandé |
| REC-A3 | Pause/reprise | Durée totale correcte | Chrome 120+ | Perte de < 3 frames au moment de la reprise acceptée |
| REC-A4 | Chunks OPFS | Toutes les 10s | Chrome 120+ | Si OPFS indispo, fallback sur IndexedDB |
| REC-A5 | Pas de limite durée | 1h sans plantage | Chrome 120+ | Surveillance mémoire simple (alerte à 80%) |
| REC-A6 | Start guard | Stale lock > 30s → automatiquement nettoyé | Chrome 120+ | Lock stocké dans chrome.storage.local |
| REC-A7 | Export WEBM | < 3s pour 5min de video | Chrome 120+ | Concaténation simple des chunks, sans ré-encodage |
| REC-A8 | Crash recovery | Récupération manuelle proposée | Chrome 120+ | Si chunks OPFS trouvés au démarrage, on propose "Restaurer" |

### 2.4 Storage layout

```
OPFS:
  cloud-chunks/<sessionId>/screen/chunk_*.bin

IndexedDB (fallback):
  Store "chunks" → clé/valeur (sessionId → Blob[])
```

Pas de tracks multiples (audio/camera) dans le MVP. Les chunks contiennent
le flux mixé complet (vidéo + audio).

### 2.5 Architecture Rust

```rust
pub struct RecorderSession {
    state: RwSignal<SessionState>,
    // Pas de codec ladder, pas de keepalive avancé
}

pub enum SessionState {
    Idle,
    Starting,
    Recording { started_at: f64 },
    Paused { paused_at: f64, total_paused: f64 },
    Stopping,
    Exporting,
}

impl RecorderSession {
    /// start() : MediaRecorder simple avec codec par défaut
    /// (première entrée de la liste : VP8/opus pour compatibilité max)
    pub async fn start(&mut self, mode: RecordingMode) -> Result<()> { .. }

    /// stop() : finalise le dernier chunk, concatène les blobs
    pub async fn stop(&mut self) -> Result<()> { .. }
}
```

---

## 3. Sous-produit B : Editor

**Scope MVP** : lecture vidéo, trim début/fin, mute, crop simple, export WEBM.

**Non-goals** (exclu du MVP) :
- Cut au milieu (P1)
- Ajout d'audio (P2)
- Waveform avancée (P1, timeline basique suffit)
- Routes d'édition multiples (une seule route : MediaRecorder + concat)
- MP4 export (P1)
- GIF export (P1)

### 3.1 User stories — Editor

| ID | Story | Priorité |
|----|-------|----------|
| ED-01 | Je peux lire mon enregistrement | P1 |
| ED-02 | Je peux couper le début et la fin (trim) | P1 |
| ED-03 | Je peux couper le son (mute) | P1 |
| ED-04 | Je peux rogner les bords (crop) | P1 |
| ED-05 | Je peux exporter après édition | P1 |

### 3.2 Architecture

```rust
pub struct EditorSession {
    source: SourceRef,         // OPFS session
    trim_start: f64,
    trim_end: f64,
    crop: Option<CropRect>,
    muted: bool,
}
```

L'édition est **non-destructive** : on stocke les opérations, on applique
à l'export en concaténant les frames utiles.

---

## 4. Sous-produit C : AI / Enrichment

**Totalement optionnel.** L'extension fonctionne à 100% sans ce module.

### 4.1 STT local — sherpa-onnx

**Modèle par défaut unique** : `Zipformer EN` (~20MB).
Un seul modèle livré. Pas de sélection multiple au lancement.

**Contraintes** :
- Téléchargé au premier usage (pas pré-packagé)
- Stocké dans OPFS
- Langue : Anglais uniquement à la V0.1 de l'IA
- CPU : 2 threads max
- RAM : < 200MB pour la transcription d'une vidéo de 30min
- Fallback : pas de transcription si le modèle n'est pas téléchargé

### 4.2 LLM cloud — aisdk

- Génération doc + résumé
- Requiert clé API (OpenAI / Anthropic / OpenRouter)
- **Non disponible** si pas de clé configurée

### 4.3 DOM capture

- Désactivé par défaut
- Scope `activeTab` uniquement
- Champs sensibles filtrés automatiquement
- Snapshots stockés dans OPFS

---

## 5. Stratégie Rust / JS

**Rust-first avec shims JS minimaux.**

| Technologie | Approche | Justification |
|------------|----------|---------------|
| Core extension | Rust (Oxichrome) | Tout le métier : recording, storage, state, UI |
| MediaRecorder | web-sys | Bindings Rust natifs |
| FFmpeg WASM | JS shim (`js/ffmpeg.js`) | Librairie npm, pas de crate Rust équivalent satisfaisant |
| MediaPipe | JS shim (`js/mediapipe.js`) | Modèle WASM propriétaire, appelé depuis Rust |
| tabCapture | JS shim (`js/chrome_shim.js`) | API Chrome non exposée dans web-sys |
| OPFS | `opfs` crate | Rust natif |
| sherpa-onnx | `sherpa-onnx` crate | Rust natif (ONNX runtime WASM) |

Les shims JS sont encapsulés derrière une interface Rust (trait + impl).

---

## 6. Non-goals (complets)

| Feature | Raison |
|---------|--------|
| Cloud recording / CaptureForge Pro | Pas de backend, pas de SaaS |
| Compte utilisateur / Login | Zéro auth |
| Télémétrie / Analytics | Zéro tracking |
| Multi-scene editing | Trop complexe pour la V1 |
| Zoom keyframes | Feature niche, ajout futur |
| Export direct YouTube/Vimeo | API tierces instables |
| Support Safari | Marché trop petit, WebExtensions limitées |
| Background removal hors-ligne pur Rust | MediaPipe fait le job en JS interop |
| Speaker diarization | Modèle trop volumineux pour une première version |

---

## 7. Matrice de compatibilité navigateur

| Feature | Chrome 120+ | Firefox 125+ | Edge 120+ |
|---------|-------------|--------------|-----------|
| Screen recording | ✅ | ⬜ (P1) | ✅ |
| Tab recording | ✅ | ⬜ (P1) | ✅ |
| MediaRecorder API | ✅ | ✅ | ✅ |
| OPFS | ✅ | ⬜ (P1) | ✅ |
| Offscreen document | ✅ | ⬜ (non supporté) | ✅ |
| sherpa-onnx WASM | ✅ | ⬜ (P2) | ✅ |

**Navigateur de référence pour le développement** : Chrome 120+ (x86_64).

---

## 8. Modèle de vie privée

| Donnée | Stockage | Réseau |
|--------|----------|--------|
| Enregistrements vidéo | OPFS local | ❌ Jamais |
| Métadonnées (durée, date) | chrome.storage.local | ❌ Jamais |
| Logs debug | chrome.storage.local | ❌ Jamais |
| Clé API (aisdk) | chrome.storage.local | ✅ Appels HTTPS au provider choisi |
| Modèle ONNX (sherpa) | OPFS | ✅ Download unique, puis offline |
| DOM snapshots | OPFS | ❌ Jamais (si feature activée) |
| Usage analytics | ❌ | ❌ Aucun |

---

## 9. Plan de QA

### 9.1 Tests unitaires Rust
- `cargo test` : tous les modules métier (recording, storage, export)
- Tests de la machine à états du RecorderSession (Idle → Starting → Recording → Paused → ...)
- Tests de sérialisation/désérialisation des messages

### 9.2 Tests d'intégration
- Message router : chaque handler reçoit le bon message et produit la bonne réponse
- OPFS : write / read / delete cycles
- Export : concaténation de chunks
- Recovery : détection de chunks orphelins au démarrage

### 9.3 Tests E2E (Playwright)
- Installation / Setup wizard
- Recording → STOP → Preview
- Recording → CRASH → Recovery
- Export WEBM → vérification du fichier
- Pause/Resume → durée correcte
- Suppression d'enregistrement

### 9.4 Tests crash recovery
1. Lancer un recording
2. Tuer le service worker (chrome://extensions)
3. Redémarrer → vérifier que le recording est récupérable

---

## 10. Roadmap séquencée

```
Séquence 1 : Recorder Core (P0, V0.1)
├── Screen + Tab recording
├── Micro simple (pas de mix audio système)
├── Pause / Resume / Stop / Cancel
├── Timer + countdown 3-2-1
├── Export WEBM (concaténation chunks)
├── Stockage OPFS (1 track : video+audio mixé)
├── Crash recovery basique
└── Setup wizard minimal

Séquence 2 : Storage + Recovery (P0, V0.1)
├── Gestion espace (storage.estimate)
├── Suppression recordings
├── Récupération après crash SW
└── Nettoyage OPFS orphelin

Séquence 3 : Overlay + Annotations (P1, V0.5)
├── Toolbar flottante
├── Canvas dessin (pen, highlighter)
├── Texte + formes + flèche
├── Blur zone sensible
└── Undo/Redo

Séquence 4 : Editor simple (P1, V0.5)
├── Lecteur vidéo
├── Trim début/fin
├── Mute audio
├── Crop simple
└── Export après édition

Séquence 5 : Camera (P1, V1.0)
├── PiP webcam
├── Redimensionnement / Drag
├── Background blur (MediaPipe JS interop)
└── Sélection caméra

Séquence 6 : Firefox support (P1, V1.0)
├── Build --target firefox
├── Adaptation offscreen → tab recording
└── Tests E2E Firefox

Séquence 7 : STT local (P2, V2.0+)
├── sherpa-onnx Zipformer EN
├── Download modèle dans OPFS
├── Transcription avec VAD
└── Export SRT / VTT

Séquence 8 : LLM cloud (P2, V2.0+)
├── aisdk + OpenAI/Anthropic/OpenRouter
├── Génération tutoriel Markdown
├── Résumé automatique
└── Smart search

Séquence 9 : DOM capture (P2, V2.0+)
├── Capture DOM au clic
├── Filtre vie privée
├── Stockage OPFS
└── Contexte pour génération doc
```

---

## 11. Feature flags Cargo

```toml
[features]
default = ["recorder", "storage", "export"]
recorder = []         # Recording core
storage = []          # OPFS + IndexedDB
export = []           # WEBM concat
overlay = []          # Content script UI + canvas (P1)
editor = []           # Video editor (P1)
camera = []           # PiP + MediaPipe (P1)
stt = ["sherpa-onnx"] # Transcription locale (P2)
llm = ["aisdk"]       # LLM cloud (P2)
dom = []              # DOM capture (P2)
```

---

## 12. Contre-mesures techniques

| Risque | Impact | Solution |
|--------|--------|----------|
| MediaRecorder échoue sur certaines machines | Moyen | web-sys `canRecord()` → message clair → suggestion de dépannage |
| OPFS non supporté (Safari, vieux Chrome) | Faible | Fallback IndexedDB transparent |
| sherpa-onnx WASM trop lent | Moyen | Modèle Zipformer tiny uniquement, async sur Web Worker |
| Oxichrome v0.2 immature | Haut | Fork du repo Oxichrome si nécessaire, PR upstream |
| Taille WASM > 2MB | Moyen | Feature gates, wasm-opt, brotli |
| Performance canvas en WASM | Faible | web-sys Canvas 2D direct, pas de couche d'abstraction |
