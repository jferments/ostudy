#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

function replaceExact(text, oldValue, newValue, label) {
  if (!text.includes(oldValue)) throw new Error(`${label}: expected text was not found`);
  return text.replace(oldValue, newValue);
}

let backupGuide = await read("docs/user-guide/backups-imports-and-sharing.md");
backupGuide = replaceExact(
  backupGuide,
  `- managed card images;
- retained source PDFs and their durable document-processing state;`,
  `- managed card images;
- active recoverable card, template, bulk-entry, and AI-provider-metadata drafts, including their draft-owned staged images;
- retained source PDFs and their durable document-processing state;`,
  "document draft backup inventory",
);
backupGuide = replaceExact(
  backupGuide,
  `Backup creation is blocked while a document task is active. This ensures the database and managed files are captured at one coherent point rather than packaging an in-progress PDF operation.`,
  `Backup creation is blocked while a document task or transient file/asset operation is active. This ensures the database and managed files are captured at one coherent point rather than packaging an in-progress PDF or half-copied external file. Successfully autosaved persistent editor drafts do not block backup creation: backup format version 3 includes their typed payload, optimistic revision, ownership records, and staged images. Credential inputs remain excluded.`,
  "document coherent backup behavior",
);
backupGuide = replaceExact(
  backupGuide,
  `2. Confirm that decks, images, schedules, recent reviews, settings, and documents are present.
3. Reconfigure or verify AI credentials on the restored machine.`,
  `2. Confirm that decks, images, schedules, recent reviews, settings, documents, and any unfinished recoverable authoring drafts are present.
3. Reconfigure or verify AI credentials on the restored machine.`,
  "document restored drafts",
);
await write("docs/user-guide/backups-imports-and-sharing.md", backupGuide);

let platformServices = await read("docs/architecture/platform-services.md");
platformServices = replaceExact(
  platformServices,
  `## Lifecycle and durability

Lifecycle events are useful opportunities to flush or checkpoint, not guarantees. Durable drafts, transfer ledgers, document checkpoints, idempotent operation identities, and startup reconciliation make abrupt termination safe.`,
  `## Lifecycle and durability

Lifecycle events are useful opportunities to flush or checkpoint, not guarantees. Durable drafts, transfer ledgers, document checkpoints, idempotent operation identities, and startup reconciliation make abrupt termination safe.

Before a canonical card, template, generated-card batch, bulk-entry batch, or AI-profile metadata mutation begins, the frontend flushes the latest draft and supplies its exact draft ID and optimistic revision. Rust validates that reference and commits the canonical records, managed-asset promotion, and draft retirement inside the same SQLite transaction. A failed canonical mutation therefore leaves the recoverable draft active; a lost success response cannot authorize a duplicate targetless batch.

Whole-library backup format version 3 includes active or conflicted persistent drafts and their staged images. Older backup versions remain readable, while transient external-file staging and in-progress document work remain outside a coherent backup snapshot.`,
  "document lifecycle transaction boundary",
);
await write("docs/architecture/platform-services.md", platformServices);

let overview = await read("docs/architecture/overview.md");
overview = replaceExact(
  overview,
  `Command or service orchestrators own logical transactions. Helpers accept a connection or transaction and do not commit secretly. Multi-card scheduling operations, generated imports, deck transfer, review submission, and generated-card persistence validate their complete logical unit before commit.`,
  `Command or service orchestrators own logical transactions. Helpers accept a connection or transaction and do not commit secretly. Multi-card scheduling operations, generated imports, deck transfer, review submission, and generated-card persistence validate their complete logical unit before commit.

Durable authoring follows the same rule. The interface flushes an exact editor-draft revision before final save; the Rust use case validates that revision and commits canonical content, managed-image promotion, and draft retirement in one SQLite transaction. Canonical failure preserves the draft for recovery, while replay of a targetless committed batch is rejected before duplicate cards can be created.`,
  "document atomic authoring transaction",
);
overview = replaceExact(
  overview,
  `.ostudybackup is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Provider metadata may be backed up; credential bytes are excluded because they never enter SQLite.`,
  `.ostudybackup is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Backup format version 3 includes active persistent authoring drafts and their draft-owned staged images; version-1 and version-2 packages remain readable. Provider metadata may be backed up, but credential bytes are excluded because they never enter SQLite or draft payloads.`,
  "document backup v3 architecture",
);
await write("docs/architecture/overview.md", overview);

