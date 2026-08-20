# Responsive layout modes

ostudy uses one shared React interface with three deliberate shell modes:

| Mode | CSS viewport width | Navigation | Primary purpose |
|---|---:|---|---|
| Phone | up to 760 px | top app bar and bottom navigation | narrow, touch-first single-column work |
| Tablet | 761–1179 px | labeled side rail | intermediate devices such as the Boox Note Air family |
| Wide | 1180 px and above | full desktop sidebar | desktop and large landscape workspaces |

The shell breakpoint selects navigation structure only. Individual workspaces adapt through named CSS container queries so a panel responds to its real available width after the side rail, UI scaling, and parent layout are applied.

## Flashcards contracts

- Deck and card lists use the compact representation at every width.
- Card previews preserve complete source strings in the DOM and use CSS ellipsis rather than JavaScript character limits.
- At narrow card-browser widths, every preview is exactly one question line and one answer line.
- Reorder and destructive actions retain accessible names and coarse-pointer touch targets.
- Toolbars wrap based on their container, not an assumed device class.

## Diagnostics

Settings → Diagnostics reports only safe layout facts: shell mode, viewport dimensions, orientation, UI scale, effective scaled width, device-pixel ratio, and pointer class. It never includes deck names, card content, document text, or prompts.

The breakpoint constants in `src/ui/layout/layoutMode.ts` and shell CSS must change together. The pure breakpoint/scale tests include phone, 824 px portrait tablet, 1024 px tablet, and wide desktop fixtures.
