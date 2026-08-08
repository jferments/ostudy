# ostudy architecture

## System summary

ostudy is a local-first Tauri 2 desktop/mobile application. React and TypeScript
render the interface; Rust owns durable domain behavior; SQLite stores structured
state; and the application-managed filesystem stores managed content bytes.
Optional AI features connect to an existing local model server or an explicitly
configured cloud API; ostudy does not install or run inference infrastructure.

```text
React feature views
        ↓
frontend controllers
        ↓
typed domain IPC gateways
        ↓
Tauri command boundary
        ↓
Rust domain/services
        ↓
SQLite + managed assets + native platform services
```

Platform capabilities such as file selection, external URL opening, hardware
Back, future audio, and future haptics are isolated under `src/platform/` rather
than being mixed into IPC or feature screens. Provider HTTP and credential
storage are separate native AI boundaries and are not exposed as frontend
platform adapters.

## Frontend layers

### Typed IPC gateways

All direct `invoke()` calls live under `src/ipc/`, split by domain. Command-name
parity with the Rust `generate_handler!` registry is mechanically verified.
Screens and controllers import only the IPC domains they use. AI IPC contains
profile administration, model discovery, and bounded profile verification; it
does not expose a generic prompt/completion command.

### Controllers

Workflow-heavy features use controllers under `src/controllers/` for:

- native reads and mutations;
- stale-read request lanes;
- mutation operation/retry identity;
- committed-success versus failed-reconciliation handling;
- feature transitions and launch requests.

Obsolete read results may finish natively but may not overwrite newer UI state.
Durable mutations are not treated as cancellable reads: if a write committed,
subsequent view-refresh failure is reported separately rather than inviting a
second write.

Settings composes independent ordinary-preference, backup, and AI-profile
controllers. Saving appearance or scheduling preferences cannot clear an
unrelated AI metadata or credential-entry draft.

### Source notes and study presentations

Editable source transport is `CardNote`. Search, scheduling, and Practice use
rendered `CardPresentation` identities. A Basic note may own forward and reverse
presentations; a Cloze note owns one stable scheduled presentation whose hidden
group rotates according to review history.

### UI platform

Shared interaction and visual infrastructure lives under `src/ui/`; detailed
rules are in `docs/UI-ARCHITECTURE.md` and ADR 0014.

- `OverlayProvider` owns the dialog/sheet/menu stack, topmost Escape/platform
  Back behavior, focus entry/containment/restoration, background inertness, and
  scroll locking.
- Feature code uses shared overlay primitives or asynchronous confirmation. It
  does not hand-write dialog semantics or ordinary `window.confirm()` calls.
- Shared Tabs own tablist/tabpanel semantics, roving focus, arrow keys, Home/End,
  compact scrolling, and stable accessible relationships.
- Only `src/ui/foundations/index.css` is imported globally. New UI styling uses
  CSS Modules and semantic tokens.
- Wide, medium, and compact layout modes share the same controllers. Compact
  layouts use bottom navigation, safe-area-aware overlays, dynamic viewport
  units, and coarse-pointer touch targets.
- Shared UI is independent of IPC, controllers, and Tauri APIs.
- Semantic feedback events provide a future seam for visual effects, local audio,
  and haptics without coupling them to durable study operations.

### Platform adapters

`src/platform/` owns operating-system/WebView capability differences:

- file open/save selection;
- external links;
- platform Back dispatch;
- pointer/hover/capability detection;
- optional audio and haptic feedback adapters.

A future Android adapter may stage content URIs into application cache without
changing Decks, Settings, Card Editor, or import workflows.

## Native composition root

`src-tauri/src/lib.rs` is the Tauri composition root. Startup:

1. resolves application paths;
2. applies a validated pending whole-library restore if present;
3. opens/migrates SQLite;
4. runs any missing versioned data repair exactly once;
5. initializes the Rust AI runtime without contacting a provider;
6. finalizes expired deletion recovery windows;
7. purges finalized deletion-owned history;
8. reconciles managed assets;
9. publishes application paths, database state, and native AI state;
10. registers the typed command surface.

A durable startup failure stops application initialization rather than exposing a
partially reconciled library. A temporarily unavailable operating-system
credential store does not prevent non-AI application startup; affected credential
operations fail explicitly or allow an intentional session-only credential.

