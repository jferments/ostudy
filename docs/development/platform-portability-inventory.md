# Platform portability inventory

This inventory records current operating-system assumptions that must be retired before Android feature work. The machine-readable companion is [`../../scripts/policy/platform-boundary-exceptions.json`](../../scripts/policy/platform-boundary-exceptions.json).

| ID | Current location | Current assumption | Target boundary | Retirement |
| --- | --- | --- | --- | --- |
| PF-CAP-001 | `src/platform/capabilities.ts` | Android hardware Back is inferred from the browser user agent and native services are booleans | native capability snapshot + WebView interaction facts | P1 |
| PF-FILE-001 | `src/platform/files.ts` | a picker returns a durable absolute path | staged import and prepared export handles | P2 |
| PF-FILE-002 | `src/screens/DecksScreen.tsx` | import kind is dispatched from filename suffix | content/package detection in external-file staging | P2 |
| PF-FILE-003 | `src/screens/DocumentsScreen.tsx` | a selected PDF remains readable by path | staged PDF adoption | P2 |
| PF-FILE-004 | `src/screens/settings/StorageBackupSettings.tsx` | backup source/destination is a path | staged restore and prepared backup publication | P2 |
| PF-ASSET-001 | `src/ipc/assets.ts`, `src/editor/richText.ts`, `src/editor/RichTextEditor.tsx` | React receives absolute managed image paths and calls Tauri conversion directly | asset-ID custom protocol | P2 |
| PF-IPC-001 | `src/ipc/decks.ts` | deck import/export accepts source/destination paths | staged ID / prepared export ID | P2 |
| PF-IPC-002 | `src/ipc/backup.ts` | backup commands accept paths | staged ID / prepared export ID | P2 |
| PF-IPC-003 | `src/ipc/documents.ts` | document import accepts a source path | staged document ID | P2 |
| PF-IPC-004 | `src/ipc/generated-import.ts` | generated import accepts a source path | staged content ID | P2 |
| PF-SECRET-001 | `src-tauri/src/ai/secrets.rs` | shared AI policy directly instantiates keyring | `SecureSecretStore` desktop adapter | P1 |
| PF-TIME-001 | `src-tauri/src/util.rs` | domain helpers read `SystemTime` directly | `Clock` port | P1 |
| PF-TIME-002 | `src-tauri/src/study_calendar.rs` | time-zone identity is discovered from Unix files/environment | `TimeZoneProvider` | P1 |
| PF-RESOURCE-001 | `src-tauri/src/system_resources.rs` | worker policy directly probes sysinfo | `ResourceProbe` + pure policy | P1 |
| PF-LIFE-001 | `src-tauri/src/lib.rs`, `src-tauri/src/task_cancellation.rs` | desktop close request is the only lifecycle warning source | `LifecycleCoordinator` | P1 |
| PF-PDF-001 | `src-tauri/src/pdf_render_backend.rs`, `src-tauri/src/pdf_ocr_backend.rs` | desktop PDFium/Tesseract implementations branch by target | document renderer/OCR adapters | Android/iOS |
| PF-DRAFT-001 | `src/screens/CardEditorScreen.tsx` | card content is process-local while only image bytes are durable | typed durable editor draft | P2 |
| PF-DRAFT-002 | `src/screens/TemplatesScreen.tsx` | template content is process-local | typed durable editor draft | P2 |
| PF-DRAFT-003 | `src/screens/cards/BulkEntryModal.tsx` | bulk rows are process-local | typed durable editor draft | P2 |
| PF-DRAFT-004 | `src/controllers/aiProfilesController.ts` | provider metadata and credential text share one dirty state | durable metadata draft; credential remains memory-only | P2 |
| PF-BACKUP-001 | `src-tauri/src/library_backup.rs` | any active asset draft blocks backup | coherent inclusion of persistent editor drafts | P2 |
| PF-BUILD-001 | `src-tauri/tauri.conf.json` | one desktop-oriented config owns every target | base + platform overlays | Android/desktop qualification |
| PF-CI-001 | `.github/workflows/*.yml`, `scripts/*.sh` | Linux is the only qualified native host | self-hosted platform matrix and target-aware tooling | later platform phases |

## Exit discipline

An item leaves this table only when:

1. the replacement boundary is implemented;
2. its tests are in the normal validation gate;
3. its machine-readable exception is removed;
4. maintained architecture documentation describes the new behavior;
5. Linux compatibility remains qualified.
