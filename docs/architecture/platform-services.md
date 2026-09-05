# Platform services

ostudy is one shared application with operating-system adapters, not separate product forks. React interaction, native study rules, SQLite migrations, scheduling, analytics, content validation, and document task policy remain shared. Platform services perform OS operations and report capabilities; they do not decide a card's schedule or a library transaction's meaning.

## Composition and ports

The native composition root selects `PlatformRuntime` implementations under [native platform](../../src-tauri/src/platform/). Frontend capability adapters live in [src/platform](../../src/platform/). Shared UI does not import Tauri APIs directly, and feature screens do not bypass domain IPC to reach storage or provider networking.

| Port | Responsibility |
| --- | --- |
| CapabilityProvider | Native-service availability, permission/degraded state, explicit unavailable reasons |
| ExternalFilePort | Bounded import acquisition into private staging and verified export publication |
| SecureSecretStore | Small protected secrets; separate from ordinary learner data |
| LifecycleCoordinator | Ordered foreground/background/suspension/termination observations |
| Clock and TimeZoneProvider | Operation-consistent time facts for shared policy |
| ResourceProbe | Available resource observations, including unknown values |

Viewport/container width, pointer/hover capability, safe areas, and motion preference describe presentation and are not proxies for operating-system identity. Current desktop window controls are native OS chrome. The old frontend `windowControls.ts`/window-controller implementation is not part of the current tree or a required new domain port.

## External-file ownership

The platform acquires selected bytes into bounded app-private staging and returns an opaque staged-file ID plus safe metadata. Native code validates size, hash, purpose, and content before importing. Domain objects never retain arbitrary desktop paths, Android content URIs, or provider document URLs.

`ExternalFilePort.acquire_import` and `publish_export` are required streaming methods. The adapter owns native selection and transfer into/out of a bounded private file; shared Rust verifies its report and advances the transfer ledger. The retired duplicate desktop path-selection/import pipeline is not a second supported API.

Exports are built and validated privately before a save/share picker runs. Canceling a picker is distinct from an unsuccessful domain operation. Opening a share sheet is not evidence of remote delivery. A multi-file export reports partial completion when some independent files were saved before cancellation/failure.

Startup transfer recovery handles interruption after copy/rename but before ledger promotion. It checks path/size/SHA-256, can promote a verified partial/ready file, synchronizes ready bytes, and either completes the state transition or records failure. It must not leave a valid import permanently stranded in staging after restart.

Sources: [external streaming](../../src-tauri/src/external_file_streaming.rs), [recovery](../../src-tauri/src/external_file_recovery.rs), [platform commands](../../src-tauri/src/external_file_commands.rs).

## Managed content

Stored rich content uses `study-asset://<asset-id>` rather than an arbitrary file URL. The managed asset protocol resolves that identity through the native ledger and serves authorized bytes without returning a filesystem path to feature code. The protocol is not an unrestricted file server.

Document derivatives and staged editor images have their own lifecycle/ownership. File I/O is not magically rolled back by SQLite: staging, promotion, quarantine, delete/Undo, and startup reconciliation provide the cross-store recovery contract.

## Secrets and device identity

The AI vault uses the secure-store port for saved values and native memory for an explicit session-only key. Newly typed credentials can transiently exist in a form, but saved keys are not read back through profile DTOs. SQLite, ordinary backups, shared decks, persistent drafts, and routine diagnostics exclude credential bytes.

Trusted-device synchronization is implemented and owns a separate identity/crypto boundary; it is not a future feature. A sender may explicitly opt into the supported API credential transfer for one authenticated encrypted offer. That is a separate secure-store transport path, not permission to include keys in a general backup/package. See [sync architecture](device-sync.md).

Secure-store availability can vary by platform/environment. Non-AI study must not require a functioning credential service. An unavailable service is reported rather than quietly persisting a key in plaintext library settings.

## Lifecycle, drafts, and task durability

Lifecycle events offer opportunities to flush/checkpoint; they are not guaranteed final callbacks. Drafts, operation identities, transfer ledgers, document checkpoints, and startup reconciliation handle abrupt termination.

Before a supported canonical card/template/bulk/generated-batch or nonsecret AI-profile metadata commit, the controller flushes the latest draft and passes its exact ID/revision. Native code validates freshness and coordinates canonical records, image promotion, and draft retirement within the durable mutation. Failure retains the recoverable draft; loss of a success response does not authorize duplicate targetless creation.

Whole-library backup version 3 includes active/conflicted durable drafts and staged images, while older supported backup versions remain readable. Transient file transfers and in-flight document processing remain outside a coherent backup snapshot and can block backup admission.

Desktop close protection is native when document work is active. Android user-started work can use a foreground service with a visible notification and bounded wake lock. Backgrounding the Activity is not the same as terminating the process, but neither mechanism guarantees survival through process death or reboot. Startup never silently restarts provider transmission.

## Android integration

The private [ostudy platform plugin](../../plugins/ostudy-platform/) implements Android file/share/secret/lifecycle operations, PDF rendering/OCR, QR acquisition, and document execution bridging. Kotlin returns bounded application-owned results; Rust retains validation, persistence, OCR selection, resource policy, and generation decisions.

Android uses PdfRenderer and bundled ML Kit Latin text recognition for its PDF/OCR path, not desktop PDFium/Tesseract. Google Code Scanner separately acquires device-sync QR input through Play services, with pasted text as fallback. Neither scanner success nor a valid decoded string bypasses native invitation validation.

Back, safe-area/IME observations, activity restoration, canceled pickers, plugin command routing, and debug/release identity have instrumentation and static contracts. Frontend mocks alone cannot qualify these services.

## Resource and time observations

Adapters report facts; shared code decides worker counts, image queue budgets, request concurrency, and study-day boundaries. Unknown observations trigger conservative behavior. Independent extraction/OCR may run concurrently where safe; a non-thread-safe renderer does not become safe merely because more cores exist.

Clock/timezone snapshots avoid inconsistent operations across a day boundary. Domain calendar policy must not be copied into independent platform-specific or React date logic.

## Enforcement and future platforms

The platform-boundary verifier rejects unlisted debt and tracks explicit temporary exceptions. The external-streaming verifier enforces required ports, startup recovery, cancellation observability, and picker coverage. A documented exception is not an instruction to spread it to new code.

Future platforms implement ports and native/plugin packaging, then qualify actual behavior. Conservative mobile stubs remain explicit unavailable capabilities, not completed iOS support. See [Android development](../development/android.md), [portability inventory](../development/platform-portability-inventory.md), and [the comprehensive architecture](overview.md).