## Rust domain modules

### Authoring

`src-tauri/src/authoring/` uses ordinary Rust modules with explicit visibility.
It owns:

- source-note/card presentation creation and update;
- tags, duplicate candidates, and Search/Filter authoring projections;
- Basic, Reverse, and Cloze preparation;
- reusable template graph validation/persistence/rendering;
- generated JSON import schema, restricted Markdown, whole-file preparation, and
  transactional apply;
- move/copy transfer operations.

Command/service orchestrators own logical transactions. Subordinate helpers take
`&Connection` or `&Transaction` and do not commit secretly. The authoring
subsystem remains network-free; future AI generation must enter through a native
task service and reuse authoring validation rather than giving a provider direct
write authority.

### Studyability and local calendar

`studyability` is the single authority for archive, manual suspension, burial,
and due readiness. Due timestamps never override archive/suspension state.

`study_calendar` owns local-date windows, DST transitions, leap-day clamping,
custom local-date resolution, and time-zone identifier validation. Product
language that says “day”, “tomorrow”, “week”, or “year” uses local-calendar
semantics rather than fixed-duration browser arithmetic.

### Projections

Bounded read paths live in projection-oriented modules. Summary/detail/window
DTOs are separate contracts. List operations do not hydrate complete rich
content or issue one asset query per row when a bounded projection is available.

### Scheduling and Practice

`src-tauri/src/scheduler.rs` is the adapter around `rs-fsrs`.

- `card_schedules` stores mutable current schedule state.
- `review_events` is the append-only historical review ledger.
- review submission validates the previewed schedule revision and commits the
  review event plus schedule update transactionally;
- submission and operation IDs make ambiguous retries idempotent;
- bulk schedule actions validate and commit all requested cards atomically;
- Practice sessions persist a bounded identity snapshot and return bounded card
  windows rather than hydrating the whole session.

### Analytics

Analytics are deterministic projections of stored practice/review data. Current
inventory and historical evidence remain separate domains.

Historical attribution follows stable entity identities:

- moving a card changes current ownership but retained review evidence follows
  the current card owner while preserving the reviewed presentation;
- renamed decks/tags display their current names;
- archived decks remain in historical analytics;
- finalized deleted-deck history is purged according to the deletion contract.

### AI provider platform

`src-tauri/src/ai/` owns provider metadata, credential isolation, endpoint policy,
native HTTP transport, provider adapters, capability probes, and redacted probe
persistence. Detailed decisions are in `docs/AI-ARCHITECTURE.md`, ADR 0015, and
ADR 0016.

Provider profiles are separate from `AppSettings`. SQLite stores stable profile
IDs, immutable provider kinds, normalized endpoints, model IDs, auth modes,
enabled state, timeouts, credential epochs, optimistic revisions, and explicit
task defaults. Credential bytes never enter SQLite.

`SecretManager` is the sole credential boundary. Secure credentials use the
operating-system credential service; explicit session-only credentials remain in
native process memory. Read DTOs expose status only. Changing a credential or
profile invalidates older capability evidence. Metadata and credential mutations
share one native mutation lane so an endpoint cannot change concurrently with a
secret write.

`AiGateway` is the sole provider HTTP transport. It uses bounded native requests,
Rustls TLS, disabled redirects, no cookies, no raw provider-body logging, and
safe error normalization. Provider adapters own route and structured-output
differences. Initial adapters support llama.cpp and DeepSeek:

- llama.cpp applies the server/model chat template; ostudy stores only presence
  and an optional SHA-256 fingerprint;
- llama.cpp structured output uses JSON Schema plus independent local semantic
  validation;
- DeepSeek structured output uses JSON object mode plus strict Rust and semantic
  validation;
- deterministic loopback fixtures verify request/response contracts without
  internet access or paid credentials.

Capabilities distinguish supported, unsupported, and unknown states and whether
evidence was declared or actively probed. Probe persistence is conditional on the
profile revision, credential epoch, and model still matching the tested snapshot.
There is no generic prompt IPC and no automatic local-to-cloud fallback.

### Imports, exports, and backup

