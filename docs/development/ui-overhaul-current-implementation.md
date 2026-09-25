# UI overhaul implementation after PR #94

Status: **historical implementation record for completed PR #96.** PR #96 finished Phases 1–15, passed exact-head automated qualification on its final PR head `8b681f9c4671946158ccd85b9747e539bec281bd`, and was squash-merged to `main` as `5a4a4212e15c847db5bbf5daf1948e6ca6b3f860`. Its intentionally separate human visual/device acceptance boundary did not mean the migration source was unfinished. Post-merge aesthetic work is now owned by [the visual refresh roadmap](ui-visual-refresh-roadmap.md); this file remains the implementation history for the architecture established by #96.

## Preserved baseline and ownership

PR #94 was squash-merged as `fd5659e77671cb814393024c1baa2e0cba9a59d7`. PR #96 was then completed against that product baseline and squash-merged as `5a4a4212e15c847db5bbf5daf1948e6ca6b3f860`. The original Phase 0 inventory used the earlier `279430d...` source; it remains historical planning evidence rather than a manifest of current files.

The final Tutor retains multi-source selection, contextual chat settings, safe Markdown and conversational questions through Send. General chat does not require embeddings or selected material. Keep hidden-answer/citation restrictions, source permissions, operation identities, private question criteria and offline conversation reading. Do not restore Quiz me. DocumentJobsProvider keeps document work alive across navigation; stop/cancel and nonfatal diagnostics retain their meaning.

No native transport, provider secret storage, database schema, scheduler, IPC or document-job redesign was part of this presentation migration. The Android instrumentation readiness repair changed the test's observation boundary, not production behavior.

## Foundation: Phases 1–3

The official Tailwind Vite integration produces static CSS. Exact dependency pins, the committed lock graph, TypeScript/Vite aliases and `components.json` define the Base UI family and `tw:` prefix. The application does not require a browser compiler, CDN or live component registry.

Generated Tailwind aliases resolve to oStudy semantic tokens. Default palette utilities are disabled; legacy `--muted` and `--accent` retain their old meanings. AppearanceProvider remains the authority for four themes and root input/motion/material/contrast capabilities. Safe areas, IME rules, local fonts and independent interface/card scaling remain.

Preflight is not imported. Prefixing avoids collisions with legacy classes. Utilities intentionally remain in the components layer. The remaining specialized geometry/content owners may override ordinary surface recipes; a global utility-priority or important flag would break that contract. All ordinary feature owners use the new presentation and no longer import superseded generic styles. Partial-border recipes explicitly clear the other edges because the browser defaults are not reset by Preflight. The final precedence review retained that explicit ordering rather than introducing a second reset or overriding measured geometry.

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

## Phase 9: card editor chrome

CardEditorScreenBase, EditorToolbar, formatting selectors, link/image/source controls and template editor chrome now use shared fields, actions, panels, notices and semantic Tailwind. Toolbars keep the existing editor command adapter; shadcn types do not enter Tiptap schemas or durable drafts. ContentEditable descendants, embedded media/math and print behavior remain in RichContent.module.css. Draft asset identities, cloze/template semantics, save/discard guards and citation policy are unchanged.

## Phase 10: Documents and Study Guides

Documents uses shared library/search/actions, metadata panels, progress, cancel/retry and empty/error presentation. The unavailable-platform fallback remains a real supported path. Native/controller jobs survive navigation and their results remain durable.

The guide library and reader use the shared scaffold, toolbar/actions, disclosures, source cards and editable learner-note controls. The reader remains a nonmodal InlineWorkspace; compact contents/article geometry and close-target size are tested. Section-specific source/note/edit/practice controls collapse into one accessible overflow menu, and legacy slash-separated topic inventories are compacted at the display boundary without changing durable learning-topic identity. Document and collection guides reuse the same semantic section presentation. Generated content, learner overrides, revisions and source provenance remain separate from the renderer; local PDF export uses its own print layout with linked navigation, semantic blocks, verified source figures, bounded math, repeated table headers, and wrapped-heading orphan protection.

## Phase 11: Analytics

Filters, deck/time selectors, section hierarchy and charts now sit inside the shared presentation. Reusable MetricCard and distribution presentation also serve Practice completion rather than creating another metric-card implementation. Chart.js, aggregation and historical attribution remain unchanged. Responsive containment and accessible data alternatives remain acceptance requirements.

## Phase 12: active Practice

ActivePractice and PracticeComplete use shared actions, inline progress, status, grading surfaces and completion metrics. The inactive 3D card face is inert. Reveal/grading controls are blocked during card management or session-end operations. The card face geometry and reduced-motion transform behavior retain a specialized stylesheet.

Session windows, snapshots, scheduling inputs, answer reveal, confidence semantics and review/retry identity stay application-owned. Contextual Tutor preserves the originating session and its hidden-answer restrictions.

## Phase 13: Tutor

TutorWorkspace, conversation history, composer, source selection, memory/index disclosures, preparation, citations and error/retry presentation use shared components. Ctrl/Cmd+Enter submission remains composition-aware. A source checklist starts a new scoped conversation without rewriting a previous conversation's scope.

