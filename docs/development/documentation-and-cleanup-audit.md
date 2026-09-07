# Documentation and cleanup audit

## Scope and evidence standard

This audit accompanies the repository-wide documentation refresh after merged PR #86. The original inspected baseline is `2e284a9f084b3bce41c2f7fe40950d1846a61a4b`. Current product descriptions belong in the [architecture](../architecture/overview.md), [user guide index](../README.md), [roadmap](../roadmap.md), and [GitHub-direct workflow](../agents/GITHUB-DIRECT-DEVELOPMENT.md); this file records why stale material was changed, removed, retained, or moved to history.

The audit used active composition/route code, native command and migration owners, targeted repository searches, current tests, workflow definitions, manifests, and exact-head CI. It is not a proof that no unreachable line exists or a substitute for a security audit. Cleanup claims below identify a concrete owner/caller check rather than relying only on filenames.

## Documentation drift corrected

| Baseline drift | Current authority / correction |
| --- | --- |
| README called Study Guides future and omitted OpenRouter | Current Study Guide screens/native services and AI catalog; README/roadmap now describe shipped Guides and OpenRouter |
| Getting Started described four Flashcards workspaces and compact More | Current Flashcards/navigation code; docs now describe Decks + Search/Filter, direct compact destinations, Settings gear |
| Templates/transfer pointed to removed standing tabs | Settings Card templates and contextual deck transfer now documented |
| Empty deck library implied folder/unsorted/selection/collapse UI existed | Current quiet empty-state contract documented |
| Appearance docs duplicated and still described old window/guide behavior | `appearance-and-guidance.md` is canonical; `interface-appearance-and-guidance.md` is a compatibility pointer |
| Roadmap listed Study Guides as future and understated document/provider support | Roadmap now separates shipped Guides/Xberg/non-PDF/OpenRouter/Android capabilities from future quizzes/tutoring/new provider adapters |
| Installation overview said Android document processing was a stub | Platform matrix now reflects the implemented shared Android document/AI/sync application and API 36 qualification |
| Development guide said mobile projects were planned | Current Android project/tooling/runtime path documented |
| CI guide blurred current public schema with the historical final-v26 pre-public adoption input | Current public version and pre-public adoption purpose explicitly separated |
| Android multi-device examples passed a standalone `--` to a parser that rejects it | Current docs call `android-install.sh --serial` directly and explain no-build provenance limits |
| Agent docs allowed optional MCP/Codex selection without an explicit current request | GitHub-direct-only normal mode is explicit; custom developer tooling requires current-task authorization |
| Workflow prose made PR metadata a prerequisite for product Check | `PR Contract` and exact-source `Check` are now documented as independent |
| Code standards referenced the removed monolithic `src/ipc.ts` | Current domain gateways under `src/ipc/` documented |
| Current architecture was fragmented and sometimes described intended rather than implemented ownership | `docs/architecture/overview.md` is now the comprehensive source-linked current-system architecture; focused references align with it |
| Peer-sync docs warned that Documents/Study Guides were outside the supported graph | Device-sync package v3 now exposes explicit Flashcards, Documents, Study Guides, API-provider settings, and Saved API key scopes; current sync docs describe the implemented allowlist/dependencies rather than the old limitation |
| Initial USB-sync docs exposed USB tethering, interface discovery, manual IP entry, QR reuse, and firewall troubleshooting as product steps | The maintained Linux→Android cable path now uses Android Open Accessory bulk I/O below `SyncIo`; current docs describe the product contract as plug in one phone, select scopes, click **USB Sync**, with OS USB permissions handled by installation/platform integration rather than the sync UI |

## Proven source/workflow cleanup

### Orphan Android share-panel stylesheet

`src/components/AndroidDeckSharePanel.module.css` had no corresponding component/import; `AndroidDeckSharePanel` and its selectors were not active owners. Current sharing is contextual through `DeckTransferDialog`/controller paths. The orphan stylesheet was removed.

### Duplicate Study Guide implementations

The active owner is `StudyGuidesScreen` + `useStudyGuidesController` + the current inline `StudyGuideDialog` reader. Repository/caller inspection showed `StudyGuideWorkspace.*` and `StudyGuideLibrarySection.*` were unmounted predecessor implementations with their own duplicate styles/tests. They were removed; regression coverage was retained on the active reader/library and an active-library empty-state test was added.

This cleanup intentionally does **not** rename `StudyGuideDialog.tsx` merely because the reader is now inline. That filename is awkward but live; renaming it is an ownership/refactor task, not dead-code removal.

### Automatic Orchestrator PR Ledger

`.github/workflows/orchestrator-pr-ledger.yml` was the only workflow requesting `pull-requests: write`. It ran after successful `Check` and automatically PATCHed PR bodies through the dedicated `scripts/project-orchestrator-checklist.mjs`. Repository search showed that script was owned only by that workflow plus its dedicated test.

The current development model does not need an automatic orchestrator to project checklists into PR metadata: the branch writer updates PR evidence directly, `PR Contract` validates metadata independently, and `Check` qualifies source. The workflow, dedicated projector script, and its exclusive test are removed. Workflow policy is hardened so pull-request write permission cannot quietly reappear as another metadata writer.

