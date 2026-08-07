# ADR 0014: UI platform, overlays, scoped styling, and responsive interaction

- **Status:** Accepted
- **Date:** 2026-08-07
- **Scope:** Stage 3.13

## Context

Stage 3.12 separated native IPC by domain, distinguished editable source notes from rendered presentations, moved workflow coordination into controllers, and standardized stale-read handling. The remaining frontend infrastructure still relies on global feature stylesheets, ad-hoc modal behavior, browser confirmation dialogs, desktop-oriented navigation, and direct imports of platform-specific Tauri plugins from feature code.

This creates several risks before Android/Windows ports and AI-assisted workflows:

- dialogs do not consistently own focus, stacking, Escape/back behavior, background inertness, or focus restoration;
- global selectors and stylesheet order allow one feature to change another feature's buttons, inputs, dialogs, or responsive behavior;
- file selection, external links, hardware back, audio, and haptics have no platform abstraction;
- narrow layouts are treated as late media-query overrides rather than first-class application modes;
- future animations and sounds would otherwise be coupled directly to Practice business logic;
- duplicated visual controls make accessibility and touch-target behavior inconsistent.

## Decision

### Dependency direction

The frontend dependency order is:

```text
Rust domain/backend
        ↓
typed IPC gateways
        ↓
feature controllers
        ↓
feature views
        ↓
shared UI primitives
```

Platform capabilities live beside IPC under `src/platform/`. UI primitives may not import IPC, Tauri plugins, feature controllers, or domain DTOs. Controllers may coordinate IPC and platform adapters but expose plain view state and callbacks.

### Overlay lifecycle

The application owns one overlay host and stack. Shared Dialog, ConfirmDialog, Sheet, Popover, and Menu primitives own:

- unique accessible labels/descriptions;
- initial focus, focus containment, and focus restoration;
- topmost-only Escape and platform-back dismissal;
- backdrop handling and busy/non-dismissible states;
- modal background inertness and scroll locking;
- safe-area-aware compact presentation.

Feature code does not hand-write `role="dialog"`, backdrops, focus traps, or ordinary `window.confirm()` calls. Browser `beforeunload` remains native because custom React UI cannot reliably complete during document shutdown.

### Platform capabilities

Feature code uses typed adapters for file selection, export destinations, external links, back navigation, audio, and haptics. Desktop adapters may wrap Tauri plugins. Future Android adapters may stage content URIs or map semantic feedback to native capabilities without changing feature controllers.

### Styling

Only foundation styles are global. They define reset, semantic design tokens, themes, typography, motion preferences, safe-area variables, accessibility helpers, and cascade layers. Feature and component styles use CSS Modules. Historical stage/feature stylesheet bundles are removed once their rules are migrated.

Themes change token values rather than feature selectors. Arbitrary z-index values are replaced by named layer tokens. Effective reduced motion is the application preference OR the operating-system preference.

### Responsive layouts

The UI supports three width-driven modes:

- wide workspace: full sidebar and optional secondary inspector;
- medium workspace: compact navigation rail and collapsible secondary content;
- compact workspace: bottom navigation, sheets/full-screen dialogs, single-column content, and safe-area-aware sticky actions.

Layout behavior is based on available space rather than operating-system names. Touch/coarse-pointer capabilities affect hit targets and hover assumptions, not domain behavior.

### Shared primitives

Buttons, form fields, selection controls, panels, notices, empty/loading states, progress, navigation, menus, dialogs, and toasts use shared primitives. Features compose these primitives but retain domain-specific copy and workflows.

### Future experience effects

Controllers emit semantic feedback events such as `answer_revealed`, `review_committed`, and `session_completed`. An ExperienceProvider may map those events to optional visual, audio, or haptic effects. Effects are non-blocking, respect reduced motion/preferences, live in a pointer-transparent effects layer, and never determine durable success.

## Consequences

- modal accessibility and platform Back behavior are implemented once;
- feature CSS cannot silently override unrelated screens;
- desktop and mobile layouts share the same controllers and domain contracts;
- Android/Windows adapters can change native capability handling without rewriting screens;
- future AI inspectors and feedback effects plug into explicit workspace/event seams;
- new UI work must use existing primitives or extend them with tests rather than adding ad-hoc interaction infrastructure.

## Non-goals

Stage 3.13 does not add AI generation, a database migration, scheduling or analytics semantic changes, Redux/Zustand, a styled component framework, a production sound pack, a complete animation suite, or a platform-specific feature implementation beyond capability adapters.

## Validation

The stage is protected by overlay lifecycle tests, primitive accessibility tests, compact/wide layout tests, CSS/platform policy checks, existing screen/controller tests, the native WebDriver workflow, and the full exact-head qualification gate. A behavior-preserving UI refactor is complete only when keyboard, touch, reduced-motion, and compact layout paths remain usable across all themes.