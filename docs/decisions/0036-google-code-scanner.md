# ADR 0036: Google Code Scanner for Android pairing

- Status: **Superseded by ADR 0052**

## Historical decision

Android QR pairing previously used Google Code Scanner (`play-services-code-scanner:16.1.0`) through the private oStudy platform plugin. That removed application camera permission and delegated camera UI/decoding to Google Play services while Rust retained pairing-envelope parsing and cryptographic validation.

PR #101 introduced a stricter dependency requirement for application-controlled non-AI functionality: it must use free/open-source software and remain locally executable. Google Code Scanner no longer satisfies that accepted release contract, even though the scanner ran on-device.

The current Android implementation and rationale are defined by [ADR 0052](0052-open-source-android-imaging.md): JourneyApps ZXing Embedded with ZXing core replaces Google Code Scanner, camera permission is explicitly application-owned, QR decoding remains local, and the existing bounded provider-neutral Rust/frontend scanner contract is unchanged.

This ADR is retained only as implementation history.
