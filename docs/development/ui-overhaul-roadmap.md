# shadcn/ui + Tailwind UI overhaul roadmap

Status: implementation roadmap recorded by Phase 0. Runtime migration has not started in this document's Phase 0 PR.

Planning base: PR #94 (`agent/semantic-ai-tutor`) at `279430d799b78add3a0885168e011de0b0d47ff8`.

The roadmap assumes PR #94's Tutor and expanded provider/model UI are part of the application being migrated. If the parent PR changes materially before a later implementation stage, that stage must re-read the current merged/source head rather than treating this inventory as a frozen source manifest.

## Target architecture

```text
feature/domain UI
  -> oStudy semantic UI facades and domain components
    -> owned shadcn source under src/ui/vendor/shadcn
      -> Base UI behavioral primitives + Tailwind CSS v4
        -> oStudy semantic design tokens and root capability state
          -> DOM/CSS in the Tauri WebView
```

Tailwind is allowed in feature/domain components for local presentation. Base UI and vendor shadcn imports are not. oStudy's token sources, appearance provider, platform capability rules, native/controller boundaries, and domain semantics remain authoritative.

## Cross-phase rules

Every implementation stage follows these rules:

1. **Re-read the actual base/head.** A later stage must not assume PR #94 or this roadmap still matches current source exactly.
2. **One subsystem at a time.** Avoid a repository-wide mechanical conversion that mixes primitives, navigation, Tutor, editor, drag/drop and platform fixes in one review boundary.
3. **Keep old and new renderers interoperable during migration.** Legacy CSS Modules and Tailwind may coexist temporarily because both consume the same oStudy semantic tokens.
4. **Do not mix semantic redesign with renderer replacement unless required.** Preserve existing public component props and behavior first; perform API cleanup only when it yields a clear product/maintenance benefit.
5. **Test the smallest affected boundary first.** Static/fast checks should catch dependency/style-policy failures before Linux/Android runtime gates.
6. **Freeze before expensive qualification.** Documentation, cleanup, stale-claim review and source audit happen before terminal full/platform qualification, not after it starts.
7. **No framework leakage into durable layers.** Rust, IPC, controllers, domain state, persistence, scheduling, Tutor retrieval/provider logic and document processing must not acquire shadcn/Base UI/Tailwind concepts.
8. **Preserve accessibility/platform state.** Keyboard, focus restoration, touch targets, coarse pointer, hover absence, reduced motion/transparency, forced colors, safe areas, Android IME and Back behavior are acceptance requirements.

## Phase 0 — inventory, architecture and migration contract

**Purpose:** establish what will change and what must not change before dependencies or rendering are touched.

**Deliverables:**

- accepted architecture record for shadcn/ui + Tailwind behind oStudy ownership;
- PR #94-aware affected-surface inventory;
- this staged roadmap;
- documentation index links;
- explicit acceptance criteria and non-goals.

**Exit criteria:** documentation-only diff; child PR is stacked on PR #94; no dependency/lockfile/source changes; inventory covers Tutor/provider UI; later phases have clear ordering and ownership.

**Validation:** documentation/link verification and PR metadata/stack review. Full runtime qualification is not justified by an intentionally Markdown-only Phase 0 change unless repository automation requires it.

## Phase 1 — Tailwind/shadcn build foundation

**Purpose:** introduce the toolchain without changing product appearance.

**Expected changes:**

- add Tailwind CSS v4 and the Vite integration;
- add `components.json` configured for this Vite/React/Tauri project and an explicit `src/ui/vendor/shadcn` alias/path;
- add only dependencies actually required by the selected initial shadcn/Base UI primitives;
- add a small `cn`/class-merge utility in the UI implementation layer;
- configure source detection so production classes are statically discoverable;
- perform license/dependency/platform audit and update lockfile coherently;
- add boundary verification scaffolding before vendor imports can spread.

**Do not:** migrate feature screens, replace themes, or import vendor components directly from features.

**Exit criteria:** existing UI renders unchanged; frontend/native builds remain green; Tailwind can compile one non-production/proof class; no duplicate global reset/preflight breaks current CSS.

**Key risk:** Tailwind preflight/layer order changing existing controls/global styling before intentional migration. Explicitly verify current shell, editor content and Android baseline.

## Phase 2 — token and capability bridge

**Purpose:** make Tailwind/shadcn consume oStudy's existing visual authority.

