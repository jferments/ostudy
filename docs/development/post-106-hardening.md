# Post-#106 hardening and audit disposition

Status: **H-01/H-02 are merged in PR #109; H-03/H-04/H-05 are merged in PR #110; H-06/H-07 are merged in PR #111; H-08/H-09 are merged in PR #112; H-10 is implemented in draft PR #113 and awaiting terminal qualification/human acceptance.** This register preserves the September 26, 2026 documentation, native and frontend review decisions. The external reviews used `f28abfd5`; the follow-up source reviews used `7f3c35d57325fbf371ff254c7f2e3ec3c02133fb` and `d1d3b176a2542e7f1b89408c8a6bf88b7d84b36b`. Recheck the relevant owner at the implementation branch's exact head before changing it. Review counts and hypotheses are not executed-test evidence.

Merged foundation: PR #106, squash `9ef91e79346b6d0d0ace983da3c1583303f15a43`. See the [completion record](lk-followups/consolidation-qualification.md). PR #108 is documentation-only relative to that foundation. The H identifiers below are scope labels, **not reserved GitHub PR numbers**, and do not authorize starting all scopes at once. PR #105 remains the deferred lessons/narration/PowerPoint plan; structural cleanup must not become an endless prerequisite to product work.

## Delivery rules

PR #108 and PR #109 are merged. PR #110 owns H-03, H-04 and H-05 together at the human's explicit request; do not split or duplicate those changes while that PR is active. After #110, H-06 is the next planned hardening boundary. H-05's hook/indexed-access safeguards are established before broader frontend restructuring; preserve the H-02/H-06 lifetime contracts before H-08, and retain H-04's filesystem/staging contract before H-09. Dependencies describe semantics, not permission for competing writers or duplicate full matrices.

Each implementation PR owns its focused regressions, source-derived documentation, compatibility decisions and exact-SHA qualification. Freeze a complete candidate before terminal gates; record fresh versus safely reused execution and unrun human/platform boundaries separately. Use the [GitHub-direct procedure](../agents/GITHUB-DIRECT-DEVELOPMENT.md). No automatic merge, source-writing Actions, alternate publisher, forced history rewrite, new service, or live-library experiment is introduced by this plan.

## Core PR scopes and acceptance boundaries

### H-01 — atomic, retry-safe save-selection-as-deck

Status: **implemented and merged in PR #109.**

Owner: `src/screens/cards/PracticeResultsModal.tsx`, `authoring/selection_deck.rs`, deck copy primitives and the release migration registry.

The frontend now sends one stable operation identity to a native transaction that creates the destination deck, copies the selected notes/questions/assets/tags, creates fresh schedules, and stores a replayable committed result. Release migration 2 adds only the operation-receipt table; the schema-1 baseline remains immutable and supported as an upgrade/backup input. React no longer deletes a committed result when its follow-up deck-list refresh fails.

Focused regressions cover rollback on copy failure, replay without duplicate creation, identity-conflict refusal, preservation of source scheduling evidence, fresh copied schedules, typed interactive question copying, post-commit refresh failure, and same-operation retry after a lost response.

Exit: failed copying leaves no partially created result; lost reply replays the same deck; reused identity with different input is rejected; refresh/navigation failure cannot delete or recreate committed content. H-02 is implemented in the same requested PR; H-03 follows after #109 rather than a broader controller/type-generation refactor.

### H-02 — activity admission, lifecycle visibility and restore preparation

Status: **implemented and merged in PR #109.**

Owners: `study_materials/cancellation.rs`, `task_activity.rs`, `library_session.rs`, `library_backup/live.rs` / `activation.rs`, and native close policy.

Collection study-material work now acquires the shared ActivityGuard for the same owned lifetime as its feature-local cancellation guard. Maintenance admission and close protection therefore see collection work consistently, while the feature registry still owns cancellation/progress. Request leases continue to protect actual command completion independently of activity admission.

Restore now resolves, copies, extracts, upgrades and validates a private candidate before closing ordinary request admission. Before exclusive activation it releases its preparation lease, acquires maintenance/exclusive admission, rechecks the original library generation, and verifies the retained source/candidate identity before journaled installation. Invalid preparation cannot replace the live library, and an abandoned prepared candidate has no startup authority.

