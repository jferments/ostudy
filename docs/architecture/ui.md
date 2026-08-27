# UI architecture

ostudy uses one shared interaction and visual foundation across desktop widths and future mobile targets. Feature screens own domain-specific copy and layout; shared UI owns reusable accessibility, focus, overlay, navigation, and feedback behavior. The purpose is to avoid rebuilding a slightly different dialog, tab system, or compact-navigation rule in every feature.

## Dependency direction

```text
Rust domain and persistence
        ↓
typed IPC gateways (`src/ipc/`)
        ↓
feature controllers (`src/controllers/`)
        ↓
feature screens (`src/screens/`)
        ↓
shared UI primitives (`src/ui/`)
```

Platform capabilities sit beside IPC:

```text
Tauri desktop or future mobile adapter
        ↓
`src/platform/`
        ↓
controllers and feature screens
```

The dependency rules are one-way:

- shared UI does not import IPC, feature controllers, or Tauri APIs;
- screens do not call `invoke()` or import dialog/opener plugins directly;
- controllers may coordinate typed IPC and platform adapters but do not own DOM layout;
- Rust remains authoritative for validation, storage, schedules, transactions, import trust, and analytics semantics.

Repository policy checks these boundaries mechanically.

## Overlay lifecycle

The root `OverlayProvider` owns the live stack of dialogs, alert dialogs, sheets, and menus. An overlay registers its parent identity, so nesting depth rather than incidental React effect timing determines which layer is topmost.

Only the top overlay may consume:

- Escape;
- backdrop dismissal;
- Android/platform Back;
- outside-click menu dismissal.

A modal overlay:

- has unique, stable accessible title and description relationships;
- moves focus to an explicit autofocus target or the first usable control;
- contains forward and reverse Tab navigation;
- restores focus to the element that opened it;
- makes the application background inert and hidden from the accessibility tree;
- locks document scrolling until the final modal closes;
- refuses dismissal while a critical operation is busy or explicitly non-dismissible;
- presents as a safe-area-aware sheet on compact screens when appropriate.

Features use `Dialog`, `Sheet`, `Menu`, the compatibility `Modal` wrapper, or the asynchronous confirmation service. They do not hand-write backdrops, focus traps, `role="dialog"`, or ordinary `window.confirm()` calls.

Browser-native `beforeunload` remains an exception because a custom React overlay cannot reliably finish while the WebView is closing. Long-running document close protection is also enforced natively, where application shutdown can actually be prevented.

## Platform adapters

`src/platform/` owns capabilities whose implementation changes by operating system or form factor:

- file open/save selection;
- external URL opening;
- platform Back dispatch;
- pointer, hover, and hardware capability detection;
- optional local audio feedback;
- optional haptic feedback.

Desktop file selection currently returns native filesystem paths. A future Android adapter may stage a content URI into application-owned cache and return an ostudy-owned reference without changing Decks, Settings, Documents, or import controllers.

Provider networking and credential storage are native security boundaries, not frontend platform adapters. A feature cannot use the platform layer to bypass the typed AI task surface.

## Styling and themes

`src/main.tsx` imports exactly one global stylesheet:

```text
src/ui/foundations/index.css
```

That foundation establishes cascade layers and imports the semantic base groups. New shared or feature styles use CSS Modules rather than adding another root-level stylesheet whose behavior depends on import order.

Global foundations are limited to:

- theme and semantic design tokens;
- reset and base typography;
- root/document sizing;
- safe-area and dynamic-viewport variables;
- reduced-motion behavior;
- accessibility utilities;
- named layer and z-index tokens.

Themes change token values, not feature selectors. Current themes are Light, Dark, Console, and Synthwave. A feature should use semantic tokens such as surface, text, border, emphasis, success, warning, and danger rather than hard-coding a theme-specific color.

Shared UI does not use arbitrary numeric z-index values. Named layer tokens keep toasts, navigation, effects, menus, sheets, and dialogs in one reviewable ordering model.

## Responsive layout modes

Layout responds to available space and input capability rather than assuming that an operating-system name determines the interface.

### Wide workspace

- full navigation sidebar;
- multi-column layouts where the content benefits;
- optional editor/preview or inspector regions;
- centered dialogs.

