# Platform services

ostudy is one application, not a collection of platform forks. React feature views, Rust study logic, SQLite migrations, scheduling, analytics, import validation, managed content, and AI task policy remain shared. Operating-system differences enter through small ports selected by the native composition root.

## Dependency direction

```text
React feature views
        ↓
controllers and typed IPC
        ↓
Rust application services
        ↓
repositories and platform ports
        ↓
desktop / Android / Apple adapters
```

Native adapters do not decide scheduling rules, import validity, deletion semantics, or database transactions. They report capabilities and perform operations that inherently cross an OS sandbox.

## Runtime ports

`PlatformRuntime` owns:

- **CapabilityProvider** — reports native service availability and permission/degradation state;
- **ExternalFilePort** — transfers selected external bytes into bounded app-private staging and publishes verified app-private exports without exposing native references to product code;
- **SecureSecretStore** — persists small protected secrets;
- **LifecycleCoordinator** — publishes ordered foreground/background/suspension/termination observations;
- **Clock** and **TimeZoneProvider** — provide operation-consistent time facts;
- **ResourceProbe** — reports CPU, memory, thermal, battery, and foreground facts where available.

The capability snapshot also reports native desktop maximize/restore and full-screen availability as independent service capabilities. The operations themselves are presentation-facing window-manager work, so the frontend implementation remains behind `src/platform/windowControls.ts` instead of becoming a Rust domain port. That adapter uses Tauri's current-window API, while a controller owns native-state reconciliation and mutation serialization. Android and conservative mobile adapters return explicit unavailable reasons; shared UI never substitutes browser full-screen or simulated dimensions.

Viewport width, hover, pointer precision, safe areas, and reduced-motion preference remain WebView/media capabilities because they describe presentation rather than OS identity.

## External files

Imports are copied into app-private staging before validation or long-running work. The application receives an opaque staged-file ID and safe metadata. Exports are built and validated privately before a platform publisher asks where to save or share them.

The registered application commands always use `ExternalFilePort.acquire_import` and `ExternalFilePort.publish_export`. Those streaming methods are required trait methods: every concrete platform adapter must implement them or the build fails. The platform adapter owns both system selection and the byte transfer into or out of a bounded app-private file. Shared Rust then verifies the adapter-reported byte count, hashes and validates imported content, and commits the transfer ledger. The retired desktop path-selection API and its duplicate staging/publication pipeline have been removed rather than retained as compatibility shims.

Transfer promotion is crash-recoverable. If the process exits after a staged import is copied or renamed but before the ledger moves from `staging` to `staged`, startup reconciliation verifies the recorded path, size, and SHA-256, promotes a remaining partial file when necessary, synchronizes the ready file, and either completes the ledger transition or marks the transfer failed. A valid ready file therefore cannot remain permanently stranded in `staging` after restart.

Durable domain objects never contain a desktop path, Android content URI, or iOS document-provider URL.

## Managed content URLs

Rich documents retain `study-asset://<asset-id>` as their durable image identity. React receives asset metadata without a filesystem path. A custom protocol resolves the ID through the managed asset ledger and serves only verified bytes.

## Secrets

The AI credential vault uses `SecureSecretStore`; session-only values remain in memory. Future sync cryptographic identities use a separate device-key interface. Credentials and private keys are excluded from SQLite learner data, backups, drafts, shared decks, and diagnostics.

## Lifecycle and durability

Lifecycle events are useful opportunities to flush or checkpoint, not guarantees. Durable drafts, transfer ledgers, document checkpoints, idempotent operation identities, and startup reconciliation make abrupt termination safe.

Before a canonical card, template, generated-card batch, bulk-entry batch, or AI-profile metadata mutation begins, the frontend flushes the latest draft and supplies its exact draft ID and optimistic revision. Rust validates that reference and commits the canonical records, managed-asset promotion, and draft retirement inside the same SQLite transaction. A failed canonical mutation therefore leaves the recoverable draft active; a lost success response cannot authorize a duplicate targetless batch.

Whole-library backup format version 3 includes active or conflicted persistent drafts and their staged images. Older backup versions remain readable, while transient external-file staging and in-progress document work remain outside a coherent backup snapshot.

## Resources

Adapters observe the device. Shared Rust policy decides how many workers, bitmaps, or network requests are safe. Unknown mobile data results in conservative plans.

## Enforcement

`scripts/verify-platform-boundaries.mjs` rejects new platform debt and requires temporary exceptions to be removed when their code disappears. The maintained exception inventory names the phase responsible for each existing violation. `scripts/verify-external-file-streaming.mjs` additionally enforces the streaming-only platform contract, startup transfer recovery, Android cancellation observability, and independent picker smoke coverage.
