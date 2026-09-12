# UI overhaul implementation after PR #94

Status: implementation in progress on PR #96. This record does not claim that the planned renderer conversion is complete.

## Baseline and scope

PR #94 was squash-merged as `fd5659e77671cb814393024c1baa2e0cba9a59d7`, with the same product tree as its qualified final head `34f7b594d0fb618868fb6f5aa020a6a84c043b8f`. Phase 0 assessed the earlier `279430d799b78add3a0885168e011de0b0d47ff8`; implementation must preserve the final merged source instead.

The next acceptance increment covers roadmap Phases 1–3: the Tailwind build foundation, generated semantic theme/capability bridge, and a narrow shared primitive renderer migration. Settings, navigation, all feature workspaces, diagnostics and help remain in the full roadmap; this increment is not their completed redesign.

## Updated migration boundaries

The final Tutor has a multi-source checklist (`TutorSources`), contextual chat settings (`TutorChatSettings`), safe Markdown rendering (`TutorMarkdown`), and conversational questions through Send. Do not restore the removed Quiz me button or require source selection/embeddings for general chat. Source permissions, private question criteria, hint safeguards, operation identities, native requests, and offline conversation reading remain application-owned.

`DocumentJobsProvider` preserves document work across route changes. A renderer must not move that lifetime back into a screen, replace cooperative cancellation with unmounting, or turn durable nonfatal diagnostics into error banners. Preserve the single red stop control for running guide generation.

## Acceptance for the next implementation increment

- AC-08: Integrate the exact merged PR #94 tree into PR #96 without losing its planning documents or rewriting shared history; target main afterward.
- AC-09: Add pinned Tailwind/Vite tooling and a reviewed dependency graph without enabling a second reset or changing unrelated dependencies.
- AC-10: Generate the semantic theme bridge from existing oStudy token authority, preserving all four themes and avoiding existing short-name collisions.
- AC-11: Enforce vendor/Base UI import ownership before screen migration spreads third-party dependencies.
- AC-12: Migrate a narrow, real shared primitive boundary with preserved button, loading, accessibility, theme and touch semantics; record exactly which primitives changed.
- AC-13: Add focused regression coverage, reconcile maintained documentation, and qualify the exact published candidate. Physical-device/visual acceptance and unrun gates must be stated separately.

## Validation and rollout

Use focused tests and static/fast qualification during implementation. Finish source, documentation and handoff review before terminal full/Linux/Android qualification. Do not update visual baselines merely to conceal a functional regression. Preserve the current native data model, schemas, imports, providers, scheduling and content-rendering safety.

The detailed sequence remains in [the overhaul roadmap](ui-overhaul-roadmap.md); [ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md) owns the renderer boundary. Original Phase 0 acceptance is a historical documentation-only increment, not a restriction against the implementation now requested in PR #96.