`.studydeck` is the content-sharing format. Imported decks receive fresh local
schedules and do not import another learner's personal review history. Legacy
released package layouts are adapted before canonical persistence.

Generated `.ostudy.json` files are strict, versioned, data-only external input.
The importer validates the entire file before one transaction and automatically
skips exact duplicates. Its executable conformance corpus covers supported and
rejected syntax, declared limits, duplicates, create/append resolution,
atomicity, fresh schedules, and cleanup in isolated temporary storage.

`.ostudybackup` is the complete personal-library backup format and includes
content, templates, managed images, schedules, review events, sessions,
analytics-source records, repairs, ordinary settings, AI provider metadata, task
defaults, and redacted capability evidence. API credentials and session-only
secrets are excluded because they never enter the SQLite snapshot.

## Data model

The major ownership relationships are:

```text
deck
  └── note (editable source)
       ├── card presentation(s)
       │    └── card schedule
       ├── tags
       └── managed asset references

practice session
  └── stable session-card snapshot

review event
  └── immutable reviewed card/presentation identity

AI task default
  └── provider profile metadata
       ├── credential status (native secret store; no bytes in SQLite)
       └── redacted capability/probe evidence
```

Cards are scheduled, buried, suspended, reviewed, and selected. Notes own
editable content and tags. This distinction must remain visible in DTO names and
frontend workflows. AI provider profiles do not own or mutate cards; future task
services must cross ordinary authoring validation and transactions.

## Migrations and repairs

Released SQL migrations and release fixtures are immutable evidence. New schema
changes append a migration and checksum registration.

Versioned data repairs are separate from schema migrations. A repair:

- has a stable ID, version, and checksum;
- runs once inside a transaction;
- is recorded in the repair ledger;
- makes normal startup read-only with respect to already-canonical application
  data.

Historical compatibility names/checksums remain stable even after active source
modules receive semantic names.

## Managed assets

SQLite owns asset metadata and references; the managed filesystem owns bytes.
Draft IDs make image selection safe before the owning note/template commits.
Startup reconciliation handles interrupted rename/copy/delete windows and
protects shared references. Diagnostic export excludes managed image bytes and
the SQLite library.

## Untrusted content and template safety

All card, template, imported-deck, provider, and future document content is
untrusted until Rust validates it.

- Cards are structured JSON, never executable HTML or script source.
- Node types, marks, attributes, nesting, sizes, colors, and image references use
  explicit allowlists and bounds.
- Links are limited to complete `http`, `https`, `mailto`, and `tel` addresses.
- Images must reference managed local assets.
- Cloze marker grammar, group numbers, nesting, and source size are validated.
- Template field names, values, rules, placeholders, stable IDs, and output sizes
  are bounded and validated.
- Template rendering performs structural literal substitution; it has no
  evaluator, JavaScript, SQL, shell, raw HTML, filesystem, network, or general
  expression capability.
- Search text is derived by Rust and SQLite statements use bound parameters.
- Unknown structured content, unknown fields, or malformed placeholders fail
  visibly.
- Provider output is parsed into strict Rust types and semantically checked; HTTP
  success never grants database or authoring authority.
- Provider redirects are rejected and public cleartext HTTP is forbidden.
- Raw provider prompts, responses, reasoning content, headers, and credentials
  are not persisted by the profile verification path.

## Quality boundaries

The static gate enforces durable contracts rather than historical source shape:

- exact dependency/toolchain pins and lockfiles;
- migration registration/checksums;
- frontend/Rust IPC parity;
- explicit Rust module policy;
- no skipped/focused/ignored tests;
- immutable self-hosted workflows;
- UI/platform dependency direction;
- shared overlay and tab ownership;
- one global style entry point and removal of historical stage-named CSS paths;
- no frontend provider networking or provider SDKs;
- provider HTTP confined to the native AI gateway;
- operating-system keyring access confined to the AI secret boundary;
- no credential-value columns in SQLite migrations;
- no generic AI completion command.

The self-hosted workflow may upload exact candidate Cargo lock and rustfmt
artifacts, but qualification still fails until both are committed. Feature
behavior belongs in Rust tests, deterministic provider contract fixtures,
Vitest/controller/component tests, migration/package/conformance fixtures, and
native WebDriver workflows.