### Medium workspace

- compact navigation and collapsed secondary regions;
- single-column forms where two columns become cramped;
- previews below the editor or behind an explicit subview.

### Compact workspace

- bottom navigation for the most frequent destinations;
- secondary destinations in a More sheet;
- single-column content;
- bottom-sheet or full-screen overlay presentation;
- sticky controls padded above system safe areas;
- coarse-pointer targets of at least the semantic minimum size;
- dynamic viewport units so virtual keyboards and browser chrome do not cover actions.

The desktop Tauri window allows compact widths so responsive behavior can be exercised before mobile projects exist.

### Application destinations

Visible application destinations use one typed route catalog. Wide and medium
navigation is ordered **Flashcards**, **Study Guides**, **Documents**,
**Analytics**, and **Settings**. Practice is an internal rendered workflow
entered through contextual study actions; it is not a standing navigation
destination and therefore does not select an unrelated visible destination
while a session is active.

Compact navigation is a deterministic partition of the same visible catalog.
**Flashcards**, **Study Guides**, and **Analytics** are direct destinations;
**Documents** and **Settings** appear exactly once in **More**. The direct and
More groups preserve their relative visible order, are disjoint, and together
cover every visible destination. More is active only for one of its actual
destinations. Shared overlay behavior owns the More sheet's focus, dismissal,
and restoration semantics.

The Study Guides route currently composes the existing guide library. Its
route-owned controller/detail state and checkpoint support are later workspace
work; until that contract is added, generic navigation and lifecycle checkpoint
writes omit Study Guides rather than widening the native checkpoint format.

## Shared primitives

The common layer includes:

- `Button`;
- `FormField` and `FieldGroup`;
- `Panel`;
- `InlineNotice`;
- `EmptyState` and `LoadingState`;
- `Progress`;
- `SegmentedControl`;
- tabs with roving keyboard focus;
- `Dialog`, `Sheet`, `Menu`, and asynchronous confirmation;
- `OverflowMenu`, `ResponsiveActionGroup`, `SelectionToolbar`, and
  `MetadataDialog` / `InfoButton`;
- interaction-safe, safe-area-aware toast feedback.

Use a shared primitive when it owns the behavior being solved. Domain-specific rules remain in the feature. A reusable Button may know how to render loading and disabled state; it must not know how a card review commits.

### Action priority

Action density follows one shared priority contract. Responsive presentation may change an action's placement, but it must not change its meaning, accessible name, authorization, or domain callback.

1. **Primary text action.** The ordinary next step for the current surface remains a visible text action. A compact layout or overflow menu must not hide the primary path or require menu exploration before a learner can continue.
2. **Familiar icon action.** A standard, unambiguous action such as edit, delete, information, schedule, or full-screen may use `IconButton`. It requires a nonblank accessible label, visible keyboard focus, and a hover/focus tooltip; tooltip text is supplemental and never the only accessible name. Disabled controls keep a complete accessible label even though native disabled buttons cannot receive keyboard focus.
3. **Contextual selection action.** A bulk action appears only while a selection is active. Its container identifies the selected count, keeps the valid primary bulk action direct, and provides an explicit keyboard- and touch-accessible way to clear the selection. Selection actions must not alter ordinary row activation or become the only path to a single-item operation.
4. **Overflow-only action.** A low-frequency secondary action may move to overflow when space is constrained. Overflow never contains the sole ordinary primary path, preserves destructive grouping and labels, and uses the shared menu/sheet lifecycle for roving focus, Escape/outside dismissal, and focus restoration.

All four priorities preserve native keyboard activation and coarse-pointer touch targets. A compact destructive icon uses the danger treatment and still opens the same explicit confirmation flow as its text equivalent; icon density never bypasses confirmation or mutation guards.

### Responsive action placement

`ResponsiveActionGroup` accepts stable, unique action IDs and one explicit
placement for every action. It resolves placement from `LayoutModeProvider`;
feature code does not reproduce the viewport breakpoints.

| Placement | Wide | Medium | Compact |
| --- | --- | --- | --- |
| `always-visible` | direct | direct | direct |
| `wide-and-medium` | direct | direct | overflow |
| `wide-only` | direct | overflow | overflow |
| `overflow-only` | overflow | overflow | overflow |

