# UI overhaul implementation after PR #94

Status: active implementation of roadmap Phases 4–8 in PR #96. The application-wide overhaul is not complete. Qualification results belong to exact-SHA PR evidence, not to this document.

## Preserved baseline

PR #94 was squash-merged as `fd5659e77671cb814393024c1baa2e0cba9a59d7`. PR #96 incorporates that final product tree and targets main. The original Phase 0 assessment used the earlier `279430d...` source and remains historical planning evidence.

The final Tutor supports multi-source selection, contextual chat settings, safe Markdown and conversational questions through Send. Source-free general chat must not require embeddings. Keep hidden-answer/citation restrictions, source permissions, operation identities, private quiz criteria and offline reading. Do not restore Quiz me. `DocumentJobsProvider` retains document work across navigation. Provider transport, native credentials, IPC, scheduling, persistence and document-job implementation remain outside this presentation migration.

## Foundation: Phases 1–3

Pinned Tailwind/Vite integration compiles static CSS with no browser compiler, CDN or runtime registry. `components.json` defines the Base UI family and the namespaced utility prefix. TypeScript/Vite aliases preserve the profiling build. The token generator owns the semantic Tailwind theme: utilities resolve to oStudy variables, the default color palette is disabled, and legacy muted/accent aliases retain their meanings. AppearanceProvider remains the single authority for all four themes and input, motion, material and contrast capabilities.

Preflight is not imported. Utilities compile in the existing components layer during migration so unmigrated callers retain their explicit CSS overrides. Migrated Settings layouts no longer apply their superseded CSS Modules over the new controls. Do not conceal override conflicts with global important styling.

Button, Badge, Skeleton, TextField and NativeSelect use owned shadcn composition. NativeSelect deliberately retains the operating-system picker, native options/optgroups, form values and listbox semantics. Button preserves the action name while loading and prevents duplicate disabled/busy activation. Superseded Button and Badge/Skeleton-exclusive styling is removed.

## Phase 4: forms and Settings

Shared Checkbox, Switch, RadioGroup, Slider, Textarea and Disclosure facades now use shadcn/Base UI composition. Tabs and segmented controls delegate ordinary composite navigation to Base UI. The segmented facade retains oStudy's Home/End shortcuts, which Base UI intentionally does not provide for ordinary radio groups. Tests wait for deferred focus and assert actual focus, selection and panel changes rather than requiring synchronous internal timing.

Surface and Panel use shadcn Card composition. FormField owns label/error/help association; a contextual-help action is not nested inside an input label. Disclosure content stays mounted to preserve unsaved draft fields.

### Visibly migrated Settings areas

- Appearance uses selectable theme-preview cards, real shared controls in its visual preview, grouped typography/scaling controls, and card-based motion/artwork settings.
- Review/Scheduling uses shared sliders, number fields, checkboxes and panels while preserving retention bounds, workload semantics and reveal preferences.
- AI task models are three responsive cards with readable stacked fields, rather than the former cramped horizontal rows. Provider connections have their own selection cards, setup panel and advanced disclosure. Secret handling, independent assignments, probes, reasoning compatibility and retry identities remain controller-owned.
- Diagnostics uses shared controls and separate session/state/privacy panels, with wrapping paths and the same native configuration operations.
- Library safety, backup and storage information use shared panels and controls. The inner device synchronization presentation is the next Settings/transfer slice; its transport, scope and credential rules are unchanged.

Old AI/provider/model and appearance-preview CSS Modules are removed. The shared Settings stylesheet remains only while the inner DeviceSync component still consumes it. The template authoring workspace is still pending; its Tiptap content model is not part of a generic form migration. These remaining owners are not counted as completed Phase 4.

## Tests and ownership

The TypeScript-parser verifier limits Base UI imports to the vendor directory and vendor imports to approved oStudy adapters. The class-ownership verifier accepts complete prefixed utilities and rejects unowned legacy classes or interpolated utility names. A production CSS compilation test covers the migrated renderers and Settings sources, all themes and root capability variants without writing source.

Behavior coverage includes form submission, DOM refs, typing/clearing, password visibility, native selection/listboxes, disabled/loading names, checkbox values, radio navigation, slider keyboard changes and disclosure draft preservation. Settings-controls regressions cover installed-font selection/editing, bounded numeric workload values and visible slider state. Existing provider, backup and diagnostics integration suites remain in qualification.

Node-only suites use `.node-test.mjs` and run explicitly under Node rather than being mistaken for Vitest suites. Font packaging verification permits equivalent minified CSS while retaining the full fallback stack, offline assets and licenses. Source copies retain their [upstream provenance and update policy](../../src/ui/vendor/shadcn/README.md).

## Remaining requested scope

Finish DeviceSync and generic template-management controls, then migrate overlays and their shared Back/focus policy (Phase 5), transient/error/status UI (Phase 6), shell/sidebar/mobile navigation (Phase 7), and Flashcards library/search/filter/bulk/transfer/practice-setup presentation (Phase 8). Preserve drag geometry, selection, navigation checkpoints and transactional mutations. Help and diagnostics remain explicit consumers of the common presentation system.

Phases 9–15 remain the later editor, Documents/Guides, Analytics, Practice, Tutor and final CSS/stabilization work except where shared components naturally affect them. A changed shared button does not mean an entire workspace is migrated.

Use focused/static/fast checks during implementation. Finish source and documentation before terminal Linux/Android qualification. Visual acceptance must inspect all themes, keyboard focus, compact touch targets, enlarged text, native pickers and actual IME/Back behavior. No measured speedup or real-world defect-rate reduction is implied by adoption of a library. Rollback is a normal UI-source revert preserving merged PR #94 and stored data.

See [the roadmap](ui-overhaul-roadmap.md), [ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md), and [the bounded publication loop](../agents/GITHUB-DIRECT-DEVELOPMENT.md).
