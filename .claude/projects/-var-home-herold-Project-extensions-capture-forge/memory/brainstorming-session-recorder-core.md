---
name: brainstorming-session-recorder-core
description: CaptureForge brainstorming — Recorder Sémantique, résilience et écosystème
metadata:
  type: reference
---

# Brainstorming — Recorder Core (2026-06-19)

**23 idées générées** via What If Scenarios + Reverse Brainstorming.

## Colonne vertébrale

1. **Recorder Sémantique** — Session structurée = format natif, vidéo = vue exportable
2. **Audience Lenses** — `{id, name, visibility, transforms, outputs}` — moteurs déclaratifs
3. **Résilience truth-first** — Manifest append-only, statuts chunk, triple vérification
4. **Lens Integrity Contract** — Vérification installation + exécution, sandbox + capabilities
5. **Rapport d'intégrité natif** — Honnêteté sur l'état = fonctionnalité

## Principes clés

- « Une métadonnée ne doit jamais annoncer plus que ce que le stockage peut prouver »
- « La source ne fait jamais confiance aux lenses »
- « Pipeline vivant ≠ capture vivante »
- « Le dernier chunk est suspect par défaut »

## Prochaines étapes

1. Fiche architecture `AudienceLens` (type Rust, contrats, capacités)
2. Story sprint : statuts chunk + triple vérification + rapport intégrité
3. Spec technique : manifest append-only + Recovery Session Bundle

## Why:
Session a révélé un pattern de pensée : ambition produit → edge cases → formalisation. L'utilisateur excelle à alterner insight créatif et architecture concrète.

## How to apply:
Reprendre la colonne vertébrale (Vision → Résilience → Écosystème) comme guide pour les décisions architecturales futures. Toujours tester une ambition produit par ses edge cases avant de formaliser.
