# AI infrastructure testing

This guide covers focused validation for changes under:

```text
src-tauri/src/ai/gateway/
src-tauri/src/ai/profiles/
src-tauri/src/ai/probes/
```

The maintained architecture is documented in
[Native AI infrastructure](../architecture/ai-infrastructure.md). These tests use
local fixtures and local mock servers; ordinary pull-request qualification never
requires a real credential or internet provider.

## Static ownership gate

Run:

```bash
node ./scripts/verify-ai-infrastructure-boundaries.mjs
python3 -m unittest tests.scripts.test_ai_infrastructure_boundaries
```

The verifier rejects:

- retired root `gateway.rs`, `profiles.rs`, or `probes.rs` monoliths;
- missing package modules or path-attributed aliases;
- a production module above the AI infrastructure size tripwire;
- `reqwest` or HTTP-client construction outside its gateway owner;
- SQLite, Tauri, profile, or document-domain dependencies in the gateway;
- Tauri commands outside profile/probe command adapters;
- profile/default/probe/capability SQL outside its repository owner;
- direct AI-table reads from document context or planning; and
- document transport that does not select the explicit document gateway mode.

Fixture SQL is test evidence, not a production owner, and is intentionally
excluded from production dependency scans.

## Focused Rust tests

Run all native AI tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml ai::
```

Useful narrower filters include:

```bash
cargo test --manifest-path src-tauri/Cargo.toml ai::gateway::tests
cargo test --manifest-path src-tauri/Cargo.toml ai::profiles::tests
cargo test --manifest-path src-tauri/Cargo.toml ai::probes::tests
cargo test --manifest-path src-tauri/Cargo.toml ai::provider_contract_tests
cargo test --manifest-path src-tauri/Cargo.toml ai::backup_tests
cargo test --manifest-path src-tauri/Cargo.toml ai::document_intelligence
```

### Gateway expectations

Gateway tests cover:

- arbitrary SSE fragmentation and CRLF/LF handling;
- keepalive versus model-output accounting;
- reasoning progress without retaining reasoning content;
- streamed content and strict-tool arguments;
- in-band errors after partial output;
- content and wire bounds;
- OpenAI and llama.cpp usage normalization;
- safe OpenRouter metadata and raw-message redaction;
- redirect and HTTP status classification; and
- the exact transient retry allowlist and bounded delay.

A new provider envelope must add deterministic parser/redaction tests before it
is accepted.

### Profile expectations

Profile tests cover:

- serde names and unknown-field rejection;
- provider/auth/privacy policy;
- endpoint-origin and auth trust-boundary changes;
- normalization and bounds;
- optimistic revisions and operation replay;
- profile capacity and active-name uniqueness;
- credential epoch/storage metadata;
- configuration projections;
- task-default idempotency; and
- secret absence from DTOs, SQLite, and backups.

Do not use a real OS credential store in deterministic service tests. Session
persistence or a reviewed fake platform store is sufficient.

### Probe expectations

Probe tests cover:

- model-match policy;
- required and optional phase construction;
- provider-specific capability inference;
- profile/model/credential freshness;
- atomic result/capability replacement;
- typed document evidence readers; and
- persisted-result bounds and redaction.

`provider_contract_tests.rs` supplies local HTTP fixtures for complete adapter and
gateway exchanges without accessing the internet.

## Frontend compatibility

The Rust refactor intentionally keeps the AI IPC contract unchanged. Run:

```bash
pnpm exec vitest run \
  src/ipc/aiProfiles.test.ts \
  src/controllers/aiProfilesController.test.tsx \
  src/screens/settings/AiLlmSettings.test.tsx \
  src/screens/SettingsScreen.test.tsx \
  src/screens/SettingsScreen.extended.test.tsx
```

Any required production change to `src/ipc/aiProfiles.ts` should be treated as a
contract change and reviewed separately rather than propagated as a mechanical
module move.

## Repository gates

Before publication, run:

```bash
./scripts/check-static.sh
./scripts/check-fast.sh
```

Before readiness, run the authoritative matrix:

```bash
./scripts/check.sh
./scripts/android-test.sh
./scripts/android-emulator-smoke.sh --artifacts artifacts/android-smoke
```

The Android smoke suite is not provider-live qualification. It verifies native
startup, packaging, platform routing, persistence, and existing application
workflows on the API-36 emulator.

## Manual acceptance

A structural AI-infrastructure PR should still exercise, with non-sensitive test
profiles:

1. create, update, enable/disable, and delete a local profile;
2. set and clear a session credential on a credential-requiring profile;
3. refresh models and run provider verification against a local fixture or
   intentionally configured test server;
4. confirm stale evidence after model/profile/credential changes;
5. start and cancel a document task;
6. generate a document artifact through the explicit streaming path; and
7. confirm that existing Xberg document evidence, study guides, flashcards,
   schedules, and backup/restore behavior remain unchanged.

Live cloud-provider acceptance is opt-in and must use a user-controlled test
credential. It is not a substitute for deterministic provider-contract tests.
