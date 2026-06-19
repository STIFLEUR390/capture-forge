---
title: "Product Brief: Capture Forge"
status: final
created: 2026-06-19
updated: 2026-06-19
---

# Product Brief: Capture Forge

## Executive Summary

Capture Forge est une extension navigateur open-source de screen recording, locale, privée et résiliente. Elle permet d'enregistrer et d'exporter sans compte, sans cloud imposé et sans watermark, avec une architecture Rust/WASM pensée pour la reprise après crash et l'évolution vers des dérivations sémantiques.

Construit en Rust compilé en WebAssembly via le framework Oxichrome, Capture Forge cible d'abord Chrome/Chromium (V0.1) puis Firefox (V1). Distribué sur GitHub (nightly, alpha, contributions) et le Chrome Web Store (distribution officielle), sous licence MIT pour le cœur original.

**"Record once, keep control, publish later in different ways."**

---

## The Problem

Les solutions actuelles d'enregistrement d'écran en extension navigateur imposent presque toutes au moins une friction : compte obligatoire, cloud imposé, limites gratuites restrictives, watermark, ou dépendance à une plateforme fermée. Les utilisateurs techniques (développeurs, QA, formateurs) — qui produisent des captures quotidiennement — subissent ces compromis sans trouver d'outil qui combine fiabilité, contrôle et absence de lock-in.

Le coût du statu quo : chaque outil ajoute une barrière (compte, limite, dépendance) qui transforme un geste simple — capturer un écran — en cascade de décisions et d'abandons.

---

## The Solution

Capture Forge est une extension navigateur qui offre :

