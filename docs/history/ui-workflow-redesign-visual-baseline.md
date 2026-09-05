# UI workflow redesign Stage 0 visual baseline

This baseline is the reproducible pre-change visual evidence for the learning-workflow redesign in [ADR 0049](../decisions/0049-learning-workflow-ui-simplification.md). It is anchored to source commit `d8e723c1cbc1a60f92881ca4e92b44268a418309` and fixture version `flashcards-library-v1`.

The evidence is intentionally automated. Successful capture and verification prove that the named source, fixture, themes, layout classes, viewport dimensions, PNG files, and cryptographic hashes form a complete matrix. They do **not** mean that a human visually approved the captures. The Stage 13 theme/layout review and other subjective manual acceptance remain separate and open.

## Scenario and matrix

Capture starts ostudy with fresh temporary XDG data, config, cache, state, and runtime directories. Through existing native UI actions, it creates the folder **Clinical foundations**, the unsorted deck **Cardiopulmonary essentials**, and one Basic card, then returns to **Flashcards > Decks**. No database, profile, document, credential, managed asset, or other content is copied from a developer's live application state.

The matrix uses CSS viewport dimensions and asserts the layout class resolved on `document.documentElement.dataset.layoutMode`:

| Layout class | CSS viewport |
| --- | --- |
| `wide` | 1440 × 900 |
| `medium` | 1024 × 768 |
| `compact` | 390 × 844 |

Each viewport is captured in Light, Dark, Console, and Synthwave for 12 images total. The native window is adjusted from measured inner dimensions rather than assuming that a requested outer window size equals the CSS viewport. The manifest records both dimensions, the final outer window rectangle, device-pixel ratio, PNG dimensions, resolved theme/layout, source SHA, fixture version, path, and SHA-256 content hash.

## Read-only verification

Ordinary verification never updates expected images:

```bash
./scripts/ui-visual-baselines.sh verify
```

It rejects duplicate JSON keys, capture entries, matrix coordinates, paths, or hashes; missing/extra images; noncanonical or traversing names; source/scenario drift; wrong theme/layout metadata; wrong viewport or PNG dimensions; and content-hash mismatches.

Focused verifier regressions run with:

```bash
python3 -m unittest discover -s tests/scripts -p 'test_ui_visual_baseline.py'
```

## Explicit regeneration

Regeneration is never part of an ordinary test. First build the exact standalone artifact, then run the explicit capture operation:

```bash
./scripts/build.sh
./scripts/ui-visual-baselines.sh capture
./scripts/ui-visual-baselines.sh verify
```

Capture requires the existing maintained Linux native stack: the staged standalone Tauri binary, `tauri-driver`, `WebKitWebDriver`, Xvfb, and Python's standard library. It launches a `1920×1200×24` virtual display at 96 DPI, uses only fresh temporary application state, persists each theme through the Appearance UI, waits for fonts/overlays/toasts/busy states/animations to settle, and uses the W3C screenshot endpoint. It captures into a temporary sibling directory, verifies the full matrix, and replaces the intentional baseline directory only after all 12 entries pass.

The committed evidence lives under `tests/visual-baselines/ui-workflow-redesign-stage-0/`, not generated build output. WebKitGTK raster output can change with native runtime or font revisions; such changes require an explicit regeneration and review of the new evidence, never silent update-by-default behavior.
