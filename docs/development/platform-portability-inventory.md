# Platform portability inventory

This document records the operating-system assumptions that remain after the P0-P2 portability foundation. The machine-readable companion is [`../../scripts/policy/platform-boundary-exceptions.json`](../../scripts/policy/platform-boundary-exceptions.json); it contains only source-level exceptions that the static verifier can enforce directly.

## Remaining assumptions

| ID | Current location | Remaining assumption | Target boundary | Retirement |
| --- | --- | --- | --- | --- |
| PF-PDF-001 | `src-tauri/src/pdf_render_backend.rs`, `src-tauri/src/pdf_ocr_backend.rs` | desktop PDFium and Tesseract implementations still branch by target and the desktop OCR adapter launches a subprocess | native renderer, text-rescue, and OCR adapters | Android/iOS |
| PF-BUILD-001 | `src-tauri/tauri.conf.json` and target resources | Linux remains the only production-qualified package configuration | shared base configuration plus target overlays, signing, and packaging | Android and later desktop platform phases |
| PF-CI-001 | `.github/workflows/*.yml` and native-host tooling | Linux is the only fully qualified native host | self-hosted Android, Windows, and macOS/iOS matrices with target-aware doctor/package tools | later platform phases |

## Retired in P0-P2

The following assumptions have left the active inventory because the replacement boundary is implemented, tested in the normal gate, and documented:

- runtime service availability now comes from a native capability snapshot rather than user-agent detection;
- external imports use opaque staged-file identities, content-based kind detection, bounded app-private copying, and startup reconciliation;
- exports are prepared and validated in app-private storage before platform publication;
- deck, generated JSON, PDF, image, and backup IPC no longer exposes user-controlled source or destination paths;
- managed images use stable asset IDs and a custom protocol instead of returning absolute application-data paths to React;
- AI credential policy depends on `SecureSecretStore`, with the desktop keyring isolated behind an adapter;
- time, time-zone identity, lifecycle observation, and resource facts enter through typed ports;
- card, template, bulk-entry, and AI-provider-metadata editors use typed, bounded, revision-safe durable drafts;
- final authoring transactions validate the exact flushed draft and retire it atomically with canonical content and asset promotion;
- complete backup version 3 includes active persistent drafts and staged images without allowing transient operations to produce incoherent snapshots.

## Exit discipline

A remaining item leaves this inventory only when:

1. the replacement boundary is implemented;
2. its behavior and failure modes are exercised by the normal validation gate;
3. any machine-readable exception is removed;
4. maintained architecture and user documentation describe the new behavior;
5. Linux compatibility remains qualified while the new target receives its own native-host checks.
