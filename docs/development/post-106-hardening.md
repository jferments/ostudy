# Post-#106 hardening and audit disposition

Status: **H-01/H-02 implemented in PR #109; qualification pending on its exact head. H-03 onward remain planned follow-up.** This register preserves the September 26, 2026 documentation, native and frontend review decisions. The external reviews used `f28abfd5`; the follow-up source reviews used `7f3c35d57325fbf371ff254c7f2e3ec3c02133fb` and `d1d3b176a2542e7f1b89408c8a6bf88b7d84b36b`. Recheck the relevant owner at the implementation branch's exact head before changing it. Review counts and hypotheses are not executed-test evidence.

Merged foundation: PR #106, squash `9ef91e79346b6d0d0ace983da3c1583303f15a43`. See the [completion record](lk-followups/consolidation-qualification.md). PR #108 is documentation-only relative to that foundation. The H identifiers below are scope labels, **not reserved GitHub PR numbers**, and do not authorize starting all scopes at once. PR #105 remains the deferred lessons/narration/PowerPoint plan; structural cleanup must not become an endless prerequisite to product work.

## Delivery rules

PR #108 is merged. PR #109 owns H-01 and H-02 together at the human's explicit request; do not split or duplicate those active changes. After #109 merges, H-03 is the next planned hardening boundary. Establish the H-05/H-06 safeguards before broad frontend restructuring, preserve the H-02/H-06 lifetime contracts before H-08, and require a proven filesystem/staging contract before H-09. H-04 can be independent of frontend work; dependencies describe semantics, not permission for competing writers or duplicate full matrices.

Each implementation PR owns its focused regressions, source-derived documentation, compatibility decisions and exact-SHA qualification. Freeze a complete candidate before terminal gates; record fresh versus safely reused execution and unrun human/platform boundaries separately. Use the [GitHub-direct procedure](../agents/GITHUB-DIRECT-DEVELOPMENT.md). No automatic merge, source-writing Actions, alternate publisher, forced history rewrite, new service, or live-library experiment is introduced by this plan.

## Core PR scopes and acceptance boundaries

### H-01 — atomic, retry-safe save-selection-as-deck

Status: **implemented in PR #109; exact-head Static/Fast qualification and human local acceptance remain the current boundary.**

Owner: `src/screens/cards/PracticeResultsModal.tsx`, `authoring/selection_deck.rs`, deck copy primitives and the release migration registry.

The frontend now sends one stable operation identity to a native transaction that creates the destination deck, copies the selected notes/questions/assets/tags, creates fresh schedules, and stores a replayable committed result. Release migration 2 adds only the operation-receipt table; the schema-1 baseline remains immutable and supported as an upgrade/backup input. React no longer deletes a committed result when its follow-up deck-list refresh fails.

Focused regressions cover rollback on copy failure, replay without duplicate creation, identity-conflict refusal, preservation of source scheduling evidence, fresh copied schedules, typed interactive question copying, post-commit refresh failure, and same-operation retry after a lost response.

Exit: failed copying leaves no partially created result; lost reply replays the same deck; reused identity with different input is rejected; refresh/navigation failure cannot delete or recreate committed content. H-02 is implemented in the same requested PR; H-03 follows after #109 rather than a broader controller/type-generation refactor.

### H-02 — activity admission, lifecycle visibility and restore preparation

Status: **implemented in PR #109; exact-head qualification and human local acceptance remain the current boundary.**

Owners: `study_materials/cancellation.rs`, `task_activity.rs`, `library_session.rs`, `library_backup/live.rs` / `activation.rs`, and native close policy.

Collection study-material work now acquires the shared ActivityGuard for the same owned lifetime as its feature-local cancellation guard. Maintenance admission and close protection therefore see collection work consistently, while the feature registry still owns cancellation/progress. Request leases continue to protect actual command completion independently of activity admission.

Restore now resolves, copies, extracts, upgrades and validates a private candidate before closing ordinary request admission. Before exclusive activation it releases its preparation lease, acquires maintenance/exclusive admission, rechecks the original library generation, and verifies the retained source/candidate identity before journaled installation. Invalid preparation cannot replace the live library, and an abandoned prepared candidate has no startup authority.

Focused regressions cover activity reservation/drop behavior, collection cancellation cleanup, maintenance refusal while activity exists, request-generation races, private candidate validation/identity checks, and the existing crash-recovery/receipt paths. No timeout authorizes replacement while an old request still owns the library.

### H-03 — observable frontend failures and asynchronous lifetimes

Owners: `src/platform/PlatformProvider.tsx`, startup/share listeners, `src/ipc/diagnostics.ts`, `ImageSetWorkspace.tsx`, `DocumentProgress.tsx`.

Replace identified console-only failure paths with deliberate bounded structured events. Do not forward arbitrary console objects, prompts, credentials or source text. Invalidate both clipboard fallback timers and unresolved reads on cleanup/source change; reproduce both paste-event orderings before claiming duplicate ingestion is fixed. Listener-lifetime disposed flags remain legitimate; request lanes address competing reads, not every cleanup case.

