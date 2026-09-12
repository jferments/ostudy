# UI overhaul implementation after PR #94

Status: the source implementation through roadmap Phase 8 is integrated in PR #96. This is not the completed application-wide overhaul. Automated qualification, human visual acceptance and merge are separate states; their actual results belong to the exact-SHA PR evidence rather than this document.

## Preserved baseline and ownership

PR #94 was squash-merged as `fd5659e77671cb814393024c1baa2e0cba9a59d7`. PR #96 incorporated its final product tree with merge `35ef21cfefc500b7ab47e4f6189e7493de124ab6` and targets main. The original Phase 0 inventory used the earlier `279430d...` source; it remains historical planning evidence.

The final Tutor retains multi-source selection, contextual chat settings, safe Markdown and conversational questions through Send. General chat does not require embeddings or selected material. Keep hidden-answer/citation restrictions, source permissions, operation identities, private question criteria and offline conversation reading. Do not restore Quiz me. DocumentJobsProvider keeps document work alive across navigation; stop/cancel and nonfatal diagnostics retain their meaning.

No native transport, provider secret storage, database schema, scheduler, IPC or document-job redesign is part of this presentation migration. The Android instrumentation readiness repair changes the test's observation boundary, not production behavior.

## Foundation: Phases 1–3

The official Tailwind Vite integration produces static CSS. Exact dependency pins, the committed lock graph, TypeScript/Vite aliases and `components.json` define the Base UI family and `tw:` prefix. The application does not require a browser compiler, CDN or live component registry.

Generated Tailwind aliases resolve to oStudy semantic tokens. Default palette utilities are disabled; legacy `--muted` and `--accent` retain their old meanings. AppearanceProvider remains the authority for four themes and root input/motion/material/contrast capabilities. Safe areas, IME rules, local fonts and independent interface/card scaling remain.

Preflight is not imported. Prefixing avoids collisions with legacy classes. Utilities temporarily compile in the existing components layer so unmigrated feature overrides remain compatible. Migrated owners no longer import their superseded generic feature styles. Partial-border recipes explicitly clear the other edges because the browser defaults are not reset by Preflight. Moving utilities above all remaining feature styles belongs to the later caller-override audit, not a global important flag.

Button, Badge and Skeleton proved the stable-facade boundary. Later shared controls extend that same design rather than inventing a parallel feature-specific vocabulary.

## Phase 4: forms and Settings

Shared TextField, NativeSelect, Textarea, Checkbox, Switch, RadioGroup, Slider, Disclosure, Tabs and SegmentedControl use owned shadcn/Base UI composition. Surface/Panel use Card composition. Native refs, form values, labels/errors, disabled/loading protection and keyboard behavior are tested. FormField separates contextual help from the input label. Disclosure content stays mounted so closing a group does not discard a draft. NativeSelect deliberately retains the operating-system picker, native options, optgroups and listbox semantics.

The visible Settings migration includes:

- **Appearance:** theme-preview cards, a preview using actual shared controls, typography/scaling, motion, artwork and accessibility groups.
- **Review/Scheduling:** shared numeric fields, sliders, checkboxes and panels, preserving bounds and scheduling meanings.
- **AI provider/task models:** provider selection/setup cards, advanced disclosure and three task-model panels. Credential ownership, model-specific probes, assignments, reasoning policy and retry identities remain in their controllers.
- **Diagnostics and library safety:** diagnostic session/state/privacy panels, backup/storage controls and readable wrapping paths. Native operations are unchanged.
- **Device sync:** identity, sender/receiver, trusted devices and conflict panels; shared scoped checkboxes, text fields, textarea and actions. Study Guides still require Documents; saved keys still require provider settings and available secure storage. USB and LAN remain distinct transports. A detected but failed connection is warning-styled, not successful.
- **Templates:** library cards, details, rich-field definitions, generated-card rules, save/discard/delete controls and error/loading/empty states. The rich Tiptap layout editor and durable asset draft remain application-owned.

The previous provider/model, appearance, template and generic sync styles are removed. Only QR quiet-zone geometry remains in DeviceSyncSettings.module.css, including its required white background.

## Phase 5: overlays and adaptive interaction

Dialog/Sheet/confirmation, menu/overflow and tooltip behavior now use Base UI behind oStudy adapters. Base UI handles DOM focus containment/restoration, keyboard interaction, popup positioning and modality. OverlayProvider retains the application stack and native Back policy; it is not a second competing browser focus trap.

Busy/non-dismissible state, compact sheets, initial focus, nested confirmations and return focus remain public contracts. A menu closes before an action opens a dialog. Compact overflow actions use a shared sheet. Native selects remain native rather than installing an unused combobox for migration bookkeeping. Additional specialized popup widgets should be added only for actual consumers.

Metadata and contextual-help presentation use shared dialogs/sheets and semantic Tailwind. InlineWorkspace remains a nonmodal route for study content and is not converted into a blocking dialog. Guided tutorials retain their explicit opt-in, module state and spotlight geometry; ordinary tutorial actions use the shared buttons.

## Phase 6: feedback, status and recovery

Inline notices, loading, EmptyState, badges/chips, progress and notifications share semantic presentation. Notifications use Base UI Toast under the application's existing one-message API; action identity and absolute Undo expiration remain app-owned. An action already past its Undo deadline cannot be restored by hovering a toast.