Focused regressions cover activity reservation/drop behavior, collection cancellation cleanup, maintenance refusal while activity exists, request-generation races, private candidate validation/identity checks, and the existing crash-recovery/receipt paths. No timeout authorizes replacement while an old request still owns the library.

### H-03 — observable frontend failures and asynchronous lifetimes

Status: **implemented in PR #110.**

Owners: `src/platform/PlatformProvider.tsx`, startup/share listeners, `src/ipc/diagnostics.ts`, `ImageSetWorkspace.tsx`, `DocumentProgress.tsx`.

PR #110 routes identified caught platform/startup/share failures through a bounded, privacy-safe diagnostic queue instead of console-only paths, while listener owners reject late callbacks after disposal. Clipboard fallback timers and unresolved reads are invalidated when their owner changes, paste-event/fallback ordering is de-duplicated, and incoming-share reconciliation retains current items without canceling already-admitted document work.

Generation progress keeps Stop, elapsed time, request/retry state and input/output/reasoning/cache usage visible. Expanded retry diagnostics move behind an accessible details disclosure; unknown provider usage remains distinct from a reported zero.

Exit: late listeners/clipboard results cannot act on obsolete owners; one gesture cannot add twice; unavailable diagnostics cannot recurse or grow an unbounded queue; post-navigation work continues legitimately.

### H-04 — portable filesystem durability and recovery evidence

Status: **implemented in PR #110, with Windows runtime execution still a separate platform-qualification boundary.**

Owners: document/checkpoint/backup/asset filesystem helpers and `studydeck/import.rs` compensation.

PR #110 centralizes shared durable file publication/directory-sync mechanics behind explicit contracts, uses platform-specific directory handling rather than assuming Unix semantics, and makes atomic replacement preserve recoverable old-or-new visibility. Studydeck import compensation preserves the primary import failure, continues bounded recovery after a secondary failure, retains recovery inputs instead of overwriting/deleting conflicting evidence, and records only bounded recovery facts. The Windows implementation remains source/API-reviewed rather than Windows-runtime-qualified.

Exit: publication/rename/flush fault injection, retained recovery material and coherent old-or-new visibility. Windows behavior needs Windows execution evidence; Linux or Android success does not qualify it. No learner-data, recovery-directory or runner-cache purge.

### H-05 — frontend correctness gates

Status: **implemented in PR #110.**

Static qualification now enforces React hook correctness (`rules-of-hooks` and `exhaustive-deps`) and TypeScript `noUncheckedIndexedAccess` against production source and tests. ESLint/parser/plugin versions are pinned in the normal JavaScript dependency contract. The implementation fixes the actual ownership/dependency/indexing findings rather than disabling rules or excluding tests; test fixtures use explicit runtime presence checks where they intentionally index known fixture values. React Compiler is not introduced.

Exit: checks are enforced against the current source; lifecycle and stale-read behavior remains covered. The cleanup verifier's historical #106 activation-text assertion should eventually be replaced with a maintained status contract, not erased to conceal a failure. PR #108 retains that phrase only as an explicitly historical quotation.

### H-06 — explicit library-aware IPC transport

Status: **implemented in PR #111.**

Owners: `src/ipc/transport.ts`, `libraryTransport.ts`, domain gateways and Vite/Vitest wiring.

PR #111 removes the private Vite/TypeScript interception of `@tauri-apps/api/core`. Every domain command gateway imports the application-owned `src/ipc/transport.ts` explicitly; that transport is the sole raw native-command caller and preserves the one-generation-per-page/stale-mutation refusal policy. Non-command SDK utilities remain direct at their narrow platform owners. Repository verification rejects raw gateway invokes and reintroduction of the retired alias, and integration coverage exercises a real gateway through the production transport boundary.

Exit: bootstrap/concurrent requests, plugin passthrough, startup diagnostics and post-restore stale writes use the same path as production; no mutation is replayed automatically with a refreshed generation; raw invocation bypass is rejected.

### H-07 — native-owned wire types and typed command declarations

Status: **implemented in PR #111.**

PR #111 uses a deliberately narrow native-owned exporter rather than adopting a broad IPC framework. Rust owns representative wire descriptors/fixtures for stable cross-boundary shapes and serialization conventions, including camelCase fields, nullable values, tagged unions, numbers and byte-array lowering. Checked-in TypeScript declarations and JSON fixtures are compared against Rust-rendered output in Static/Fast qualification.