Preserve visible progress, elapsed time, Stop, input/output/reasoning/cache usage and retry count. Expanded retry categories may use a Details disclosure and readable explanations. Unknown provider usage is not zero. Do not hide useful cost information or cancel already-admitted document jobs when the uploader closes.

Exit: late listeners/clipboard results cannot act on obsolete owners; one gesture cannot add twice; unavailable diagnostics cannot recurse or grow an unbounded queue; post-navigation work continues legitimately.

### H-04 — portable filesystem durability and recovery evidence

Owners: document/checkpoint/backup/asset filesystem helpers and `studydeck/import.rs` compensation.

The helper copies have divergent platform and missing-parent behavior. The Windows concern is source/API-based, not Windows runtime qualification. Define explicit file-content and directory-entry durability contracts; centralize matching mechanics without silently converting mandatory durability into successful no-ops. Preserve primary errors and record safe secondary compensation failures.

Exit: publication/rename/flush fault injection, retained recovery material and coherent old-or-new visibility. Windows behavior needs Windows execution evidence; Linux or Android success does not qualify it. No learner-data, recovery-directory or runner-cache purge.

### H-05 — frontend correctness gates

Add narrowly selected React hook correctness rules and stronger indexed-access checking. Fix actual semantic findings with focused regressions; do not hide them behind blanket exclusions, unsafe assertions, dependency-array churn or a stylistic reformat. Integrate check-only execution into existing wrappers. Do not adopt React Compiler merely to enforce hook rules.

Exit: checks are enforced against the current source; lifecycle and stale-read behavior remains covered. The cleanup verifier's historical #106 activation-text assertion should eventually be replaced with a maintained status contract, not erased to conceal a failure. PR #108 retains that phrase only as an explicitly historical quotation.

### H-06 — explicit library-aware IPC transport

Owners: `src/ipc/transport.ts`, `libraryTransport.ts`, domain gateways and Vite/Vitest wiring.

The transport already has focused tests; the gap is production build-alias integration, not absence of generation tests. Prefer visible application-owned imports and representative real gateway tests with mocks beneath the transport. Preserve SDK/plugin behavior and original-page generation.

Exit: bootstrap/concurrent requests, plugin passthrough, startup diagnostics and post-restore stale writes use the same path as production; no mutation is replayed automatically with a refreshed generation; raw invocation bypass is rejected.

### H-07 — native-owned wire types and typed command declarations

Evaluate a narrow Rust-to-TypeScript exporter before choosing a framework. Validate field renames, tagged unions, null versus omitted fields, defaults, numeric and byte encoding, errors and deliberate dynamic JSON. Generated clients must use H-06's transport rather than bypass generation fencing.

The native leased wrappers deserialize into typed domain inputs; they do not eliminate native validation. Reducing their duplication must preserve command names, argument semantics and original-generation lifetime. The generic dispatch lease alone is not an async-future lease. Keep execution placement separate from shape-only refactoring.

Exit: native wire changes update the checked-in contract or fail check-only verification; serialization fixtures agree across the boundary. Actions may compare temporary generator output but never rewrite or publish maintained source.

### H-08 — bounded nonblocking native command execution

Owners: native command adapters, database owner, request/activity leases.

Move substantial blocking SQL/filesystem work off the native event thread without merely placing synchronous work inside async functions. Keep bounded concurrency and hold leases in the actual worker through completion, including caller cancellation or lost replies. Measure connection wait and execution time without logging learner content.

Exit: a blocked database operation does not block unrelated event-thread interaction; transaction/replay/restore behavior is unchanged. A read-only connection pool is a separate measurement-driven proposal, not included automatically.

### H-09 — bounded startup validation and conversion staging

Owners: `database.rs`, `database/release.rs`, `payload_entry.rs`, restore admission and private staging.

Remove demonstrably redundant current-payload validation while retaining appropriate admission, migration/import/restore and write/read validation. Replace whole-database in-memory staging where justified by a private disk-backed candidate with space/error/interruption handling and the same typed preservation proof.

Exit: current reopen, unsupported/corrupt input refusal, historical restore, interrupted/low-space staging and typed row/file preservation tests pass. Report representative elapsed-time and peak-memory measurements. A permanent validation stamp is not proof about later bytes.

### H-10 — obsolete bundled interfaces, not supported data readers

Owners: Modal/Dialog callers, current settings/appearance interfaces, legacy live routing, fixture-only props and profile DTO consumers.

Remove unused `bindPreview`/`onManageTemplates` and fixture-only `aiReasoningEffort`; migrate Modal callers without changing focus/Back/busy behavior. Normalize real old checkpoints at their boundary before simplifying live route branches. Trace model/reasoning consumers: the task-request path already selects task defaults, so duplicate DTO fields alone do not establish competing authority.

Exit: current bundled APIs and fixtures match, while dirty-state protection, practice return paths, saved checkpoint normalization and supported restored settings remain intact. Stored-description rewrites or source-identity normalization require separate data-preservation designs.

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
