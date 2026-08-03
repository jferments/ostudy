# ostudy Flashcard Import Agent Instructions — v1

Use these instructions when generating an ostudy flashcard import file from supplied source material.

## Output contract

Produce one valid UTF-8 JSON document only. Do not add commentary, Markdown code fences, JSON comments, or trailing commas.

The top-level object must contain:

- `"format": "ostudy-flashcards"`
- `"formatVersion": 1`
- one `"deck"` object
- an optional `"defaultTags"` array
- a non-empty `"cards"` array

Do not generate ostudy database IDs, timestamps, schedules, review history, analytics, settings, template IDs, file paths, HTML, or image data.

## Choose the destination in the file

Every file must state what ostudy should do with the named deck:

### Create a new deck

```json
"deck": {
  "mode": "create",
  "name": "NCLEX Pharmacology",
  "description": "Medication safety and nursing priorities"
}
```

`create` always creates a new deck. It does not search for or overwrite another deck with the same name.

### Append to an existing deck

```json
"deck": {
  "mode": "append",
  "name": "NCLEX Pharmacology"
}
```

`append` adds cards to exactly one active deck whose name matches exactly. Omit `description` in append mode. Import fails without changing the library when no deck has that exact name or when more than one active deck has that name.

## Basic cards

```json
{
  "type": "basic",
  "front": "Question in restricted Markdown",
  "back": "Answer in restricted Markdown",
  "reverseEnabled": false,
  "tags": ["optional tag"]
}
```

Set `reverseEnabled` to `true` only when both directions are independently useful and unambiguous.

## Fill-in-the-blank cards

```json
{
  "type": "cloze",
  "text": "The {{c1::sinoatrial node}} is the normal cardiac pacemaker.",
  "mode": "one_at_a_time",
  "tags": ["optional tag"]
}
```

Use exact markers in the form `{{c1::hidden text}}`. Reuse the same number when multiple blanks should be hidden together. Allowed modes are `one_at_a_time` and `all_together`.

## Content rules

Basic sides support paragraphs, headings, bold, italic, strikethrough, inline code, fenced code blocks, block quotes, ordered lists, bullet lists, absolute links, hard line breaks, and horizontal rules.

Do not use raw HTML, images, local file paths, executable content, or Markdown tables in version 1. Represent table material as a concise list or code block.

Tags are human-readable topic labels without `#`. After combining default and card tags, use no more than 32 unique tags per card, with no tag longer than 64 characters.

## Import behavior

The user selects the JSON file through the ordinary **Import deck** button. ostudy validates every card before opening one transaction. If any card is invalid, nothing is written and the error identifies the affected `cards[index]` entries. Exact duplicates within the file or destination deck are skipped automatically. Accepted cards receive fresh local schedules.

## Flashcard quality

- Use only information supported by the supplied source.
- Test one main idea per card.
- Make each question specific enough to have a clear answer.
- Keep answers concise but sufficient to explain the concept.
- Prefer mechanisms, relationships, and meaningful distinctions over isolated trivia.
- Avoid redundant cards and vague prompts.
- Use cloze cards only when sentence context is useful.
- Preserve important terminology, units, qualifiers, and exceptions.

## Complete template

```json
{
  "format": "ostudy-flashcards",
  "formatVersion": 1,
  "deck": {
    "mode": "create",
    "name": "Required deck name",
    "description": "Optional source or scope description"
  },
  "defaultTags": ["optional shared tag"],
  "cards": [
    {
      "type": "basic",
      "front": "A focused question",
      "back": "A concise, accurate answer",
      "reverseEnabled": false,
      "tags": ["topic"]
    },
    {
      "type": "cloze",
      "text": "A statement containing {{c1::one or more hidden terms}}.",
      "mode": "one_at_a_time",
      "tags": ["topic"]
    }
  ]
}
```

Before returning the file, verify that the JSON parses, `deck.mode` is `create` or `append`, every property name matches this contract exactly, every Basic card has non-empty `front` and `back`, and every cloze card contains at least one valid marker.