**Expected changes:**

- extend `scripts/generate-ui-tokens.mjs` to emit a Tailwind semantic theme adapter and shadcn compatibility aliases;
- map `background`, `foreground`, `card`, `popover`, `primary`, `secondary`, `muted`, `accent`, `destructive`, `border`, `input`, `ring`, radii and application-specific semantic states onto `--ostudy-*` variables;
- expose root `data-theme`, `data-motion`, `data-material`, `data-input`, `data-hover`, and contrast states to Tailwind through explicit custom variants where useful;
- keep light/dark/console/synthwave JSON theme parity validation;
- add tests that prove every compatibility token resolves from oStudy-owned tokens.

**Do not:** create a separate shadcn palette or a parallel `dark` class theme source.

**Exit criteria:** a Tailwind-styled proof component changes correctly across all current themes and capability states without hard-coded palette values.

**Key risk:** semantic-name collision between legacy short aliases and new Tailwind/shadcn names. Resolve centrally in generated adapter output, not per component.

## Phase 3 — low-risk primitive renderer proof

**Purpose:** prove the vendor boundary and incremental facade strategy.

**Initial candidates:** Button, Badge, Separator, Skeleton, Progress, simple Surface/Panel composition.

**Expected changes:**

- generate/copy selected shadcn source into the vendor directory;
- adapt existing oStudy primitives to the new renderer while preserving caller-facing semantics where practical;
- retain focused tests for disabled, focus, intent, touch target, ARIA and theme behavior;
- add `verify:ui-boundaries` or equivalent enforcement forbidding Base UI/vendor imports outside approved UI implementation owners.

**Exit criteria:** representative existing screens use new renderer through unchanged or deliberately versioned oStudy APIs; no feature imports the vendor layer; old primitive CSS is removed only where fully superseded.

**Key risk:** using the migration as an excuse to change button semantics/labels everywhere. Keep renderer proof narrow.

## Phase 4 — forms and Settings vertical slice

**Purpose:** stabilize the generic control vocabulary on a real, form-heavy product area.

**Primitive scope:** Input/TextField, Textarea, Checkbox, RadioGroup, Switch, Select, Slider where currently needed, field descriptions/errors, password visibility controls, form grouping.

**Product scope:** general Settings plus PR #94's expanded AI provider/model settings.

**Important PR #94 surfaces:** provider list/detail, Add provider, provider/API URL, API key/password visibility, connection state, model catalog, Tutor/default/embedding model selection, timeout/auth/storage/privacy options, dirty/discard/delete confirmation and responsive layout.

**Architecture:** preserve `FormField`-style oStudy ownership for labels/help/error ARIA relationships. Controllers continue to own dirty state, requests and secret-safe operations.

**Exit criteria:** Settings no longer relies on ad-hoc native generic controls or legacy shared button classes except documented transitional cases; keyboard/tab order and form labels remain correct; provider secrets remain behind native ownership.

**Validation emphasis:** focused Settings/controller tests, accessibility roles/names, provider error/dirty-state flows, compact layout and Android text-input/IME behavior.

## Phase 5 — overlays and adaptive interaction

**Purpose:** replace the hardest generic UI behavior once the theme/form foundation is stable.

**Scope:** Dialog, AlertDialog, Drawer/Sheet, DropdownMenu, ContextMenu as needed, Popover, Tooltip, Select/Combobox popovers, confirmation rendering, focus/portal/scroll-lock behavior.

**Application abstractions:** preserve or introduce oStudy `ConfirmDialog`, `AdaptiveDialog`, `AdaptiveActionMenu`, and one overlay stacking/back-navigation policy.

**Exit criteria:** feature code does not implement ad-hoc backdrops/focus traps/ordinary `window.confirm`; existing confirmation API callers continue to work; compact/coarse-pointer variants are deliberate and tested.

**Validation emphasis:** Escape, focus restoration, nested overlays, pointer-outside behavior, Android Back, safe areas, IME-visible behavior, scroll locking and reduced motion.

## Phase 6 — feedback, status and transient presentation

**Purpose:** unify loading/success/warning/error/transient UI.

**Scope:** Toast renderer (using the current recommended shadcn ecosystem choice at implementation time), InlineNotice/Alert, progress/skeleton states, status badges, empty states, cancel/retry presentation.

**Architecture:** existing semantic feedback/events remain application-owned. Durable operation success must never depend on animation, toast or haptic/audio effects.

