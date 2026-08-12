# ADR 0028: Secret storage and device-key isolation

- **Status:** Accepted
- **Date:** 2026-08-11
- **Scope:** Cross-platform preparation phases P0–P2

## Context

AI credentials are correctly excluded from SQLite and backups, but the shared AI secret manager directly instantiates the Rust keyring implementation. Future Android Keystore, Apple Keychain, and synchronization device keys need a stable application boundary and clearer error semantics.

## Decision

`SecureSecretStore` is the operating-system persistence port. It supports read, write, delete, and availability/status operations over a typed `SecretLocator`.

The AI layer owns an `AiCredentialVault` above that port. The vault retains current product policy:

- credentials are write-only from the UI;
- secure and session-only persistence remain distinct;
- session values live only in process memory;
- temporary byte/string copies are cleared best-effort;
- credential status is safe metadata, never a secret readback.

Store failures are normalized as unavailable, locked, permission denied, invalidated, or failed. Product services map those categories to stable command errors.

Future synchronization identity uses a separate `DeviceKeyStore`. AI credentials, relay tokens, and device cryptographic keys use separate namespaces and access policies. None may enter SQLite content tables, authoring drafts, library backups, deck packages, generated imports, diagnostics, or ordinary sync payloads.

Desktop uses the current keyring behavior through an adapter. Android and Apple adapters will use their native protected stores.

## Consequences

AI profile, probing, and document-generation services remain platform-neutral. Secure-store availability can be represented accurately on every target, and future device keys do not become an accidental extension of AI credential handling.

## Non-goals

This decision does not implement end-to-end synchronization cryptography or account recovery.

## Validation

Shared vault tests use an in-memory fake store. Desktop adapter tests cover locator mapping and error classification without modifying a developer's real credential store during ordinary unit tests.
