# Owned shadcn components

Upstream: shadcn-ui/ui commit `3ba91b1cc83e1bbe4ab35a422ff2a694849c5048`.
License: MIT; the distributed notice is [SHADCN-MIT.txt](../../../../public/licenses/SHADCN-MIT.txt).

The initial components are adapted from `apps/v4/registry/bases/base/ui/button.tsx`,
`badge.tsx`, and `skeleton.tsx`. This directory is the only Base UI import owner.
Feature callers use the oStudy facades, not these files or vendor prop types.

## Deliberate adaptations

- Button retains upstream Base UI behavior and CVA composition. Fully expanded
  semantic utility strings replace the registry's `cn-button-*` style placeholders.
  `tw:` is the configured Tailwind prefix. oStudy supplies colors, radii, motion,
  font, focus treatment and touch dimensions rather than an upstream palette.
- The existing destructive-outline, destructive-ghost and back appearances are
  represented internally so the public Button API does not change.
- Badge retains Base UI mergeProps/useRender composition; oStudy status tones
  supply its semantic variants. Badges remain spans, not buttons or live regions.
- Skeleton remains a span to preserve existing callers. Its pulse uses the
  existing oStudy animation duration and is disabled for reduced/no-motion states.
- Utility composition uses the conventional clsx/tailwind-merge combination,
  configured for the prefix and semantic font sizes, behind `ui/internal/cn`.

## Updating

These are source copies, not a package that automatically updates our customized
components. Compare a pinned upstream revision, review behavior and dependency
changes, then port the relevant changes. Never run an unreviewed CLI overwrite
against the owned source. Record the new revision and local adaptations here.
`components.json` describes the Base UI style family, aliases and prefix; no CLI,
remote registry, CDN, hosted font or provider is required when the app runs.

Use the primitive behavior tests and CSS compilation contract before wider UI
qualification. Primitive utilities currently compile into the existing components
layer, allowing unmigrated feature CSS to retain its override precedence. Remove
that temporary placement only after auditing caller overrides, not by globally
forcing `!important`.