**Exit criteria:** one semantic vocabulary for status states across Settings, Documents, Tutor, sync and imports; no duplicate toast stack; reduced-motion/material settings respected.

## Phase 7 — shell and navigation

**Purpose:** make the entire application feel visually coherent after low-level components are stable.

**Scope:** App shell, NavigationSidebar, MobileAppBar, Brand, headers, route chrome, common toolbars, responsive surfaces and navigation action presentation.

**PR #94 requirement:** Tutor remains a first-class section in desktop/compact navigation and UI checkpoints. Contextual Tutor entry does not become an ordinary route that destroys origin state.

**Exit criteria:** navigation semantics and labels are unchanged unless separately approved; compact navigation remains shallow; Settings placement and Android Back contracts pass; shell uses the new visual hierarchy/tokens.

## Phase 8 — Flashcards library, search/filter and bulk workflows

**Purpose:** migrate the largest ordinary data-management workspace.

**Scope:** deck/folder library, archived/unsorted sections, deck detail, Search/Filter, CardFilters, tags, selection toolbar, bulk dialogs, transfer/import UI and practice setup chrome.

**Keep custom:** selection models, folder/deck organization, drag/reorder state/geometry, FTS/query semantics, transfer rules, scheduling mutations and checkpoint compatibility.

**Order inside phase:** migrate static row/card chrome and controls first; drag handles/reorder DOM last.

**Exit criteria:** ordinary controls use shared primitives; migrated subtrees no longer add CSS Modules; drag/reorder and touch selection behavior match focused regressions.

## Phase 9 — card editor chrome

**Purpose:** modernize the authoring experience without coupling the rich-content model to the UI library.

**Scope:** editor toolbar, buttons, dropdowns, popovers, dialogs, tabs, tags, metadata, template controls, media/math controls, save/delete status and responsive panel layout.

**Retain:** Tiptap/ProseMirror schema/extensions, content validation/serialization, cloze/domain behavior, asset/citation ownership and specialized content-editable CSS.

**Exit criteria:** generic chrome uses shared primitives/Tailwind; rich-content rendering remains application-owned; no vendor types enter editor data structures.

## Phase 10 — Documents and Study Guides

**Purpose:** unify content-library and generated-artifact presentation.

**Documents scope:** import/upload/search, document list/cards, progress/status, metadata, action menus, generation state, errors and confirmation UI.

**Study Guides scope:** library, reader toolbar, dialogs/sheets, sections/disclosures, source/citation presentation, export/practice actions and contextual Tutor entry.

**Retain:** ingestion/OCR/rendering, document/guide revisions, source provenance, managed files, generation/checkpoint logic, learner overlays and rich body CSS where appropriate.

**Exit criteria:** ordinary chrome is migrated; source/citation semantics remain intact; reader/document state survives responsive overlay transitions.

## Phase 11 — Analytics

**Purpose:** migrate a visually dense but behaviorally isolated area after common form/layout primitives are mature.

**Scope:** metric surfaces, filters, date/deck selectors, tabs, empty/loading states, chart containers and responsive layout.

**Retain:** Chart.js, analytics query/aggregation semantics and difficulty-series logic.

**Exit criteria:** Chart.js remains renderer-independent; filter accessibility and responsive chart containment pass.

## Phase 12 — Practice and contextual hint integration

**Purpose:** migrate the most interaction-sensitive study workflow after overlays, forms and shell behavior are proven.

**Scope:** practice shell, card surface chrome, reveal/status/progress UI, action menus, completion screen, results, practice setup presentation, and PR #94 contextual Tutor hint entry/panel.

**Retain:** session snapshots, scheduler operations, keyboard shortcuts, interval semantics, review submission, reveal state, exit guard and grading meaning.

**Domain components:** grading controls should remain practice-owned even if they use shared Button internals; they should not become arbitrary generic color variants.

**Exit criteria:** no scheduler behavior changes; active card/session state survives opening/closing contextual Tutor; keyboard, touch and Android Back/IME paths pass focused and platform smoke coverage.

## Phase 13 — Tutor workspace

**Purpose:** migrate the PR #94 Tutor only after its dependencies and interaction patterns are stable.

**Scope:** conversation/material selectors, composer Textarea, retrieval/memory controls, preparation progress, transcript/message layout, citations/source dialog, quiz/feedback presentation, retry/cancel/error states and responsive contextual/top-level layouts.