The registered Rust command set separately owns command membership. A checked-in generated `NativeCommandGatewayMap` maps every registered command to its exported typed frontend gateway and derives request/result types with TypeScript `Parameters`/`ReturnType`; it is a drift detector/inventory, not a replacement for native validation. Generated declarations contain no invoke implementation and therefore cannot bypass H-06. Native leased wrappers continue to deserialize/validate typed domain input and preserve original-generation lifetime; execution placement remains outside H-07.

Exit: native wire changes update the checked-in contract or fail check-only verification; serialization fixtures agree across the boundary. Actions may compare temporary generator output but never rewrite or publish maintained source.

### H-08 — bounded nonblocking native command execution

Status: **implemented in PR #112.**

Owners: native command adapters, database owner, request/activity leases.

PR #112 routes synchronous native command dispatch through a bounded FIFO ordinary worker rather than running substantial SQL/filesystem work on the native event thread. A separate bounded control lane keeps library-session observation, cancellation and platform Back/control operations responsive when ordinary work is blocked. Queue admission is nonblocking and rejects explicitly when full; it does not grow a thread-per-command backlog.

The queued closure owns the original library-generation lease until deserialization, domain work, serialization and reply are finished. A lost reply or dropped producer therefore cannot make restore replace the library under accepted work, and a worker panic releases the lease without clearing an independently poisoned database. Queue wait, database-connection wait and database execution durations are recorded as content-free diagnostics after the database mutex is released.

Focused regressions cover FIFO/bounds, event-thread separation, blocked database work with independent control progress, restore drainage after a lost reply, queue rejection cleanup, worker panic/poison behavior and the maintained transaction/replay/session contracts.

Exit: a blocked database operation does not block unrelated control interaction; transaction/replay/restore behavior is unchanged. A read-only connection pool remains a separate measurement-driven proposal and was not introduced.

### H-09 — bounded startup validation and conversion staging

Status: **implemented in PR #112.**

Owners: `database.rs`, `database/release.rs`, `payload_entry.rs`, restore admission and private staging.

PR #112 keeps present-byte admission on every open, backup/import and restore boundary, but removes repeated current-payload scans within one already-admitted current-release initialization. The fact is process-local to that initialization call; no durable validation stamp can authenticate later bytes. Corrupt data written after a prior successful open is rejected on the next open and by read-only backup admission.

The one-time public-v1-to-release converter no longer copies the whole SQLite database into an in-memory connection. It creates a private disk-backed candidate beside the source database (or in the OS temporary directory for an in-memory test source), uses file-backed SQLite temporary storage, disables mmap for the candidate and bounds its page cache. SQLite backup still copies bounded page batches. Only after the candidate passes the existing schema, integrity and typed non-ledger preservation proof can the live lineage transaction begin. Dropping an abandoned candidate removes its database and sidecars; `SQLITE_FULL` becomes an actionable low-space error while the live legacy library remains unchanged.

Focused regressions cover one validation pass per current open, later corrupt-payload refusal, private candidate permissions/cleanup, low-space failure, candidate abandonment before live mutation, schema-30 typed preservation and sibling managed-file preservation. Fast qualification also emits an advisory Linux measurement for legacy conversion/current reopen elapsed time and process peak RSS; those observations are diagnostic evidence, not correctness thresholds.

Exit: current reopen, unsupported/corrupt input refusal, historical compatibility conversion/restore ownership, interrupted/low-space staging and typed row/file preservation are covered without a permanent validation stamp.

### H-10 — obsolete bundled interfaces, not supported data readers

Status: **implemented in draft PR #113; terminal qualification and human acceptance remain.**

Owners: Dialog callers, current settings/appearance interfaces, live routing, fixture-only props and profile DTO consumers.

PR #113 removes the unused `bindPreview`/`onManageTemplates` props, fixture-only global `AppSettings.aiReasoningEffort`, and frontend `animationsEnabled`/`reduceMotion` compatibility fields. Current AI reasoning remains owned by profile/task assignments; historical motion rows normalize to `motionPreference` in the native settings reader before crossing IPC; released migrations/readers are unchanged. Card workflows now use the shared Dialog owner directly and the compatibility `Modal` facade is gone. Focus restoration, Escape/native Back and busy/non-dismissible mutation behavior have focused regressions.

