# oStudy Flashcard Import Agent Instructions — v1/v2

Use these instructions when an external model or authoring tool generates an oStudy flashcard import file. The native importer remains authoritative; this document describes the public content-only transport.

For the maintained ChatGPT implementation, see the [skills-only oStudy Flashcards plugin](../../integrations/ostudy-flashcards/README.md). It writes version 2 and runs a deterministic preflight before returning the artifact.

## Output contract

Produce one valid UTF-8 JSON document. Do not add commentary, Markdown code fences, JSON comments, or trailing commas inside the file.

The top-level object contains:

- `"format": "ostudy-flashcards"`;
- `"formatVersion": 1` for Basic/Cloze-only compatibility or `2` for current mixed generation;
- one `"deck"` object;
- an optional `"defaultTags"` array; and
- a non-empty `"cards"` array.

Do not generate oStudy database IDs, timestamps, schedules, review history, analytics, settings, reusable template IDs, local paths, HTML, or image bytes.

New generators should use **format version 2**. The maintained machine-readable contracts are the [v1 schema](../ostudy-flashcards-v1.schema.json) and [v2 schema](../ostudy-flashcards-v2.schema.json).

## Choose the destination in the file

Create a new deck:

```json
"deck": {
  "mode": "create",
  "name": "NCLEX Pharmacology",
  "description": "Medication safety and nursing priorities"
}
```

`create` always creates a new deck. It does not search for or overwrite another deck with the same displayed name.

Append to an existing deck only when the user deliberately requests it:

```json
"deck": {
  "mode": "append",
  "name": "NCLEX Pharmacology"
}
```

`append` requires exactly one active deck whose name matches exactly. Omit `description` or leave it empty. Import fails without changing the library when no active deck has that exact name or when the name is ambiguous.

## Recall cards

Basic:

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

Cloze:

```json
{
  "type": "cloze",
  "text": "The {{c1::sinoatrial node}} is the normal cardiac pacemaker.",
  "mode": "one_at_a_time",
  "tags": ["optional tag"]
}
```

Use markers in the form `{{cN::hidden text}}`, where `N` is 1–10000. Reuse a group number when multiple blanks belong to one retrieval target. Modes are `one_at_a_time` and `all_together`.

Version 1 stops here. Interactive cards require version 2.

## Version-2 interactive cards

All text-native interactive cards use:

```json
{
  "type": "interactive",
  "question": {
    "kind": "single_choice"
  },
  "tags": ["optional tag"]
}
```

Generated component IDs are transport-local keys containing 1–64 ASCII letters, digits, `_`, or `-`. They are not oStudy database IDs.

Version 2 supports:

- `single_choice`: `prompt`, 2–32 `options`, one valid `correctOptionId`, optional `explanation`;
- `select_all`: `prompt`, 2–32 `options`, unique valid `correctOptionIds`, optional `explanation`;
- `true_false`: `statement`, Boolean `correct`, optional `explanation` and `correctedStatement`;
- `matching`: `prompt`, 2–16 `pairs`, optional `explanation`;
- `grouping`: `prompt`, 2–8 `categories`, 2–32 `items` whose `categoryId` references a category, optional `explanation`;
- `ordering`: `prompt`, 2–32 `items` in the authored correct order, optional `explanation`.

Choice options use `{ "id", "content", "rationale"? }`. Matching pairs use `{ "id", "left", "right", "rationale"? }`. Grouping categories and ordering items use `{ "id", "content" }`; grouped items add `"categoryId"` and may add `"rationale"`.

Image occlusion is deliberately excluded from generated JSON because this text-only transport cannot create or own managed local image assets.

The [all-card-types plugin fixture](../../integrations/ostudy-flashcards/skills/generate-ostudy-flashcards/references/all-card-types.ostudy.json) is executable conformance data rather than pseudocode.

## Restricted Markdown

Basic sides and interactive rich-text strings support paragraphs/headings, bold, italic, strikethrough, inline and fenced code, block quotes, ordered/bullet lists, absolute `http`, `https`, `mailto`, and `tel` links, hard line breaks, and horizontal rules.

Do not use raw HTML, Markdown images, Markdown tables, local file links, executable content, or unsafe/relative links. Ordinary comparison text may contain literal `<` or `>` characters. Syntax-looking text inside supported inline or fenced code is literal code.

The Rust restricted-Markdown parser and rich-content validator are authoritative. An external preflight should fail closed rather than trying to broaden the accepted syntax.

## Tags and bounds

Tags are human-readable topic labels without `#`.

- At most 10,000 source cards per file.
- At most 16 MiB of UTF-8 JSON per file.
- Deck names: 1–200 characters after trimming.
- Deck descriptions: at most 2,000 characters after trimming.
- At most 32 combined default/card tag entries per note.
- Each tag: 1–64 visible characters after trimming.

Interactive cardinalities and references are validated by the native question domain as well as the maintained skills-only preflight.

## Import behavior

The user selects the file through **Flashcards → Decks → Import deck from file**. oStudy validates the complete file before opening one transaction. If any card is invalid, nothing is written and diagnostics identify affected `cards[index]` paths. Exact duplicates within the file or destination deck are skipped automatically. Accepted cards receive fresh local schedules; generated imports do not carry donor review history.

## Flashcard quality

- Use only information supported by supplied source material when the request is source-grounded.
- Prefer concepts, mechanisms, relationships, classifications, sequences, and meaningful distinctions over isolated trivia.
- Test one coherent retrieval target per card.
- Make each prompt specific enough to have a clear intended answer.
- Keep answers concise but sufficient to understand the concept.
- Use plausible distractors that test discrimination without trick wording.
- Avoid redundant cards and vague prompts.
- Use Cloze only when sentence context is useful.
- Use matching/grouping/ordering only when the relation or order is itself worth learning.
- Preserve important terminology, units, qualifiers, exceptions, and uncertainty.

## Validate before returning a generated file

A generator should parse the final JSON, check exact property names and supported enums, verify interactive references/cardinalities, validate Cloze markers and tags, enforce file/card limits, and refuse to return an invalid artifact.

The maintained ChatGPT skill performs this deterministic step with:

```bash
python3 integrations/ostudy-flashcards/skills/generate-ostudy-flashcards/scripts/write_ostudy.py \
  --input /path/to/draft.json \
  --output /path/to/deck.ostudy.json
```

That preflight complements rather than replaces the native Rust importer.
