# ostudy Flashcard Import Agent Instructions — v2

Generate one UTF-8 `.ostudy.json` document with no surrounding commentary or Markdown fence.

## Required envelope

Use the current mixed-card format:

```json
{
  "format": "ostudy-flashcards",
  "formatVersion": 2,
  "deck": {
    "mode": "create",
    "name": "Required deck name",
    "description": "Optional description for a new deck"
  },
  "defaultTags": [],
  "cards": []
}
```

`deck.mode` is `create` or `append`. Append requires exactly one active deck with the exact supplied name and must omit or empty `description`.

The authoritative reviewable schema is [`docs/reference/ostudy-flashcards-v2.schema.json`](ostudy-flashcards-v2.schema.json). The native Rust importer remains authoritative for semantic validation.

## Supported card families

Version 2 may mix all of these in one file:

- `basic`
- `cloze`
- `interactive` / `single_choice`
- `interactive` / `select_all`
- `interactive` / `true_false`
- `interactive` / `matching`
- `interactive` / `grouping`
- `interactive` / `ordering`

Image occlusion is not part of generated JSON because this text-only transport does not own local managed images.

### Basic

```json
{"type":"basic","front":"Question","back":"Answer","reverseEnabled":false,"tags":[]}
```

### Cloze

```json
{"type":"cloze","text":"The {{c1::SA node}} is the normal pacemaker.","mode":"one_at_a_time","tags":[]}
```

### Single choice

```json
{
  "type":"interactive",
  "question":{
    "kind":"single_choice",
    "prompt":"Which finding is expected?",
    "options":[
      {"id":"a","content":"Finding A"},
      {"id":"b","content":"Finding B"}
    ],
    "correctOptionId":"a",
    "explanation":"Why A is correct."
  }
}
```

### Select all

```json
{
  "type":"interactive",
  "question":{
    "kind":"select_all",
    "prompt":"Select all that apply.",
    "options":[
      {"id":"a","content":"A"},
      {"id":"b","content":"B"},
      {"id":"c","content":"C"}
    ],
    "correctOptionIds":["a","c"]
  }
}
```

### True / false

```json
{
  "type":"interactive",
  "question":{
    "kind":"true_false",
    "statement":"The statement to evaluate.",
    "correct":false,
    "correctedStatement":"The corrected statement."
  }
}
```

### Matching

```json
{
  "type":"interactive",
  "question":{
    "kind":"matching",
    "prompt":"Match each item.",
    "pairs":[
      {"id":"a","left":"ACE inhibitor","right":"Lisinopril"},
      {"id":"b","left":"ARB","right":"Losartan"}
    ]
  }
}
```

### Grouping

```json
{
  "type":"interactive",
  "question":{
    "kind":"grouping",
    "prompt":"Place each item in its category.",
    "categories":[
      {"id":"arterial","content":"Arterial"},
      {"id":"venous","content":"Venous"}
    ],
    "items":[
      {"id":"one","content":"Example one","categoryId":"arterial"},
      {"id":"two","content":"Example two","categoryId":"venous"}
    ]
  }
}
```

### Ordering

```json
{
  "type":"interactive",
  "question":{
    "kind":"ordering",
    "prompt":"Put the steps in order.",
    "items":[
      {"id":"first","content":"First step"},
      {"id":"second","content":"Second step"}
    ]
  }
}
```

## Content and quality rules

Basic, interactive prompt/content, rationales, explanations, and corrections use oStudy's restricted Markdown. Do not emit raw HTML, remote or embedded images, local file paths, executable content, or Markdown tables.

Component IDs are local transport keys: 1–64 ASCII letters, digits, `_`, or `-`. References such as `correctOptionId`, `correctOptionIds`, and `categoryId` must resolve to IDs in the same question. Pair/item/option IDs must be unique where the native question contract requires uniqueness.

Use only source-supported information. Prefer conceptually useful questions over trivia, preserve qualifiers and units, avoid duplicate cards, and use the card family that best tests the relationship. A file is validated completely before any write; one invalid card aborts the whole import. Accepted cards receive fresh local schedules.

Version 1 remains readable for legacy Basic/Cloze files, but new generated files should use version 2.
