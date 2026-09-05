# Documentation and cleanup audit

## Scope and source baseline

This is a maintainer's audit record for the documentation refresh after merged PR #86. The inspected baseline is `2e284a9f084b3bce41c2f7fe40950d1846a61a4b`. Current descriptions belong in the [architecture](../architecture/overview.md), [user guides](../README.md), and [GitHub-direct workflow](../agents/GITHUB-DIRECT-DEVELOPMENT.md); this record explains cleanup decisions and evidence, not a second product specification.

The audit combines the repository tree, active entry points, typed command registration, relevant contracts/tests, and targeted reference searches. It is not a line-by-line security review of every implementation or proof that no unreachable code exists. Exact final qualification belongs to the PR's current-head Actions evidence.

## Confirmed documentation drift

| Finding at the baseline | Current source authority | Resolution owner |
| --- | --- | --- |
| README describes study guides as planned and omits OpenRouter | `src/App.tsx`, `src/screens/StudyGuidesScreen.tsx`, `src-tauri/src/ai/catalog.rs` | Product introduction and roadmap |
| Getting started still describes four Flashcards tabs and a compact More menu | `src/screens/flashcards/FlashcardsWorkspace.tsx`, navigation components | Getting started and Flashcards guide |
| Templates and transfer instructions point to removed standing tabs | `src/screens/SettingsScreen.tsx`, `src/screens/flashcards/DeckTransferDialog.tsx` | Settings and backup/transfer guidance |
| Empty-library instructions imply empty folder/unsorted controls are always present | `src/screens/flashcards/deck-library/DeckLibraryView.tsx` | Organization guide |
| Runbook calls `build:app` a bundle build | `package.json`, `scripts/build.sh`, `scripts/build-state.sh` | Command runbook and local handoff |
| Serial-install examples risk passing an unsupported extra argument; no-build path lacks provenance warning | `scripts/android-install.sh` | Command runbook and Android handoff |
| Agent guides permit optional MCP/Codex detours without a current request | Latest explicit GitHub-direct operating requirements | AGENTS and canonical agent workflow |
| Some guides permit manual Actions source-writing | `scripts/verify-agent-workflow-policy.mjs`, source-publication policy | Contributor/agent workflow |
| AGENTS and workflow prose make PR metadata a prerequisite for Check | `.github/workflows/check.yml`, separate `pr-contract.yml`, execution-state contract | Workflow and CI documentation |
| Code standards name the removed monolithic `src/ipc.ts` boundary | `src/ipc/`, IPC verifier | Code standards |
| Architecture describes a frontend window-control adapter absent from the current source topology | Current App/platform tree and native window configuration | Architecture and UI guide |

## Confirmed isolated source cleanup

`src/components/AndroidDeckSharePanel.module.css` is an orphan of the previous share-panel implementation. The baseline tree contains no corresponding component. Reference search for `AndroidDeckSharePanel` finds no importer; selector search for `deck-share-panel` finds only that stylesheet. The current contextual share path is `DeckTransferDialog` and `DeckTransferController`, not this panel.

Remove the orphan stylesheet as an isolated cleanup and run the style/manifest/static and frontend gates. Do not infer that every file with an old component name is equally unused. The PR diff and exact-head qualification determine whether the deletion is published and validated; this record supplies its rationale.

## Intentionally retained boundaries

| Apparent leftover | Why it is live or compatibility-relevant | Removal condition |
| --- | --- | --- |
| Legacy `templates` application section | `App.tsx` routes it to Settings → Card templates; saved/legacy navigation can still enter it | Prove all supported checkpoint/route consumers are migrated and preserve their regression behavior |
| Legacy Flashcards workspace values | Current visible tabs are two, while route normalization handles older state | Coordinate checkpoint/route normalization, tests, and transport changes |
| Schema-v1 UI checkpoint reader | Existing saved session hints must normalize to schema v2 without losing a real resumable session | Only after an explicit supported-data retirement decision |
| `DocumentsDevelopmentStub` | `DocumentsSection` actively renders it when native PDF rendering is unavailable; UI policy also governs it | A replacement must handle the same unavailable-capability state |
| `pdf_*` command/helper names | Some names remain registered or used by PDF compatibility and generation paths | Trace all native/IPC/test consumers; rename as a coherent internal-contract change, not cosmetic deletion |
| Numbered migrations with `anydoc` in the filename | Applied migration identity and checksum are durable evidence even though Xberg is now the non-PDF runtime adapter | Do not rename or rewrite released/applied migration history |
| Older parser fixtures and regression tests | They capture supported input and previous defects independently of today's parser implementation | Replace coverage only when the supported behavior itself changes explicitly |
| `documents/` facade alongside root `document_*` modules | The facade delegates to current lifecycle/repository/task owners; many root modules remain registered and active | A separately scoped ownership refactor must update all consumers and boundary verifiers |
| Metadata projection workflows and scripts | A historical name does not grant source ownership; inspect actual read/write scope | Remove only after checking consumers and preserving required evidence/metadata behavior |

Compatibility is not permission for unlimited legacy code. Each retained boundary needs a concrete consumer and tests, not merely the phrase “backward compatibility.” Conversely, a documentation refresh must not erase durable readers or unsupported-platform feedback to make a directory look tidy.

## Follow-up engineering observations

The current architecture is not a claim that every screen is a pure view or every native service has reached its ideal final module layout. Feature-local controller hooks coexist with `src/controllers/`; `DocumentsScreen`, the application controller, and several native document services still contain substantial orchestration. Their responsibilities should be made explicit rather than hidden behind a diagram that suggests otherwise.

The Android no-build installer selects the newest APK for the chosen build mode by modification time. It does not attest Git SHA or select a previously built artifact by target ABI. The documentation therefore recommends a fresh normal build/install for PR acceptance. Adding explicit artifact provenance/selection would be a separate tested tooling improvement, not a behavior silently promised by this refresh.

Source-level comments or old stage records may preserve names that no longer describe visible UI. Evaluate whether each is historical provenance, an active regression contract, or actually obsolete before deleting. Do not mass-delete stage-named tests, rewrite checksums, or remove a live compatibility route on the strength of a filename search.

## Maintenance exit criteria

The maintained documentation must have one discoverable owner for each current workflow, no contradictory executor policy, and no product claims inferred from roadmap text. All published relative links and required canonical paths must pass the documentation verifier. Any source cleanup must be narrow, caller-checked, and qualified on the final SHA.

Keep historical evidence labeled and separate from current instructions. Report unresolved engineering observations openly rather than claiming an absolute absence of stale code. [Documentation maintenance](../agents/DOCUMENTATION-MAINTENANCE.md) defines how future changes repeat this audit without recreating documentation drift.
