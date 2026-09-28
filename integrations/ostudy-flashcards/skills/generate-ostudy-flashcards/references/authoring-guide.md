# oStudy flashcard authoring guide

This reference is for the `generate-ostudy-flashcards` skill. The native oStudy importer is authoritative; this guide focuses on choosing useful study prompts and the generated v2 transport shapes.

## Content-first card selection

Choose the question form because it matches the knowledge being learned, not to create superficial variety.

| Learning target | Preferred form |
| --- | --- |
| Definition, mechanism, explanation, comparison | Basic |
| Term or value embedded in useful context | Cloze |
| One best decision among plausible alternatives | `single_choice` |
| A bounded set of independently correct selections | `select_all` |
| A proposition whose truth and correction matter | `true_false` |
| Pairwise relationship between two small sets | `matching` |
| Classification into meaningful categories | `grouping` |
| A sequence where order is part of the knowledge | `ordering` |

A deck does not need every type. Avoid interactive formats when a direct recall prompt tests the concept better.

## Basic and Cloze

Basic:

```json
{
  "type": "basic",
  "front": "What mechanism causes the finding?",
  "back": "A concise explanation.",
  "reverseEnabled": false,
  "tags": ["mechanism"]
}
```

Cloze:

```json
{
  "type": "cloze",
  "text": "The {{c1::sinoatrial node}} normally initiates cardiac depolarization.",
  "mode": "one_at_a_time",
  "tags": ["cardiac-electrophysiology"]
}
```

Cloze groups are `c1` through `c10000`. Hidden text must be non-empty. Reuse a group number when blanks belong to one retrieval target.

## Interactive envelope

Every text-native interactive card uses:

```json
{
  "type": "interactive",
  "question": {
    "kind": "single_choice"
  },
  "tags": []
}
```

Local component IDs are transport keys, not database IDs. Use stable short keys containing only ASCII letters, digits, `_`, or `-`, at most 64 characters.

### Single choice

Use 2–32 distinct options and one valid `correctOptionId`.

```json
{
  "kind": "single_choice",
  "prompt": "Which finding is most consistent with prerenal acute kidney injury?",
  "options": [
    {"id":"low_na","content":"Urine sodium <20 mEq/L","rationale":"Intact tubules avidly retain sodium during hypoperfusion."},
    {"id":"high_na","content":"Urine sodium >40 mEq/L","rationale":"This is more typical of impaired tubular reabsorption."}
  ],
  "correctOptionId": "low_na",
  "explanation": "Prerenal physiology preserves tubular sodium reabsorption."
}
```

### Select all

Use 2–32 distinct options. `correctOptionIds` must contain unique IDs present in `options`. The native format permits an empty correct set, although generated study questions should usually avoid an all-false item unless that absence is itself educational.

### True/false

Use `statement`, Boolean `correct`, and optional `explanation` / `correctedStatement`. When false, a concise corrected statement usually teaches more than a bare verdict.

### Matching

Use 2–16 unique pairs. Each pair has `id`, `left`, `right`, and optional `rationale`. Left-side content must be distinct; right-side content must be distinct. Use matching only when the pair relation is meaningful, not as a way to bundle unrelated facts.

### Grouping

Use 2–8 categories and 2–32 items. Categories use `{id, content}`. Items use `{id, content, categoryId, rationale?}`. Every `categoryId` must refer to a declared category.

### Ordering

Use 2–32 `{id, content}` items in the authored correct order. The order itself must encode knowledge worth retrieving.

## Restricted Markdown

Basic sides and interactive rich-text strings may use paragraphs/headings, bold, italic, strikethrough, inline/fenced code, block quotes, ordered/bullet lists, hard line breaks, horizontal rules, and absolute `http`, `https`, `mailto`, or `tel` links.

Do not use raw HTML, Markdown images, Markdown tables, local file paths, executable content, remote embedded images, or unsafe/relative links. Literal comparison characters such as `2 < 3` are ordinary text. Syntax-looking text inside inline or fenced code is literal code.

## Tags and limits

- One generated file targets one deck.
- The file may contain at most 10,000 source cards and 16 MiB of UTF-8 JSON.
- Deck names: 1–200 visible characters after trimming.
- Deck descriptions: at most 2,000 characters after trimming.
- Combined default + per-card tags: at most 32 unique normalized tags.
- Tags: 1–64 visible characters after trimming and no leading `#`.
- Generated local component IDs: 1–64 ASCII letters, digits, `_`, or `-`.
- Interactive component/cardinality limits are enforced by the bundled writer and native importer.

## Source grounding

When a source document is present, every factual card should be supportable from that source. Prefer synthesis that connects nearby evidence, but do not introduce outside facts as if they came from the document. If a source is ambiguous, phrase the card conservatively or omit it.

For topic-only generation, ordinary model knowledge may be used. Keep uncertainty and context where they materially affect correctness.