**Domain components to keep application-owned:** `TutorMessage`, `TutorComposer`, `TutorCitation`, `TutorContextIndicator`, quiz/feedback presentation, material-preparation semantics and conversation/memory policy.

**Retain outside UI:** semantic retrieval, source revision checks, provider calls, consent/privacy, persistence, operation identity, hidden-answer safeguards and quiz evaluation.

**Exit criteria:** top-level and contextual Tutor share one coherent design vocabulary; hint mode never exposes source previews/hidden-answer controls through a presentation regression; saved conversations remain readable offline; Back/focus/return behavior remains intact.

## Phase 14 — remaining CSS Module migration

**Purpose:** finish ordinary presentation conversion after all major subsystems have established patterns.

For every remaining Module:

1. classify each rule as ordinary component styling, reusable semantic pattern, specialized content CSS, platform workaround, animation, or obsolete rule;
2. migrate ordinary component styling to Tailwind;
3. move truly reusable semantics into the correct oStudy UI owner rather than copy/pasting class strings;
4. retain dedicated CSS when descendant/pseudo/platform/content rules are clearer there;
5. delete the Module only after import/caller tests prove it is fully superseded.

**Exit criteria:** no unexplained generic feature CSS Modules remain; retained CSS has an owning rationale; no parallel legacy button/form vocabulary remains.

## Phase 15 — cleanup, enforcement and stabilization

**Purpose:** turn temporary migration rules into permanent architecture.

**Scope:**

- remove dead old primitives/overlay implementations and exclusive tests/styles;
- retire legacy shared class maps after the final consumer moves;
- remove temporary compatibility aliases that no longer serve supported code;
- strengthen boundary/native-control/semantic-color/dynamic-class/legacy-CSS checks;
- update current UI architecture/user guidance to describe the implemented system rather than this plan;
- refresh visual/performance baselines only for the accepted final visual system;
- evaluate whether retaining shadcn tooling still adds value or whether an eventual supported eject/vendor-only state is preferable.

**Exit criteria:** replacing Base UI/shadcn later would primarily affect `src/ui`; feature/domain code remains stable; current docs no longer describe migration as future work; Linux/Android and active desktop/mobile target qualification is green for the frozen candidate.

## Validation ladder for implementation phases

The exact maintained wrappers remain authoritative, but the intended order is:

```text
component/unit tests
  -> migration-specific static policy checks
    -> ./scripts/check-changed.sh where useful during iteration
      -> ./scripts/check-static.sh
        -> ./scripts/check-fast.sh
          -> targeted native/WebDriver/Android smoke for affected behavior
            -> required terminal full/platform qualification on the frozen candidate
```

A late Markdown-only correction must follow the repository's qualification-reuse rules; it is not a reason to repeatedly rerun expensive platform suites. Source, tests, configuration, dependency or lockfile changes are qualification-bearing and invalidate that narrow reuse path.

## Visual acceptance strategy

Do not judge success solely by old screenshot pixel equality: the overhaul intentionally changes presentation. Instead each migrated phase should distinguish:

- **behavioral invariants** that must remain identical;
- **visual baselines** that are intentionally replaced after human acceptance;
- **theme/capability matrices** that must continue rendering coherently;
- **performance budgets** that prevent expensive component/portal/animation regressions.

Representative viewport coverage should continue to include compact/mobile, medium/tablet-like, and wide desktop layouts across the supported themes, with actual Tauri/Linux/Android runtime checks for high-risk interactions.

## Branch/PR sequencing

Phase 0 is intentionally a child of PR #94 because the active Tutor/provider work changes the UI inventory. Later implementation should not grow forever on one stacked branch. After PR #94 is merged and this planning PR is reconciled/merged, implementation phases should use short-lived `agent/<phase-description>` branches from the then-current authoritative base.

If implementation must begin before PR #94 lands, each child PR must clearly state its parent/base dependency and be reconciled with the moving parent before qualification. Do not silently retarget a stale child to `main` and assume the diff still represents the intended product.

## Rollback model

The migration is deliberately reversible by stage. Each phase must leave the application runnable and must delete old implementations only after its replacement is integrated and tested. Because the token source and oStudy semantic facades remain stable, a failed renderer migration can revert at the subsystem boundary without reverting unrelated Tutor/provider/domain work.

This is the central reason to avoid a repository-wide one-shot conversion.