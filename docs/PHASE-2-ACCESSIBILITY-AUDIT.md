# Phase 2 accessibility audit

This audit covers the standalone flashcard-core interactions added in Phase 2.
It is an implementation checklist and a manual acceptance script rather than a
claim of formal conformance certification.

## Keyboard operation

- The primary navigation, deck list, card browser, filters, editor controls,
  template builder, bulk-entry form, move/copy dialog, settings, and Practice
  setup use native buttons, links, inputs, selects, and checkboxes.
- Practice can be completed without a pointer: `Space` or `Enter` reveals the
  answer, `0` through `3` records Wrong/Hard/OK/Easy, and `U` or `Ctrl/Cmd+Z`
  undoes the last review.
- Keyboard shortcuts are ignored while focus is in an input, textarea, select,
  or editable rich-text surface.
- Modal dialogs trap `Tab`/`Shift+Tab`, close with `Escape`, place initial focus
  inside the dialog, and restore focus to the opener after closing.
- All interactive controls have a visible `:focus-visible` indicator in every
  bundled theme.

## Structure and announcements

- Screens use one top-level heading and ordered section headings.
- Dialogs expose `role="dialog"`, `aria-modal="true"`, a labelled title, and a
  description.
- Errors use `role="alert"`; transient progress and successful state changes
  use polite live regions.
- Practice announces the current card, answer reveal, review completion, undo,
  and session completion without moving keyboard focus unexpectedly.
- Rating controls include the numeric shortcut, everyday label, meaning, and
  interval preview in their accessible names.
- Decorative confidence bars and icons are hidden from assistive technology;
  the same information is present as text counts.

## Visual and motion checks

- Text and controls retain usable contrast in Light, Dark, Console, and
  Synthwave themes.
- Information is never communicated by color alone: Wrong, Hard, OK, and Easy
  always include text labels and counts.
- Layouts reflow at narrow window widths without requiring horizontal page
  scrolling for ordinary text sizes.
- UI scaling and card font-size preferences continue to apply to Phase 2
  screens.
- The existing Reduce motion preference disables nonessential transitions and
  animations.
- User images have constrained Small/Medium/Large sizing and Left/Center/Right
  alignment options so oversized content cannot make the card controls
  unreachable.

## Manual audit before merge

Run the application and complete these checks using only the keyboard:

1. Create a deck, create and edit each card type, add tags, and recover an
   autosaved draft.
2. Search and filter the card browser, then suspend, bury, reset, reschedule,
   move, copy, and delete a card.
3. Open every Phase 2 dialog; verify focus stays inside, `Escape` closes it, and
   focus returns to the control that opened it.
4. Start Practice with the compact **Shuffle decks** checkbox, reveal and rate
   cards with keyboard shortcuts, undo a review, and inspect Recently practiced.
5. Repeat at 200% UI scale, with Reduce motion enabled, and in all four themes.
6. Use a screen reader to verify headings, field labels, errors, live
   announcements, rating descriptions, and confidence-count summaries.

Any failure discovered in this checklist blocks merging the pull request until it is corrected and re-tested.
