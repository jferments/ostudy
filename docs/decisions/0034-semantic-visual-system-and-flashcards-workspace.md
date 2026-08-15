# ADR 0034: Semantic visual system and route-backed Flashcards workspace

- **Status:** Accepted
- **Date:** 2026-08-14
- **Scope:** UI modernization phases U0–U9

## Context

The current interface has useful responsive, overlay, platform, and theme foundations, but much of its visual behavior still lives in broad global styles and feature-specific markup. Adding richer depth, motion, artwork, help, folders, or new themes directly to those screens would make the application harder to change and would create inconsistent desktop and mobile behavior.

The Android application introduced safe-area, input-method, lifecycle, restoration, capability, and system-Back boundaries that must remain the platform correctness layer. The UI modernization must consume those boundaries rather than create Android-only screen implementations.

## Decision

ostudy will use a semantic visual system with the following dependency direction:

1. versioned design-token source files define reference scales, semantic roles, and component contracts;
2. a deterministic repository script generates CSS custom properties and TypeScript token names;
3. an `AppearanceProvider` resolves persisted preferences, operating-system media preferences, and platform capabilities into root data attributes;
4. shared primitives own material, elevation, motion, icon, field, progress, help, and artwork behavior;
5. reusable patterns compose primitives into page, workspace, command-bar, filter, sheet, and editor layouts;
6. feature screens provide domain state and semantic intent but do not select raw colors, shadows, blur radii, z-indexes, animation timing, or artwork filenames.

Light, Dark, Console, and Synthwave implement one complete semantic theme contract. A theme can change visual values and artwork variants, but not information architecture, accessibility semantics, touch targets, or domain behavior. Feature components must not branch on a theme name.

The user-facing Decks area becomes a route-backed **Flashcards** workspace with Decks, Templates, Search, Import/Export, and Create Cards destinations. Large workspace destinations mount independently rather than using a tab component that keeps every panel mounted. Stable route identifiers extend the device-local UI checkpoint and participate in the existing overlay-first Android Back order.

Deck organization uses one durable folder level in the first implementation. Folder membership and ordering are native transactional state. Dragging is an enhancement; every move and reorder operation has keyboard and menu alternatives.

Motion and feedback remain semantic. Screens emit events such as review committed, deck moved, or session completed. The experience layer interprets those events as visual feedback, haptics, audio, or no effect according to preferences and capabilities. Completion celebrations are one-shot and must not replay after lifecycle restoration.

Decorative artwork is registered by semantic slot. Turning artwork off never hides user-authored educational images or functional icons. Context help and tutorial steps are registered by stable identifiers rather than DOM selectors.

## Consequences

- New themes can be added without editing feature screens.
- Desktop, tablet, and mobile share the same component and route semantics while using platform-appropriate presentation.
- The legacy global CSS can be removed incrementally as each owner moves into tokens, primitives, patterns, or CSS Modules.
- Visual effects have explicit reduced-motion, reduced-transparency, forced-colors, and low-complexity fallbacks.
- Folder organization changes the durable library model and complete-library backup format, so migrations, import/export, projections, and regression tests must change together.
- The visual system adds up-front structure, but prevents future screen work from accumulating another ungoverned styling layer.

## Validation

Validation includes token generation and completeness checks, repository style policy, component behavior tests, accessibility fixtures, representative browser screenshots, desktop native smoke, and Android emulator coverage for safe areas, Back, input-method behavior, folder movement, practice, completion, and restoration.