The live React route vocabulary now contains only current application sections and the live Flashcards workspace accepts only Decks/Search. Historical checkpoint/workspace strings remain accepted at their dedicated compatibility readers/route constructor and normalize to current destinations before reaching live UI state. Templates remain a Settings tab, so its dirty state participates in the Settings owner rather than a second top-level route guard.

Exit evidence is owned by the PR's current-route/checkpoint/settings/Dialog regressions and terminal platform qualification. Supported legacy checkpoint readers, restored settings, practice return targets and durable migrations are retained. Stored-description rewrites or source-identity normalization remain separate data-preservation designs.

## Conditional and separate work

| Candidate | Disposition and required evidence |
| --- | --- |
| Read-only database connection pool | Consider after H-08 measurements; needs generation-fenced connection drainage, snapshot consistency and single-writer policy. |
| Platform global silently falling back after poison | Accepted failure-policy concern; repair at the platform owner with explicit failure tests, not generic database poison recovery. |
| ID generation coupled to SQLite | Low-priority coupling; callers often already hold a transaction. Measure before claiming one extra global lock per ID. Preserve identifier format. |
| `asset_lifecycle` include fragments | One behavior-preserving native-owner refactor, with staging/promotion/reconciliation tests; not arbitrary line-count splitting. |
| Large hooks/screens and root document modules | Refactor one demonstrated responsibility at a time. Size and filenames alone are not defects; no replacement global state bag. |
| device_sync rustfmt exemption | Isolated mechanically reviewed formatting change; do not hide functional changes or touch a frozen candidate. |
| Stage-named tests and stateless SVG index keys | Minor cleanup only where it improves ownership/identity; no durable IDs or schema changes just for SVG keys. |
| Sync schema metadata and duplicate version constants | Clarify wire format versus database lineage/version before changing stamps; do not globally substitute 1 for 30. |
| Future release repairs/migrations | Extend the release registry at the first real forward change, with immutable identities/checksums and admission tests; current rejection of unknown repair rows is not itself a defect. |
| Old settings, description suffixes, source identity defaults | Normalize only with proven eligibility and preservation of learner edits and restored historical input. Current source identity serialization can omit v1, so removing its decoder default alone is unsafe. |
| Sync-v2, checkpoint-v1 and restore receipt readers | Retire only after tracing supported inputs and testing a replacement. Different package/database versions are separate contracts. |
| Unstamped generation-zero requests | Tighten only after proving bundled startup and native test requirements; a post-replacement unstamped request is already stale. |
| ADR number collisions | Use the [decision index](../decisions/README.md) and full filename; historical links remain. Any renumbering is a separate coordinated link change. |
| ostudy-io documentation/PR drift | Separate repository task; verify its own head. No website or VM edits belong here. |

## Findings not accepted as proposed

- **Blind database mutex recovery:** rejected. A panic can interrupt connection replacement or filesystem coordination, not just an SQL transaction. Reopening admission requires proven coherence; clearing poison alone is not recovery.
- **Validate once forever:** rejected. A historical stamp does not authenticate present data. H-09 optimizes redundancy without discarding admission.
- **Delete allegedly write-only PDF tables:** rejected as a dead-code claim. `document_semantic_elements` and `document_semantic_chunks` are in the production device-sync table set and generic merge reads. Any removal needs coordinated storage/sync/backup preservation.
- **Drop readers because the product was unreleased:** rejected as a general rule. Real retained libraries, backups and external formats remain supported inputs.
- **Old stage36_38 schema-30 assertion:** already corrected before merge; do not reopen it based on the older audit.

## Documentation-only scope and preserved earlier work

PR #108 aligns maintained prose, records #106's completed boundary and keeps this plan reachable. It does not claim to fix H-01 through H-10. Its original commit `b09d0abec747a54eb043cc48c3284e88678a247b` contains the small Tutor/native collection-label changes, a focused label test and a workflow-comment correction. The non-force integration commit preserves that history but defers those non-Markdown edits so the documentation proposal has no runtime/workflow diff from merged #106. Reapply only the still-relevant narrow changes in their owning later scope; do not cherry-pick the whole old mixed commit.

The current product term is document collection; Tutor Sources still displays the legacy label Document folder. The guide records that wording honestly until the implementation changes. All workflows are source-read-only despite the stale Check comment; the executable maintenance workflow and agent policy are authoritative.