- **Capture souveraine** : écran, onglet, micro — pause, reprise, arrêt — export WebM. Tout reste en local (OPFS + chrome.storage). 0 compte, 0 télémétrie, 0 watermark.
- **Architecture résiliente** : protocole d'écriture 2 phases (`.partial → .bin`), vérification triple à la récupération, rapports d'intégrité natifs. L'utilisateur n'est jamais face à un « échec silencieux ».
- **Modularité WASM** : le cœur en Rust/WASM permet des performances prévisibles, une empreinte mémoire maîtrisée, et une extensibilité par feature flags. L'utilisateur reçoit un module optimisé pour son besoin.
- **Prêt pour l'évolution** : l'abstraction backend capture (Chrome aujourd'hui, Firefox demain) et l'infrastructure de *chunks* (format natif, non-destructif) posent les fondations du Semantic Recorder des versions futures.

L'expérience utilisateur tient en une phrase : **cliquer → enregistrer → stopper → prévisualiser → exporter**. Pas de détour par un tableau de bord web, pas d'inscription.

---

## What Makes This Different

| Critère | Loom | Screenity | OBS | Capture Forge |
|---------|------|-----------|-----|---------------|
| Compte requis | Oui | Non | Non | **Non** |
| Watermark gratuit | Oui | Non | Non | **Non** |
| Limite durée gratuit | 5 min | Non | Non | **Non** |
| Éditeur intégré gratuit | Paywall | Paywall | N/A | **Inclus (P1)** |
| Cross-browser | Non | Chrome only | Desktop | **Chrome V0.1 → Firefox V1** |
| Technologie | React/Node | React/JS | C++ natif | **Rust/WASM natif** |
| Architecture modulaire | Fermée | Monolithe | Plugins C++ | **WASM modulaire + feature flags** |
| IA locale | Cloud | Non | Non | **Optionnelle (P2, WASM locale)** |
| Open source | Non | Oui (GPLv3) | Oui (GPLv2) | **Oui (MIT)** |
| Vision sémantique | Non | Non | Non | **AudienceLens (V1+)** |

**L'avantage réel :** Capture Forge combine une architecture Rust/WASM modulaire, une persistance locale résiliente (truth-first) et une vision sémantique du recording. Aucun concurrent ne réunit ces trois piliers dans une extension navigateur.

> **Note concurrentielle :** Ce tableau est indicatif — les offres (prix, limites, fonctionnalités) évoluent vite. Plusieurs extensions revendiquent déjà « no sign-up », « offline », « no watermark » ou export local MP4/WebM. La différenciation de Capture Forge est la **combinaison** de tous ces attributs, pas leur somme isolée.

---

## Who This Serves

**ICP prioritaire V0.1 :** développeurs, QA, support technique, formateurs techniques — un public technophile qui comprend et accepte le format WebM et privilégie le contrôle des données à la compatibilité universelle.

**Secondaire :** sales engineering, product education (V0.5+ quand l'export MP4 et l'éditeur seront disponibles).

---

**Alex — Développeur(se)**
- *Besoins* : revues de code asynchrones, démos rapides, GIF pour PRs. Veut un outil sans friction, sans compte, qui livre un fichier.
- *Succès* : capturer une session de débogage en 3 clics et partager le fichier WebM sans passer par un cloud.

**Marie — Formatrice technique**
- *Besoins* : tutoriels longs (30 min+), plusieurs prises, besoin de couper/muter sans outil externe.
- *Succès* : enregistrer, ajuster rapidement le trim, exporter — le tout dans l'extension, sans perte de qualité.

**Karim — Ingénieur QA**
- *Besoins* : rapports de bug avec capture d'écran + annotations, export MP4 pour Jira.
- *Succès* : capturer un bug reproductible, annoter les étapes, livrer un fichier exploitable par le développeur. *(Export MP4 et annotations en P1 — V0.1 propose WebM, déjà lisible par les navigateurs et lecteurs modernes.)*

---

## Success Criteria

| Catégorie | Métrique | Cible V0.1 |
|-----------|----------|------------|
| **Fiabilité** | Taux de capture terminée sans corruption | ≥98% |
| | Taux de récupération après crash SW | ≥95% |
| | Sessions crash-free | ≥99% |
| **Performance** | FPS en capture 1080p | ≥25 FPS |
| | Temps moyen capture → export prêt (5 min) | <3s |
| | Taille moyenne export (5 min 1080p) | <50 Mo |
| **Adoption** | Étoiles GitHub | 500 |
| | Téléchargements hebdo CWS | 100 |
| | Avis CWS | ≥4.0★ |
| **Qualité** | Installation → première capture réussie | ≤3 clics |
| | Aucune donnée télémétrique envoyée | 0 |

*Les cibles chiffrées sont provisoires — voir `.decision-log.md`.*

---

## Scope

### Version 0.1 — Recorder Core (P0)

**In :**
- Capture écran/onglet + micro (GetDisplayMedia / tabCapture)
- Pause, reprise, arrêt
- Export WebM (VP8 + Opus)
- Stockage OPFS
- Protocole d'écriture résilient (chunk status lifecycle)
- Vérification triple à la récupération (manifeste vs fichiers, taille, séquence)
- Rapport d'intégrité natif après recovery
- Heartbeat offscreen → service worker pour résilience MV3
- Popup de contrôle (mode selector, mic toggle, start/stop)
- Compteur + timer pendant l'enregistrement
- Écran de prévisualisation après capture
- Feature flags Rust (recorder, storage, export)

**Explicitement out :**
- Sélection de région (P1)
- Webcam PiP (P1)
- Barre d'outils flottante et annotations canvas (P1)
- Éditeur vidéo (trim, mute, crop) (P1)
- Export MP4 ou GIF (P1)
- Firefox (P1)
- IA/locale STT (P2)
- Audience Lenses (P2)
- Gestion avancée de la mémoire (baisse de qualité automatique) (P1)

### Version 0.5 — Editor + Overlay (P1)
Éditeur non-destructif (trim, mute, crop), barre d'outils flottante, annotations canvas, caméra PiP, Firefox, export MP4/GIF.

### Version 2.0+ — AI & Semantic (P2)
STT locale (sherpa-onnx/WASM), LLM optionnel (aisdk), DOM capture, Audience Lenses.

---

## Key Choices

- **Licence : MIT** pour le cœur original (maximise adoption et contributions externes). [Sous réserve de compatibilité avec tout code hérité Screenity/capture-forge.]
- **Distribution : GitHub + Chrome Web Store.** GitHub pour nightly/alpha et contributions ; CWS pour distribution officielle et crédibilité.
- **Firefox : P1, pas P0.** Firefox est un objectif d'architecture, pas un engagement de parité en V0.1. L'abstraction capture backend est préparée dès le départ, mais la parité fonctionnelle est un engagement V1.
- **Pas de télémétrie, pas de compte.** La confiance est une fonctionnalité, pas un compromis.
- **Feature flags Rust.** L'utilisateur final reçoit un module WASM optimisé pour son besoin ; le code inactif ne grossit pas le binaire livré.

---

## Product Principles

Des contraintes qui guident chaque décision produit, de V0.1 à V2+ :

1. **Local-first by default.** Tout ce qui peut tourner en local tourne en local. Le cloud est optionnel, jamais obligatoire.
2. **No account required.** L'extension s'installe et fonctionne — pas d'inscription, pas de login, pas de création de profil.
3. **No silent failure.** Toute erreur est signalée avec un message compréhensible. L'utilisateur n'est jamais devant un « ça a planté » sans explication.
4. **Source session is immutable.** Une fois capturée, la session source n'est jamais modifiée — les transformations (trim, lenses, exports) produisent des dérivés, pas des altérations.
5. **Optional cloud, never mandatory.** Si un service cloud est ajouté (ex. partage, synchronisation), il reste optionnel et explicitement activé par l'utilisateur.

---

## Risques Produit

| Risque | Probabilité | Impact | Mitigation |
|--------|------------|--------|------------|
| **Complexité MV3 / SW lifecycle** : le service worker peut être tué ~30s après le dernier événement, avec une limite de ~5 min par opération continue | Élevée | Critique | Architecture « design for resurrection » : WAL, heartbeat, offscreen document outlive SW, reconnection |
| **Décalage WebM vs attentes MP4** : le persona QA (Karim) et certains utilisateurs s'attendent à MP4 nativement, alors que V0.1 livre WebM | Élevée | Moyen | Assumer le positionnement tech en V0.1 ; prioriser MP4 en P1 et le communiquer clairement |
| **Parité Firefox** : pas d'équivalent à tabCapture + offscreen ; aucune API native de capture audio d'onglet | Moyenne | Élevé | Abstraction capture backend dès V0.1, mais la parité complète n'est pas promise avant V1 |
| **Confusion V0.1 vs vision sémantique** : le marché peut lire « screen recorder de plus » sans voir l'ambition AudienceLens | Moyenne | Faible | Séparation claire dans les messages : V0.1 = recorder souverain, V1+ = Semantic Recorder |
| **Évolution des politiques Chrome** : Google peut modifier les règles de keepalive, les contraintes de permissions MV3, ou la découvrabilité CWS | Faible | Critique | Architecture résiliente (WAL, heartbeat) ; distribution GitHub comme canal secondaire toujours disponible |

---

## Vision

Si Capture Forge réussit, il devient le recorder de référence pour les développeurs, formateurs et équipes techniques — non pas comme un énième outil de capture, mais comme **le pont entre l'enregistrement et la connaissance**.

À 2-3 ans :
- Une session enregistrée devient un **objet réutilisable** : vidéo pour les utilisateurs, transcript pour la doc, captures pour les bugs, logs pour les devs — tout dérivé d'une même source immuable.
- Un **écosystème de Lenses communautaires** (par ex. `lens:stripe-dashboard`, `lens:figma-plugin`) permet de capturer des contextes spécialisés sans que le cœur de l'extension ait à tout connaître.
- L'extension est **multi-navigateur (Chrome, Firefox, Edge)** et pourrait être déclinée en client bureau léger.

**"Record once, keep control, publish later in different ways."**