Every group keeps its primary action `always-visible`. Direct familiar actions
may use `IconButton`; primary and unfamiliar actions retain text. Placement
never changes the action ID, label, disabled/loading guard, destructive status,
or callback. Collapsed actions use the canonical `OverflowMenu`, and a group
with no collapsed actions does not render an empty overflow trigger.

`OverflowMenu` owns one interaction contract for direct consumers and the thin
legacy `Menu` presentation:

- enabled items use roving `tabindex`; Arrow Up/Down wrap, Home/End move to the
  boundary, and Enter/Space activate the current item;
- disabled items remain labelled native disabled buttons, are skipped by
  roving focus, and cannot activate;
- destructive items are moved to a final group separated from ordinary items;
- Escape, platform Back, outside pointer dismissal, and compact backdrop
  dismissal close only the top overlay and restore the trigger;
- selection closes and restores the menu before invoking the feature callback,
  so a callback that opens another overlay records the correct opener; and
- wide/medium layouts use the anchored menu, while compact layout uses the
  shared safe-area-aware `Sheet` lifecycle.

`SelectionToolbar` exists only while a selection is active. It is a labelled
region with a polite, atomic selected-item count, one always-direct primary bulk
action, responsive secondary actions, and a separately named clear-selection
`IconButton`. Compact presentation is sticky above safe areas and the virtual
keyboard; all controls retain semantic coarse-pointer target sizes. Selection
state and bulk-operation policy remain feature/controller concerns.

`MetadataDialog` is a controlled composition over `Dialog`; `InfoButton` is the
self-contained disclosure composition for an information icon and that dialog.
Callers supply the title, optional description, and domain-neutral React
content. `MetadataList` / `MetadataItem` provide semantic `dl` rows when useful.
Wide and medium layouts use a centred dialog and compact layout uses a sheet.
Labelling, initial focus, focus containment/restoration, Escape, backdrop,
platform Back, reduced-motion, forced-colors, and safe-area behavior remain the
shared overlay and CSS-Module contract. Operational reads stay in their owning
controller or feature and are never initiated by shared UI.

## Semantic feedback

Controllers and screens emit semantic events such as:

```text
answer_revealed
review_committed
session_completed
import_completed
operation_failed
```

`ExperienceProvider` may translate these events into optional visual, audio, or haptic feedback. Durable work never waits for an effect, and a controller never calls a particular animation or sound directly.

The effects layer is pointer-transparent, does not affect layout, uses named UI layers, respects application and operating-system reduced-motion preferences, and may be lazy-loaded when larger assets are introduced. Information must remain understandable without motion, color, sound, or haptics.

## Accessibility rules

- Every control has an accessible name.
- Icon-only actions always have an `aria-label` and use visible explanatory text or a tooltip when needed.
- Information is not conveyed only by color, sound, motion, or haptics.
- Keyboard shortcuts do not fire while an overlay is active or focus is inside an interactive editor/control.
- Core actions do not require hover or long-press.
- Drag-and-drop enhancements retain button and keyboard alternatives.
- Loading, empty, error, success, and committed-but-not-reconciled states remain explicit.
- A busy operation explains what is happening and whether cancellation or dismissal is safe.
- Dense tables and charts retain semantic text or table equivalents.

## Testing

Component tests cover overlay stacking, initial focus, focus containment and restoration, Escape/Back precedence, menu dismissal, tab keyboard behavior, responsive navigation, toast interaction, and feature use of shared primitives.

Representative manual widths include:

```text
360 × 640
390 × 844
768 × 1024
1024 × 768
1280 × 800
1440 × 900
```

High-risk screens should also be checked with:

- Light, Dark, Console, and Synthwave themes;
- 80%, 100%, and 140% interface scale;
- reduced motion on and off;
- keyboard-only and coarse-pointer/touch interaction;
- long translated-style text and validation errors;
- no document-level horizontal overflow;
- overlay actions remaining visible above safe areas and virtual keyboards.

A UI refactor is not complete because it looks correct at one desktop width. It must preserve the shared behavior that makes the same workflow usable with different input methods, themes, and available space.
