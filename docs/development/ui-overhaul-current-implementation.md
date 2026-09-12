# UI overhaul implementation after PR #94

Status: the Tailwind foundation, initial shared controls and first AI/provider form slice are implemented in PR #96. The application-wide overhaul is not complete. Qualification results belong to exact-SHA PR evidence, not to this document.

## Preserved baseline

PR #94 was squash-merged as `fd5659e77671cb814393024c1baa2e0cba9a59d7`.
PR #96 incorporated that final product tree with non-force merge
`35ef21cfefc500b7ab47e4f6189e7493de124ab6` and targets main. The original Phase 0
assessment used the earlier `279430d...` source and is historical evidence.

The final Tutor has multi-source selection, contextual chat settings, safe
Markdown and conversational questions through Send. Source-free general chat
must not require embeddings. Keep hidden-answer/citation restrictions, source
permissions, operation identities, private quiz criteria and offline reading.
Do not restore the removed Quiz me button. `DocumentJobsProvider` retains document
work across navigation; stop/cancel and nonfatal diagnostics keep their meaning.
No native, provider, IPC, scheduler, persistence or document-job implementation
changes are part of these renderer increments.

## Foundation: roadmap Phases 1–3

The official `@tailwindcss/vite` integration compiles static CSS. The build has
exact pins and a complete lock graph; no browser compiler, CDN or runtime
registry is used. `components.json` defines the Base UI family and the `tw`
prefix. TypeScript/Vite aliases agree and preserve the profiling build alias.

The token generator owns `tailwind-theme.css`. Color utilities resolve directly
to oStudy semantic variables; the default Tailwind color palette is disabled.
Legacy `--muted` and `--accent` keep their old meanings. AppearanceProvider remains
the theme/capability authority. Root capability variants support coarse input,
reduced/no motion, no hover, simple materials and forced contrast. The existing
four themes, platform insets and IME rules remain in place.

Preflight is not imported. Prefixed utilities prevent collisions with existing
classes. Utilities initially compile in the existing components layer, not above
all feature CSS: this preserves caller-owned CSS Module overrides during the
mixed-renderer stage. Moving them to the final utilities layer requires a later
caller-override audit. Do not use a global important flag to paper over that issue.

### Migrated production components

| Public oStudy component | Implementation | Preserved contract |
| --- | --- | --- |
| `Button` | shadcn Base UI Button + CVA | Existing variants/sizes, native button/form behavior, DOM ref, ARIA/data props, disabled/loading state, caller classes |
| `Badge` | shadcn Base UI useRender/mergeProps | Span semantics, six status tones and arbitrary text |
| `Skeleton` | shadcn Skeleton | Decorative span, dynamic dimensions/radius, reduced/no-motion behavior |
| `TextField` | shadcn Base UI Input | Native input props/ref/change events, password visibility, FormField label/error associations |
| `NativeSelect` | shadcn native-select composition | Operating-system picker, native options/optgroups, refs/events, form values, multiple selection and numeric listbox size |

Button loading retains the action's accessible name instead of replacing it with
an anonymous Working label. Disabled/loading controls cannot repeat an action.
Focus uses a visible outline, and coarse-pointer targets retain their minimum.
The old Button stylesheet and Badge/Skeleton-exclusive style rules are removed;
Chip's still-live shared status styles remain.

The source provenance and deliberate adaptations are recorded in
[the vendor directory](../../src/ui/vendor/shadcn/README.md). Customized shadcn
source copies need reviewed upstream refreshes; they do not update automatically.

## Phase 4: first AI/provider Settings slice

`AiLlmSettings` now uses shared Button actions (including provider selection and
password visibility), TextField text/URL/password controls, and NativeSelect
provider/timeout/authentication controls. `DefaultAiModelSettings` uses the same
selectors for provider, model, reasoning and embedding input policy, plus
TextField for optional embedding dimensions. Native selection semantics avoid
changing existing model-specific assignment/probe tests or mobile picker behavior.

Controllers still own provider/model separation, dirty state, saved credentials,
connection/probe results, reasoning compatibility, retries and operation IDs.
The wrapper never reads saved API keys or contacts providers. The provider tile
keeps its multi-line content inside the shared Button rather than adding another
button implementation.

This is a partial Settings migration: checkbox/radio inputs, disclosures, panels,
notices and screen layout remain with their existing owners until their shared
replacements are ready. No claim is made that all Settings or Phase 4 is finished.

## Tests and enforcement

The TypeScript-parser import verifier limits Base UI to the vendor implementation
and vendor imports to primitive/overlay/layout adapters. The existing ownership
verifier accepts complete namespaced Tailwind utilities while continuing to reject
unowned old classes and interpolated utility names, including inside `cn`.
Theme foreground checks inspect the actual renderer/mapping rather than a deleted
stylesheet. The root organization contract recognizes the shadcn configuration.

The ten Node theme/import-policy tests run explicitly in static qualification as
`.node-test.mjs` files. Their previous `.test.mjs` names were also discovered by
Vitest, producing two empty-suite failures despite passing under Node; the naming
separation fixes that collision without disabling any tests.

Vitest coverage exercises Button form/ref/click/disabled/loading/name semantics,
public variants, Badge/Skeleton contracts and prefixed class composition. Field
tests cover typing/clearing, FormField ARIA/required/ref behavior, password type
changes, native select submission/optgroups and disabled listboxes. Existing
provider/model integration tests continue exercising the migrated screens.

A production Vite/Tailwind CSS test builds in memory, checks every utility used by
the migrated renderers and AI settings slice, all four themes and capability
variants, and verifies that Tailwind directives are compiled away. This is CSS
compilation coverage, not visual or physical-device acceptance.

## Acceptance state and remaining scope

AC-08 preserves merged PR #94 and the main base. AC-09 supplies pinned build
integration. AC-10 supplies the generated theme/capability bridge. AC-11 enforces
vendor ownership. AC-12 migrates Button, Badge and Skeleton. AC-13 tracks tests,
documentation and qualification. AC-14 persists the bounded publication/repair
loop in the canonical GitHub-direct procedure. AC-15 covers only the first
AI/provider native-field vertical slice above. No broader phase is implicitly
marked complete by these increments.

Continue Phase 4 with remaining generic form controls and Settings layouts.
Overlay primitives must be stable before replacing their focus/Back stack.
Later phases cover sidebar/navigation, Flashcards/Search, editor,
Documents/Guides, Analytics, Practice, Tutor, contextual help, diagnostics, errors
and transient feedback. Progress (including its domain-specific inline content),
Chip, Surface and other unmigrated primitives keep their existing implementation.

Use focused/static/fast checks while iterating. Finish source/docs/handoff review
before terminal Linux/Android qualification. Visual acceptance must inspect all
themes, keyboard focus, compact touch targets, enlarged text and preserved screen
layout. Source rollback is a normal revert of the UI increments, not a revert of
the unrelated Tutor/provider baseline. No stored-data format is changed.

See [the roadmap](ui-overhaul-roadmap.md),
[ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md) and
[the bounded publication loop](../agents/GITHUB-DIRECT-DEVELOPMENT.md).
