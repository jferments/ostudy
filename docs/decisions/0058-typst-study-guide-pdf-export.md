# ADR 0058: Typst lays out study-guide PDFs

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

Study-guide PDF export drew every line itself through `oxidize-pdf`'s page
API: a hand-written flow writer measured text with approximate character
widths, wrapped it, broke pages, and drew a bounded subset of math from its own
layout boxes. It produced a correct, plain document, but every design element
(colour tiles, two-column grids, step cards, a cram sheet) meant more manual
geometry, and the approximate metrics limited justification and hyphenation.
The maintainer asked for professionally designed guides in a chosen style with
selectable themes.

## Decision

Lay out study-guide PDFs with the Typst compiler, embedded as a library
(`typst`, `typst-layout`, `typst-pdf` and `typst-assets` `0.15.1`, Apache-2.0,
pure Rust, exact pins).

- `document_study_guide_export` still composes one frozen
  `EffectiveGuideExport` from the generated guide and the learner's overlays.
  `guide_typst.rs` turns it into JSON and compiles a fixed template,
  `typst/guide.typ`, against an in-memory world holding only the template, its
  themes, the JSON and the figure PNGs. The world has no file system access,
  no package downloads and no clock.
- Learner- and model-authored text stays data. The template places JSON
  strings as text; it never evaluates them. The only evaluated code is math
  that `math_layout::typst_markup` builds from its parsed LaTeX subset, where
  letters are separate tokens and every other character is a quoted string.
  Expressions outside the subset stay visible LaTeX source, as before.
- Themes (`navy`, `sage`, `slate`, `plum`) change colours and typefaces, not
  structure. The export commands take an optional `theme` that defaults to
  `navy`; an unknown theme is rejected at the IPC boundary.
- The PDF ends with a cram sheet (key concepts, key-idea callouts, terms,
  warnings and misconceptions) and a self-test of the guide's retrieval
  checks, both gathered from the guide's own content rather than generated.
- Fonts are compiled into the binary so output does not depend on the device:
  IBM Plex Sans, Lato, Sora, Roboto Slab and Caladea as TrueType files that
  `scripts/install-local-fonts.mjs` downloads into
  `src-tauri/resources/pdf-fonts` (Typst cannot read WOFF2), plus Libertinus
  Serif, New Computer Modern Math and DejaVu Sans Mono from `typst-assets`.
  Noto Sans, DejaVu Sans or Arial from the operating system are added when
  present for wider script coverage. `build.rs` fails with the install command
  when the TrueType files are missing.
- `oxidize-pdf` stays the PDF parsing and extraction backend; it no longer
  writes study guides.

## Consequences

- The application binary grows by the Typst compiler and roughly 11 MB of
  fonts (9.6 MB of it is `typst-assets`' bundled set, which includes faces
  the template does not use).
- Exports gain real text shaping, kerning, justification, hyphenation and
  Typst's math layout, PDF bookmarks from headings, linked contents with page
  numbers, and repeated table headers. Running headers, footers and repeated
  table headers are tagged as artifacts, so text extraction skips them.
- Typst memoizes compilation process-wide; the exporter evicts that cache after
  each export because nothing is reused between exports.
- A richer self-test (multiple-choice questions with distractors) needs new
  generated content and is not part of this change.