Committed success remains success if a later refresh/navigation fails. Anki import completion is a post-commit receipt, not a preview or another submit opportunity. Indeterminate package preparation does not fabricate a zero-of-one count. Card-level progress retains accurate processed/succeeded/skipped values. Error notices use alert semantics; ordinary success/status does not masquerade as an error. Sync failure, conflict decisions, cancellation and retry keep their original operations.

Progress's rich inline status content remains a specialized oStudy presentation, while standard bar behavior comes from the shared renderer. No controller depends on an animation or notification successfully completing.

## Phase 7: shell and navigation

AppFrame and the non-collapsing shadcn Sidebar composition replace the old shell presentation. Wide/medium layouts use a labeled sidebar and inset content surface. Compact layouts retain five direct destinations and the Settings action in the app bar. There is no new hidden More navigation or competing vendor router.

PageScaffold, PageHeader, CommandBar, selection toolbars and responsive action groups provide shared hierarchy and spacing. Navigation identities, lazy feature roots, dirty guards, contextual workflows, checkpoint semantics, safe areas and IME behavior stay with the existing application/platform owners. Generic error-boundary actions use the same controls.

## Phase 8: Flashcards, search, authoring setup and transfer

The production Flashcards route now uses the migrated library, folder/archive presentation, row selection/actions, search/filter workspace and card browser. Pointer drag geometry and mutation policy remain separate from visual recipes. Keyboard move/reorder alternatives remain available; hover or canceled drags do not commit an organization change.

CardFilters uses shared fields, checkbox/radio groups and chips, while the shared browser retains distinction between source notes and generated presentations, visible/all-matching selection, tag scope, query/sort behavior and transactional bulk identities. Bulk entry/tagging, move/copy, deletion, rescheduling and practice-result dialogs are migrated. Deck-only authoring remains deck-only.

Selected-deck Practice setup uses shared panels, tagged choices, explicit scheduled/extra modes and meaningful workload metrics. Obsolete workload results disappear until the exact new query is ready. The start lock, invalidated deck snapshot handling and refusal to silently substitute extra study remain.

DeckDetailRoute owns the migrated header/actions and source footer. The browser remains the same mounted instance when entering/leaving its editor; a regression test verifies unsaved local state survives that layout transition. Deck import/export/share and bounded Anki conversion receipts use the same panels and feedback.

## Deliberate retained CSS and later scope

Completion through Phase 8 does not mean zero CSS or all application screens converted.

| Retained owner | Reason |
| --- | --- |
| Deck pointer/list/drag feedback geometry | Hit testing, insertion destinations, drag transforms, scrolling and performance; not a second generic button system. |
| DeviceSync QR frame | Machine-readable format needs fixed quiet-zone geometry and literal white contrast. |
| Guided tutorial spotlight/popover | Target measurement, viewport collision geometry and spotlight masking. Actions/text are migrated. |
| InlineWorkspace and platform foundation | Native route-height, safe-area, IME and nonmodal layout contracts. |
| Rich editor, media/math, document and print CSS | Specialized content styling; feature-level editor/reader migration belongs to Phases 9–13. |
| Legacy shared class maps and unmounted DecksScreen regression harness | Remaining later-phase consumers and historical behavior coverage. Live DeckDetailRoute no longer imports DecksScreen styling. Retire the unmounted harness with equivalent active-route coverage in Phase 15. |

Phases 9–13 still cover editor chrome, Documents/Study Guides, Analytics, the active Practice experience and Tutor transcript/composer layouts. Shared controls affect them now, but that is not a claim that their domain-specific layout migration is complete. Phases 14–15 finish the remaining CSS owners, legacy compatibility cleanup, visual/performance baselines and final whole-application stabilization.

## Tests and acceptance

The TypeScript import verifier restricts vendor/Base UI ownership. The class-ownership check rejects interpolated utility names and unowned legacy classes. The in-memory CSS build now discovers all shared UI plus live Phase 4–8 feature sources, verifies every used utility and the four themes/capability variants, and rejects raw generic controls in migrated feature owners. It also checks explicit partial-border behavior with no Preflight.

Behavior suites retain form/ref/loading/disabled contracts, keyboard tab/radio behavior, disclosure drafts, Settings/provider/sync operations, atomic import receipts, deck-library drag/selection, browser bulk actions, Practice setup and nested modal/Back behavior. They assert observable outcomes rather than hidden-input implementation details. Node-only suites remain explicit `.node-test.mjs` files.

Android's fresh-library test now waits for the asynchronous library surface, not only the earlier route heading. All existing create/import, empty-state and irrelevant-control assertions remain. A Python policy regression locks in that readiness boundary and semantic selectors.

Qualification proceeds focused/static/fast first, then frozen full Linux and Android. Do not perform opportunistic source/docs edits during terminal qualification. A queued run or an ancestor pass is not a current-head result. Manual acceptance still needs four-theme, enlarged-text, keyboard/focus, narrow layout, actual Android picker/IME/Back and representative populated/error states. No measured speedup or real-world defect-rate improvement is asserted solely from library adoption.

Use [local testing and handoff](local-testing-and-handoff.md) for exact checkout, safe data isolation and maintained build/install commands. Revert a UI increment normally to roll back; do not revert unrelated PR #94 work or discard user data.

See [the roadmap](ui-overhaul-roadmap.md), [ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md), [vendor provenance](../../src/ui/vendor/shadcn/README.md), and [the bounded publication loop](../agents/GITHUB-DIRECT-DEVELOPMENT.md).
