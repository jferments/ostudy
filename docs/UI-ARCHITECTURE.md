# ostudy UI architecture

This document is the implementation guide for the frontend platform introduced by Stage 3.13. ADR 0014 records the durable decision; this document describes how contributors apply it.

## Dependency direction

```text
Rust domain and persistence
        ↓
typed IPC gateways (`src/ipc/`)
        ↓
feature controllers (`src/controllers/`)
        ↓
feature screens/views (`src/screens/`)
        ↓
shared interaction and visual primitives (`src/ui/`)
```

Platform capabilities live beside IPC:

```text
Tauri desktop / future Android / Windows capability adapter
        ↓
`src/platform/`
        ↓
controllers and feature views
```

The arrows are one-way:

- `src/ui/` does not import IPC, feature controllers, or Tauri APIs.
- screens do not call `invoke()` and do not import Tauri dialog/opener plugins.
- controllers may coordinate domain IPC and platform adapters but do not own DOM layout.
- Rust remains authoritative for persistence, validation, schedules, transactions, import trust, and analytics semantics.

## Overlay lifecycle

The root `OverlayProvider` owns the live overlay stack. Dialogs, alert dialogs, compact sheets, and action menus register themselves with their parent overlay identity. Nesting depth, not React effect timing, determines which layer is topmost.

The top overlay alone may consume:

- Escape;
- backdrop dismissal;
- Android/platform Back;
- outside-click menu dismissal.

A modal overlay:

- uses unique accessible title/description IDs;
- moves focus to an explicit autofocus target or the first available control;
- contains forward and reverse Tab navigation;
- restores focus to the element that opened it;
- makes the application root inert and hidden from the accessibility tree;
- locks document scrolling until the final modal closes;
- ignores dismissal while busy or non-dismissible;
- becomes a safe-area-aware bottom sheet on compact screens.

Feature code uses `Dialog`, `Sheet`, `Menu`, the `Modal` compatibility wrapper, or `useConfirm()`. It must not add a custom backdrop, focus trap, `role="dialog"`, or ordinary `window.confirm()` call.

`beforeunload` remains browser-native because a custom React dialog cannot reliably finish while the WebView is closing.

## Platform adapters

`src/platform/` isolates APIs whose behavior changes by operating system or form factor.

Current adapters cover:

- file open/save selection;
- external URL opening;
- platform Back dispatch;
- pointer/hover/hardware capabilities;
- future audio feedback;
- future haptic feedback.

The desktop file adapter currently returns filesystem paths from Tauri. A future Android implementation may stage a content URI into application-owned cache and return a local reference without changing Decks, Settings, Card Editor, or import controllers.

## Styling

`src/main.tsx` imports exactly one global stylesheet:

```text
src/ui/foundations/index.css
```

That file establishes the cascade layers and imports the current semantic base/feature style groups. New shared UI and new feature/component styles use CSS Modules.

Global foundation responsibilities are limited to:

- theme and semantic design tokens;
- reset/base typography;
- document/root sizing;
- safe-area and dynamic-viewport variables;
- reduced-motion behavior;
- accessibility utilities;
- named layer/z-index tokens.

Do not add a new root-level feature stylesheet. Do not use import order as a feature boundary. Do not add arbitrary numeric z-index values to shared UI; use `--layer-*` tokens.

Themes modify semantic token values, not feature selectors. Existing themes are Light, Dark, Console, and Synthwave.

## Responsive layout modes

The interface responds to available width, not operating-system names.

### Wide workspace

- full navigation sidebar;
- multi-column screen layouts where useful;
- optional editor/preview inspector;
- centered dialogs.

### Medium workspace

- compact navigation and collapsed secondary content;
- single-column forms where two columns would become cramped;
- previews move below or into an explicit subview.

### Compact workspace

- bottom navigation for Decks, Search, Practice, and Analytics;
- Templates and Settings in the More sheet;
- single-column content;
- bottom-sheet/full-screen overlay presentation;
- sticky/fixed controls padded above the system safe area;
- touch targets of at least `--touch-target-min`;
- dynamic viewport (`dvh`) sizing so virtual keyboards and mobile browser chrome do not cover controls.

The desktop Tauri window permits compact widths so narrow behavior can be tested without an Android build.

## Shared primitives

Stage 3.13 establishes:

- `Button`;
- `FormField` and `FieldGroup`;
- `Panel`;
- `InlineNotice`;
- `EmptyState` and `LoadingState`;
- `Progress`;
- `SegmentedControl`;
- `Dialog`, `Sheet`, `Menu`, and asynchronous confirmation;
- interaction-safe, safe-area-aware Toast feedback.

Use an existing primitive when it owns the accessibility or responsive behavior you need. Add domain-specific copy and layout in the feature; do not put deck/card/template logic into the primitive.

## Motion, graphics, audio, and haptics

Controllers and views emit semantic feedback events such as:

```text
answer_revealed
review_committed
session_completed
import_completed
operation_failed
```

`ExperienceProvider` may translate those events into optional visual, audio, or haptic channels. Durable operations never depend on an effect and never wait for one to finish.

The effects layer:

- is pointer-transparent;
- uses a named UI layer token;
- does not change layout;
- respects application and OS reduced-motion preferences;
- may be lazy-loaded when larger effects are introduced.

The initial Stage 3.13 audio interface is deliberately a no-op. A future feature stage can add local bundled cues and user volume preferences without putting audio calls into Practice scheduling logic.

## Accessibility and interaction rules

- Every control has an accessible name.
- Icon-only controls need a visible tooltip where useful and an `aria-label` always.
- Information must never rely only on color, sound, motion, or haptics.
- Keyboard shortcuts do not run while an overlay is active or focus is inside an interactive control.
- Touch users must not need hover or long-press for core actions.
- Reordering controls need button/keyboard alternatives even if drag interaction is added later.
- Focus, loading, error, empty, success, and committed-but-not-reconciled states remain explicit.

## Testing matrix

Component tests cover overlay focus/stack behavior, platform Back precedence, menus, primitive ARIA relationships, responsive navigation, interaction-safe toasts, and feature workflows.

Manually and in native smoke testing, check representative widths:

```text
360 × 640
390 × 844
768 × 1024
1024 × 768
1280 × 800
1440 × 900
```

For high-risk screens also check:

- Light, Dark, Console, and Synthwave;
- 80%, 100%, and 140% interface scale;
- reduced motion on and off;
- keyboard-only operation;
- coarse-pointer/touch operation;
- no document-level horizontal overflow;
- dialog/sheet actions remaining above safe areas and virtual keyboards.

Repository policy rejects ordinary browser confirmations, direct platform-plugin imports from features, raw feature dialogs, UI-to-IPC/controller dependencies, historical root stylesheet entry points, and multiple global CSS imports from `main.tsx`.
