# Frontend and CSS ownership refactor checkpoint

This is the durable completion checkpoint for ADR 0048 after PR #75 and Stage 8 in PR #76.

## Repository lineage

- PR #75 (`refactor(ui): complete explicit frontend and CSS ownership`) was squash-merged to `main` as `b62206bfe79d2afd081cdc2420d5a89abfb17b36`.
- Stage 8 is implemented on `agent/stage8-zero-legacy-css` in draft PR #76.
- PR #76 starts exactly from the PR #75 squash merge; it does not reuse the old `scratch/pr75-stage8-materialize` payload branch.
- The Stage 8 work does not change learner-data schema, IPC semantics, scheduling, document/study-guide semantics, analytics semantics, backup format, or persisted appearance settings.

## Bounded Stage 8 checkpoints

### Standalone desktop artifact boundary

Commit `717029868d96493af4ebe46a49abda4004f4549b` separates Cargo/Tauri's mutable debug output from the exact standalone executable used for native acceptance and manual launch.

The accepted executable is staged under the configured Cargo target's `standalone/ostudy` path, hashed in `.ostudy-build-stamp`, and used by both native WebDriver smoke and `./run-app.sh`.

### Feature-global stylesheet retirement

Commit `9cf1937a8e19b6b5f09939c3624753840f996c60` is the qualified feature-retirement checkpoint. It removes the global feature stylesheets, `DecksScreen.stage12.css`, and `CardListing.css` in favor of explicit feature/shared module owners. Exact-head static and fast qualification passed at this checkpoint.

### Zero-legacy topology

Commit `2e642844e66059d216c07e4996a2c6c0015b4720` deletes the remaining `application.css`/`src/styles` tree, removes the `legacy` layer, normalizes the cascade, switches the style manifest to `zero`, requires zero temporary policy allowances, and strengthens completion assertions. Exact-head static and fast qualification passed at this checkpoint.

### Compatibility-alias removal

Commit `cffe1b6dd9a24f33ba034cc9be45e3945b5ab865` removes the temporary `data-animations-enabled` and `data-reduce-motion` DOM attributes. `data-motion` is the canonical motion presentation contract. Exact-head static and fast qualification passed at this checkpoint.

## Final architecture contract

The completed source architecture requires all of the following simultaneously:

```text
src/main.tsx
└── ./ui/foundations/index.css     only global TypeScript CSS import
```

```css
@layer reset, tokens, theme, base, components, patterns, features, utilities;
```

- `src/styles/base` and `src/styles/features` do not exist.
- Authored global CSS exists only in `src/ui/foundations/`.
- Shared and feature presentation uses CSS Modules.
- `scripts/style-ownership-baseline.json` uses `mode: "zero"`, `legacyFiles: {}`, and `policyAllowances: {}`.
- The topology verifier rejects reintroduction of legacy/global feature ownership and raw authored visual debt.
- The TypeScript AST verifier rejects raw/unowned production class semantics and validates semantic keys used by `ownedClassNames(...)` against imported module owners.
- QR quiet-zone white is represented by generated reference token `color.neutral.0`, and the QR verifier pins that token source to `#ffffff`.
- Motion/material/layout/platform behavior is expressed through canonical semantic datasets/tokens rather than retired compatibility aliases.

## Final qualification boundary

The maintained source documents are completed **before** the expensive final qualification. Final evidence is recorded in PR #76 and its exact-head workflow results rather than by modifying this file afterward, because any post-test source edit would create a different unqualified SHA.

Before PR #76 is merge-ready, its exact final head must pass:

1. static qualification;
2. fast qualification;
3. authoritative Linux/Tauri full qualification, including startup and native WebDriver smoke;
4. Android compile/package qualification;
5. API-36 emulator smoke.

The final staged standalone artifact must then be launched through `./run-app.sh` for the human acceptance pass.

The remaining human-required matrix covers:

- Light, Dark, Console, and Synthwave;
- compact, medium, and wide representative widths;
- 80%, 100%, and 140% interface scale;
- full, reduced, and no-motion;
- rich and simple materials;
- keyboard navigation and focus restoration;
- pointer/touch alternatives;
- safe-area and IME behavior;
- representative long-content/horizontal-overflow paths.

The PR remains draft until automated qualification is green. Manual visual/accessibility acceptance remains explicitly human-required and is never inferred from automated success.
