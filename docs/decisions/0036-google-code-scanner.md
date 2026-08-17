# ADR 0036: Google Code Scanner for Android pairing

## Decision

Android QR pairing uses Google Code Scanner (`play-services-code-scanner:16.1.0`) through the private ostudy platform plugin. The scanner is restricted to QR codes, enables first-party auto zoom, requests the `barcode_ui` module at install time, and delegates camera permission/UI to Google Play services. ZXing Embedded and the application camera permission are removed.

The native scanner returns only a bounded decoded string. Rust retains pairing-envelope parsing, expiry/protocol checks, and cryptographic validation. Raw QR contents never enter logs. Atomic guards reject concurrent scans and ignore duplicate terminal callbacks.

## Consequences

Google Play services is required for this Android backend. Unsupported devices receive a typed scanner failure rather than a silent stale-library fallback. The Rust/frontend scanner contract remains provider-neutral so a bundled non-GMS backend can be added later without redesigning device sync.
