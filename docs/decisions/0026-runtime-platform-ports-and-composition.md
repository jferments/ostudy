# ADR 0026: Runtime platform ports and composition

- **Status:** Accepted
- **Date:** 2026-08-11
- **Scope:** Cross-platform preparation phases P0–P2

## Context

ostudy already keeps most study behavior in shared React and Rust code, but operating-system details still enter through several unrelated seams: browser user-agent detection, direct Tauri dialog and asset helpers, keyring and sysinfo calls, Unix-specific time-zone discovery, desktop window-close handling, and path-valued import/export commands. Those seams are small enough to replace without forking the product, but only if the dependency direction is explicit before Android work begins.

## Decision

ostudy uses narrow, typed platform ports. Shared application and domain code may depend on capability contracts, but it may not depend directly on Tauri plugins, WebView implementation details, keyring, sysinfo, Android/Kotlin APIs, Apple/Swift APIs, or target-specific file references.

The native composition root creates one `PlatformRuntime` containing independently testable services:

- `CapabilityProvider`;
- `ExternalFilePort`;
- `SecureSecretStore`;
- `LifecycleCoordinator`;
- `Clock`;
- `TimeZoneProvider`;
- `ResourceProbe`.

Feature code does not call a generic stringly typed platform dispatcher. It asks for the narrow service required by the use case. Platform adapters report facts and perform OS-bound I/O; shared Rust services retain validation, transaction, recovery, scheduling, import, and merge decisions.

The dependency direction is:

```text
React feature views
        ↓
feature controllers and typed IPC
        ↓
Rust application services
        ↓
platform ports + repositories
        ↓
desktop/mobile adapters
```

`cfg(target_os = ...)` belongs in composition or adapter modules, with the existing document renderer/OCR adapters remaining explicit reviewed exceptions until their mobile implementations are introduced. Kotlin and Swift adapters must not write the ostudy SQLite database directly.

An internal Tauri plugin may bridge native mobile APIs, but its public Rust surface implements the same ports used by desktop adapters. The plugin is transport infrastructure, not a second application service layer.

## Consequences

- Linux behavior can be moved behind the ports before another platform exists.
- Android and iOS implementations can replace only the OS edge.
- Tests can use deterministic fake clocks, resource snapshots, lifecycle events, file selections, and secret stores.
- Unsupported capabilities are represented explicitly rather than inferred from an OS name.
- The composition root becomes the only place that selects concrete platform implementations.

## Non-goals

This decision does not create Android or iOS projects, implement a mobile PDF engine, add synchronization, or split the whole Rust application into many crates. Stable modules may be extracted into workspace crates later, after the boundaries have proven useful.

## Validation

Repository policy checks reject new direct platform dependencies outside approved adapters. Contract tests exercise every port through fake and desktop implementations. Existing Linux behavior must remain qualified throughout the migration.
