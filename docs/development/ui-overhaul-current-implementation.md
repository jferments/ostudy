# UI overhaul implementation after PR #94

Status: implementation in progress on PR #96. The semantic theme adapter and import-boundary checks are implemented; Tailwind build activation and shared-control renderer replacement are the next checkpoint. This is not the completed application-wide redesign.

## Baseline and scope

PR #94 was squash-merged as `fd5659e77671cb814393024c1baa2e0cba9a59d7`, with the same product tree as its qualified final head `34f7b594d0fb618868fb6f5aa020a6a84c043b8f`. Phase 0 assessed the earlier `279430d799b78add3a0885168e011de0b0d47ff8`; implementation preserves the final merged source instead. PR #96 incorporated current main with non-force merge `35ef21cfefc500b7ab47e4f6189e7493de124ab6` and now targets main.

The current acceptance increment covers roadmap Phases 1–3: the Tailwind build foundation, generated semantic theme/capability bridge, and a narrow shared primitive renderer migration. Settings, navigation, all feature workspaces, diagnostics and help remain in the full roadmap; this increment is not their completed redesign.

## Updated migration boundaries

The final Tutor has a multi-source checklist (`TutorSources`), contextual chat settings (`TutorChatSettings`), safe Markdown rendering (`TutorMarkdown`), and conversational questions through Send. Do not restore the removed Quiz me button or require source selection/embeddings for general chat. Source permissions, private question criteria, hint safeguards, operation identities, native requests, and offline conversation reading remain application-owned.

`DocumentJobsProvider` preserves document work across route changes. A renderer must not move that lifetime back into a screen, replace cooperative cancellation with unmounting, or turn durable nonfatal diagnostics into error banners. Preserve the single red stop control for running guide generation.

The overhaul includes error/retry/empty/loading presentation, contextual help and tours, diagnostics/settings surfaces and shell/sidebar chrome, not only the central study screens. Generic interactions belong in the shared UI layer; domain meaning, native services and application-lifetime state do not move into a component vendor.

## Implemented foundation checkpoint

`scripts/ui/tailwind-theme.mjs` defines semantic aliases onto existing oStudy tokens. The existing token generator produces and verifies `src/ui/tokens/generated/tailwind-theme.css` together with its previous artifacts. All four theme contracts remain authoritative. The adapter deliberately does not overwrite short legacy properties: `--muted` currently means text, whereas shadcn's muted utility describes a surface; shadcn's accent surface likewise differs from the legacy brand accent. Utilities resolve directly to application-owned variables with `@theme inline`.

`scripts/verify-ui-boundaries.mjs` uses the existing TypeScript parser to inspect literal imports, re-exports, type imports, import-equals and dynamic import/require calls. Base UI imports belong only in `src/ui/vendor/shadcn`. Vendor imports belong only in that implementation directory and oStudy's primitive, overlay and layout adapters. Features consume the public oStudy components. Normalized relative paths and the `@/` alias cannot bypass the directory boundary.

`check-static.sh` runs the theme/policy regression tests and the import verifier before expensive qualification. The parser-dependent verifier runs after frozen dependency installation. Ten focused Node tests cover theme resolution, alias collisions, invalid mappings, variable normalization and import ownership.

This checkpoint does not yet import the generated Tailwind stylesheet into the application, install a dependency, enable Preflight, or replace a visible control. Keeping it inactive until the build and renderer changes land avoids accidentally changing every existing screen through CSS layer/reset effects.

## Acceptance for the current implementation increment

- AC-08: Integrate the exact merged PR #94 tree into PR #96 without losing its planning documents or rewriting shared history; target main afterward.
- AC-09: Add pinned Tailwind/Vite tooling and a reviewed dependency graph without enabling a second reset or changing unrelated dependencies.
- AC-10: Generate the semantic theme bridge from existing oStudy token authority, preserving all four themes and avoiding existing short-name collisions. Runtime theme/capability proof follows build activation.
- AC-11: Enforce vendor/Base UI import ownership before screen migration spreads third-party dependencies.
- AC-12: Migrate a narrow, real shared primitive boundary with preserved button, loading, accessibility, theme and touch semantics; record exactly which primitives changed.
- AC-13: Add focused regression coverage, reconcile maintained documentation, and qualify the exact published candidate. Physical-device/visual acceptance and unrun gates must be stated separately.

## Next implementation boundary

Activate Tailwind through the reviewed Vite integration, with a complete frozen lockfile and no duplicate reset. Prove the generated theme on actual shared controls behind existing oStudy APIs, starting with Button and other low-risk primitives. Record upstream source provenance and local adaptations: shadcn source copies require reviewed updates; installing a package does not automatically maintain customized copies. Update the existing style/contrast ownership checks alongside their replacement contracts, rather than disabling those checks.

Preserve native element/ref/form semantics, disabled/loading behavior, accessible names, theme contrast, coarse-pointer target sizes and reduced-motion/forced-color behavior. Do not expand into overlays, shell, Settings or Tutor layout before the foundation compiles and its focused tests pass.

## Validation and rollout

Use focused tests and static/fast qualification during implementation. Finish source, documentation and handoff review before terminal full/Linux/Android qualification. Do not update visual baselines merely to conceal a functional regression. Preserve the current native data model, schemas, imports, providers, scheduling and content-rendering safety.

Auxiliary source checks are not a substitute for pinned-toolchain GitHub Actions qualification. Record exact tested SHAs and current results in the PR evidence, not as timeless claims here. A passed ancestor Check is historical evidence, not qualification of a newly published candidate.

The detailed sequence remains in [the overhaul roadmap](ui-overhaul-roadmap.md); [ADR 0051](../decisions/0051-shadcn-tailwind-presentation-architecture.md) owns the renderer boundary. Original Phase 0 acceptance is a historical documentation-only increment, not a restriction against the implementation now requested in PR #96.