Source permissions, pending operation IDs, provider transport, citations, private evaluation criteria and saved/offline conversations are unchanged. Consent tests address the accessible checkbox state instead of assuming that a Base UI checkbox is an HTMLInputElement. Each test resets its mocked response queue so an earlier assertion failure cannot leave a rejected response queued for another case.

The only raw feature inputs are explicitly disabled checklist markers emitted by the sanitized Markdown renderer. The policy rejects interactive controls, spread props and alternative fields under that exception.

## Phases 14–15: cleanup and enforceable ownership

The remaining ordinary layout, branding, responsive actions, visually hidden utility and InlineWorkspace chrome use Tailwind. Dead Menu/Settings styles and the old Button, Tabs, Surface and feature-exclusive styles have been removed. The unmounted all-in-one DecksScreen and its private styles/tests are retired; live DeckDetailRoute, DeckLibraryWorkspace and DeckTransferDialog are the owners.

Coverage is preserved on those active paths: creation routes to the authoritative new identity; deck-only browsing and direct Practice keep their scope; empty/archived decks cannot practice; archive/restore uses the exact revision and prevents duplicate actions; deletion requires confirmation and retains Undo's absolute deadline; post-commit refresh/navigation errors do not invite another write. Transfer tests cover chooser cancellation, unsupported inputs, retry, duplicate reports, exact export totals, busy-state locking and post-commit recovery. Existing library/controller, browser, Anki, dirty-state and native smoke suites remain.

The legacy classOwnership API and its three shared CSS maps are removed after their last production consumer. The replacement verifier checks explicit CSS Module selectors, complete utilities, conditional/cn usage, and rejects reintroducing the retired API. The production CSS contract discovers the entire source tree rather than maintaining a partial screen list.

`scripts/ui/presentation-policy.mjs` is the checked retained-CSS manifest: every remaining Module must have a specific rationale, exist, and have a production importer. No new unexplained Module is permitted. Generic native controls belong only inside the vendor implementation; public oStudy facades and features cannot grow a second checkbox/button system.

### Retained specialized presentation

| Owner | Rationale |
| --- | --- |
| AppShell | Definite-height native route and scroll containment. |
| RichContent and TutorMarkdown | Rich-content descendants, media/math/print, sanitized Markdown and overflow. |
| PracticeCard | Two-face 3D geometry, inert-face visibility, independent scaling and reduced motion. |
| Five deck-library geometry Modules | Hit testing, scrolling, pointer transforms and insertion destinations. |
| DeviceSync QR frame | Machine-readable quiet zone and literal white contrast. |
| TourProvider | Measured spotlight masking and viewport collision geometry. |
| Artwork, Motion, Materials and experience | Illustration cropping and optional motion/material/effect geometry. |
| Progress | Text contrast split across the filled and unfilled track. |

There are 16 registered Modules. Retaining specialized CSS is intentional, not an unfinished generic screen conversion. Semantic short aliases still used by these owners remain; removing them merely to reduce a count would break supported styling. Owned shadcn source and its provenance are retained; no extra runtime registry or dependency ejection is required.

### Historical human-acceptance boundary

PR #96's source completion and automated qualification did not declare subjective visual approval. Its handoff explicitly assigned four-theme appearance, long/populated/error states, enlarged text, keyboard focus, physical Android IME/Back and real asset/document rendering to human review. The current visual-refresh work intentionally changes those aesthetics, so a new candidate must be reviewed rather than treating the #96 appearance as a pixel specification.

## Tests and acceptance

The TypeScript import verifier restricts vendor/Base UI ownership. The class-ownership check rejects interpolated utility names and unowned legacy classes. The in-memory CSS build discovers all production frontend sources, verifies every used utility and the four themes/capability variants, and rejects raw generic controls outside the approved implementation boundary. It also checks explicit partial-border behavior with no Preflight.

Behavior suites retain form/ref/loading/disabled contracts, keyboard tab/radio behavior, disclosure drafts, Settings/provider/sync operations, atomic import receipts, deck-library drag/selection, browser bulk actions, Practice setup and nested modal/Back behavior. They assert observable outcomes rather than hidden-input implementation details. Node-only suites remain explicit `.node-test.mjs` files.

Android's fresh-library test waits for the asynchronous library surface, not only the earlier route heading. All existing create/import, empty-state and irrelevant-control assertions remain. A Python policy regression locks in that readiness boundary and semantic selectors.

Qualification proceeds focused/static/fast first, then a frozen full Linux and Android candidate. Do not perform opportunistic source/docs edits during terminal qualification. A queued run or an ancestor pass is not a current-head result. Visual redesign acceptance remains separate from behavioral qualification.

Use [local testing and handoff](local-testing-and-handoff.md) for exact checkout, safe data isolation and maintained build/install commands. Revert a UI increment normally to roll back; do not revert unrelated PR #94 work or discard user data.

See the completed [migration roadmap](ui-overhaul-roadmap.md), active [visual refresh roadmap](ui-visual-refresh-roadmap.md), [ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md), [vendor provenance](../../src/ui/vendor/shadcn/README.md), and [bounded publication loop](../agents/GITHUB-DIRECT-DEVELOPMENT.md).