let roadmap = await read("docs/roadmap.md");
roadmap = replaceExact(
  roadmap,
  `- portable deck exchange, generated JSON import, and complete personal-library backup;`,
  `- portable deck exchange, generated JSON import, and complete personal-library backup including recoverable unfinished authoring drafts;`,
  "update current backup capability",
);
roadmap = replaceExact(
  roadmap,
  `The shared runtime now exposes typed platform capabilities for external files, secure secrets, lifecycle, time zones, and resource observation. External imports are staged into managed storage, exports are prepared before publication, card images use opaque asset identities, and card/template/bulk/AI-profile metadata editors retain bounded device-local drafts across process loss. Android, iOS, Windows, and macOS adapters and packaging remain later platform phases.`,
  `The shared runtime now exposes typed platform capabilities for external files, secure secrets, lifecycle, time zones, and resource observation. External imports are staged into managed storage, exports are prepared before publication, card images use opaque asset identities, and card/template/bulk/AI-profile metadata editors retain bounded device-local drafts across process loss. Final authoring saves retire the exact flushed draft inside the canonical SQLite transaction, and backup format version 3 restores persistent drafts with their staged images. Android, iOS, Windows, and macOS adapters and packaging remain later platform phases.`,
  "update completed portability foundation",
);
roadmap = replaceExact(
  roadmap,
  `### 2. Complete everyday authoring and review ergonomics

Planned improvements include autosaved editor drafts, interrupted-edit recovery, richer image sizing and alignment, and a carefully designed undo-last-review operation that restores an exact prior schedule state without rewriting historical evidence incorrectly.`,
  `### 2. Refine everyday authoring and review ergonomics

Autosaved card, template, bulk-entry, and AI-profile metadata drafts now recover after interruption and participate in complete-library backup. Remaining improvements include richer image sizing and alignment, clearer multi-draft management, and a carefully designed undo-last-review operation that restores an exact prior schedule state without rewriting historical evidence incorrectly.`,
  "remove completed drafts from future roadmap",
);
await write("docs/roadmap.md", roadmap);

const inventory = `# Platform portability inventory

This document records the operating-system assumptions that remain after the P0-P2 portability foundation. The machine-readable companion is [\`../../scripts/policy/platform-boundary-exceptions.json\`](../../scripts/policy/platform-boundary-exceptions.json); it contains only source-level exceptions that the static verifier can enforce directly.

## Remaining assumptions

| ID | Current location | Remaining assumption | Target boundary | Retirement |
| --- | --- | --- | --- | --- |
| PF-PDF-001 | \`src-tauri/src/pdf_render_backend.rs\`, \`src-tauri/src/pdf_ocr_backend.rs\` | desktop PDFium and Tesseract implementations still branch by target and the desktop OCR adapter launches a subprocess | native renderer, text-rescue, and OCR adapters | Android/iOS |
| PF-BUILD-001 | \`src-tauri/tauri.conf.json\` and target resources | Linux remains the only production-qualified package configuration | shared base configuration plus target overlays, signing, and packaging | Android and later desktop platform phases |
| PF-CI-001 | \`.github/workflows/*.yml\` and native-host tooling | Linux is the only fully qualified native host | self-hosted Android, Windows, and macOS/iOS matrices with target-aware doctor/package tools | later platform phases |

## Retired in P0-P2

The following assumptions have left the active inventory because the replacement boundary is implemented, tested in the normal gate, and documented:

- runtime service availability now comes from a native capability snapshot rather than user-agent detection;
- external imports use opaque staged-file identities, content-based kind detection, bounded app-private copying, and startup reconciliation;
- exports are prepared and validated in app-private storage before platform publication;
- deck, generated JSON, PDF, image, and backup IPC no longer exposes user-controlled source or destination paths;
- managed images use stable asset IDs and a custom protocol instead of returning absolute application-data paths to React;
- AI credential policy depends on \`SecureSecretStore\`, with the desktop keyring isolated behind an adapter;
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
`;
await write("docs/development/platform-portability-inventory.md", inventory);

console.log("Updated maintained documentation for the completed P0-P2 foundation.");
