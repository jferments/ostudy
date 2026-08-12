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
- **ExternalFilePort** — selects external sources and destinations without exposing their native references to product code;
- **SecureSecretStore** — persists small protected secrets;
- **LifecycleCoordinator** — publishes ordered foreground/background/suspension/termination observations;
- **Clock** and **TimeZoneProvider** — provide operation-consistent time facts;
- **ResourceProbe** — reports CPU, memory, thermal, battery, and foreground facts where available.

Viewport width, hover, pointer precision, safe areas, and reduced-motion preference remain WebView/media capabilities because they describe presentation rather than OS identity.

## External files

Imports are copied into app-private staging before validation or long-running work. The application receives an opaque staged-file ID and safe metadata. Exports are built and validated privately before a platform publisher asks where to save or share them.

Durable domain objects never contain a desktop path, Android content URI, or iOS document-provider URL.

## Managed content URLs

Rich documents retain `study-asset://<asset-id>` as their durable image identity. React receives asset metadata without a filesystem path. A custom protocol resolves the ID through the managed asset ledger and serves only verified bytes.

## Secrets

The AI credential vault uses `SecureSecretStore`; session-only values remain in memory. Future sync cryptographic identities use a separate device-key interface. Credentials and private keys are excluded from SQLite learner data, backups, drafts, shared decks, and diagnostics.

## Lifecycle and durability

Lifecycle events are useful opportunities to flush or checkpoint, not guarantees. Durable drafts, transfer ledgers, document checkpoints, idempotent operation identities, and startup reconciliation make abrupt termination safe.

## Resources

Adapters observe the device. Shared Rust policy decides how many workers, bitmaps, or network requests are safe. Unknown mobile data results in conservative plans.

## Enforcement

`scripts/verify-platform-boundaries.mjs` rejects new platform debt and requires temporary exceptions to be removed when their code disappears. The maintained exception inventory names the phase responsible for each existing violation.