### Completed frontend-refactor source checkpoint

`.github/workflows/frontend-refactor-source-checkpoint.yml` targeted only completed PR #75/#76-era branches/paths and uploaded a one-day source snapshot. It no longer participates in current development. The workflow is removed.

Its useful completion record, `frontend-css-refactor-checkpoint.md`, is preserved under `docs/history/` rather than retained in current development guidance.

### Completed UI redesign stage documents

The full Stage 0/redesign roadmap, pre-change characterization, affected-surface inventory, performance baseline, and visual baseline are historical evidence. Their full records are preserved under `docs/history/`; old `docs/development/ui-workflow-redesign-*.md` paths remain short explicit historical pointers where stable paths are useful for test/provenance references. They no longer compete with current architecture/roadmap/user guides.

## Intentional compatibility and live-looking leftovers

| Apparent leftover | Why it remains | Removal condition |
| --- | --- | --- |
| Legacy `templates` application section/route values | Normalizes/redirects supported old navigation into Settings → Card templates | Remove only with coordinated checkpoint/route compatibility change and regression coverage |
| Legacy Flashcards workspace values | Current UI has two tabs; supported old state still needs normalization | Versioned retirement of all durable route/checkpoint consumers |
| Schema-v1 UI checkpoint reader | Can recover a real supported old session while normalizing to schema v2 | Explicit supported-data retirement decision |
| `DocumentsDevelopmentStub` | Actively renders the unavailable-native-document-capability state | Replacement must preserve honest unavailable/degraded behavior |
| PDF-prefixed compatibility names | Some remain active command/checkpoint/internal compatibility owners | Trace native/IPC/test/durable consumers and migrate coherently |
| Migration filename containing `anydoc` | Applied migration identity/checksum is durable history; current runtime parser is Xberg | Never rename/rewrite released migration history |
| Older parser fixtures | Capture supported inputs/regressions independent of current backend | Replace only when the supported behavior/fixture itself is intentionally retired |
| `documents/` facade plus root `document_*` modules | Root modules remain registered/active behind current facade/service boundaries | Separately scoped ownership refactor with all consumers/verifiers/tests updated |
| `StudyGuideDialog.tsx` name | File is active; implementation renders an inline workspace despite old name | Optional rename/refactor, not dead-code cleanup |
| Legacy split-runner note | Explicitly documents migration context and warns against old labels | Could move to history if all current operator links are redirected |
| Performance/visual baseline files | Stable provenance/comparison evidence, not current UI spec | Keep or version when new benchmark methodology supersedes them |

Compatibility must name a real consumer. Conversely, an old name or historical fixture is not automatically dead code.

## Follow-up engineering observations

The current architecture is deliberately truthful about remaining concentration. `useAppController`, document workflow code, and several native document services still coordinate substantial behavior. Feature-local controller hooks coexist with `src/controllers/`. Future decomposition should isolate one reason-to-change at a time and preserve behavior tests rather than inventing another framework solely to make files smaller.

The Android no-build installer chooses the newest matching APK by modification time. It does not attest Git SHA/ABI provenance. Current handoff docs therefore recommend a fresh normal build/install for PR acceptance. Explicit artifact-provenance selection would be a separate tooling improvement.

Device synchronization has since gained the versioned granular-scope extension that the original audit said was required: Documents, their managed source/derived files and durable processing/LLM graph, and Study Guides are explicit selectable domains in package v3. The architectural rule behind the old warning still stands: a new table is **not** synchronized merely because it exists. Future learner-data domains still require an explicit scope/allowlist decision, coherent graph/package handling, compatibility policy, merge semantics, tests, and documentation.

The first USB synchronization proof-of-concept also demonstrated why transport-neutral architecture is not the same thing as acceptable platform UX. Reusing TCP over USB tethering preserved crypto/package/merge ownership but exposed transport plumbing to the user. The corrected maintained design keeps that upper-layer reuse while replacing the cable adapter with direct Android Open Accessory I/O. Linux libusb and Android `UsbManager` now own device discovery/opening; `SyncIo` owns only the byte-stream contract; platform installation owns USB access rules. Future platform-specific USB work should preserve that separation and must not reintroduce manual networking configuration as a normal product workflow.

Current non-PDF canonical provenance remains section-oriented despite Xberg exposing richer internal structure. Promoting slide/sheet/chapter identities into public citations requires coordinated schema, backup, UI, and compatibility work.

## Exit criteria for this refresh

The refresh is ready for review when:

- README remains a human product introduction rather than implementation ledger;
- user/install/roadmap claims match active source/platform boundaries;
- one comprehensive current architecture is discoverable with focused supporting references;
- agent/contributor docs converge on one-writer GitHub-direct publication, independent PR Contract/Check, exact-SHA evidence, and safe local handoff;
- completed stage/checkpoint material is labeled historical;
- only caller-checked obsolete code/workflows are removed;
- documentation/workflow/static/fast/full/platform gates required by the final exact head pass;
- remaining manual review is explicitly identified rather than inferred from automation.

This audit intentionally does not claim mathematical proof that no unreachable code remains. [Documentation maintenance](../agents/DOCUMENTATION-MAINTENANCE.md) defines how future reviews continue the evidence-based cleanup without recreating documentation drift.
