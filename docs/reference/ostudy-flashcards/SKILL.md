---
name: ostudy-flashcards
description: Create importable oStudy .ostudy.json v2 flashcard decks from topics or supplied study material. Use for oStudy flashcards, decks, or import files.
---

# oStudy Flashcards

Create one valid, importable oStudy .ostudy.json flashcard file.

This skill only authors the file. It does not require MCP, an oStudy account, a remote oStudy service, or access to the user's local oStudy library. Never claim that a generated deck has already been imported.

## Workflow

1. Determine the requested source, scope, difficulty, and approximate card count.
2. Ground the deck in the user's supplied material when sources are provided.
3. Choose create or append semantics.
4. Author high-quality cards using format version 2.
5. Create one UTF-8 JSON file whose name ends in .ostudy.json.
6. Re-open and parse the completed file.
7. Check the whole file against the structural, reference, Markdown, Cloze, tag, and size rules below.
8. Correct every validation problem before returning the file.
9. Return the file plus a brief deck summary.

When code execution and file creation are available, use them to serialize and re-parse the JSON rather than manually escaping a large JSON document.

## Source grounding

For a topic-only request, use ordinary model knowledge at the requested level.

For pasted text, PDFs, slides, notes, or other supplied sources:

- ground factual content in those sources;
- preserve important qualifications, units, exceptions, mechanisms, and distinctions;
- cover the important concepts across the source instead of overproducing cards from easy isolated facts;
- do not invent citations or imply that an unavailable source was reviewed;
- do not silently replace uncertain source language with stronger certainty.

If the user supplies both sources and instructions, use the sources for factual grounding and the instructions for emphasis, card count, difficulty, and format.

## Deck destination

Default to a new deck:

    {
      "mode": "create",
      "name": "Useful human-readable deck name",
      "description": "Brief scope description"
    }

Use append only when the user explicitly asks to append and provides the exact existing oStudy deck name:

    {
      "mode": "append",
      "name": "Exact existing deck name"
    }

For append mode, description must be omitted or empty. The file cannot inspect the user's local library. Tell the user that oStudy requires exactly one active deck with that exact name.

## Top-level format

Always write format version 2:

    {
      "format": "ostudy-flashcards",
      "formatVersion": 2,
      "deck": {
        "mode": "create",
        "name": "Example",
        "description": "Example scope"
      },
      "defaultTags": [],
      "cards": []
    }

Allowed top-level properties are only:

- format
- formatVersion
- deck
- defaultTags
- cards

Required top-level properties are format, formatVersion, deck, and cards.

Use formatVersion 2 even when a deck happens to contain only Basic or Cloze cards. Version 1 is a compatibility reader, not the current authoring target.

## Global limits and identifiers

- File size: at most 16 MiB.
- Source cards: 1 through 10,000.
- Deck name: 1 through 200 characters.
- Deck description: at most 2,000 characters.
- Tag arrays: at most 32 entries.
- Tag length: 1 through 64 characters.
- Tags must not start with #.
- Avoid duplicate tags after normalization.
- Component IDs: 1 through 64 ASCII letters, digits, underscore, or hyphen.
- Do not include database IDs, schedules, review history, analytics, settings, local paths, credentials, or internal oStudy state.
- Do not invent extra object properties.

## Card quality

1. Test a meaningful retrievable idea, discrimination, relationship, mechanism, sequence, or classification.
2. Prefer conceptual understanding and useful application over isolated trivia.
3. Make each prompt specific enough that the intended answer is unambiguous.
4. Keep answers concise enough for active recall while retaining the explanation needed to understand why.
5. Avoid near-duplicate cards unless another card tests a genuinely different retrieval route.
6. Enable Basic reversal only when both directions are independently useful and unambiguous.
7. Use Cloze when sentence context improves retrieval; do not convert every sentence into deletion trivia.
8. Use single-choice for one best answer with plausible distractors.
9. Use select-all only when the set itself is worth learning.
10. Use matching only for a true one-to-one relationship: every visible left item and every visible right item must be unique. If two or more items share the same answer/category, use grouping instead.
11. Use grouping for many-to-one classification, including cases where several items correctly belong to the same category.
12. Use ordering only when a total sequence is intrinsically useful to study.
13. Avoid answer leakage through wording, grammar, option length, or category names.
14. Add concise rationales or explanations when they improve learning rather than merely restating the answer.
15. Use sparse, reusable topic tags.

For medical or nursing material, preserve clinically meaningful units, contraindications, conditions, exceptions, timing, routes, monitoring, and distinctions between association, mechanism, diagnosis, and treatment. Do not turn facility-specific conventions into universal rules.

## Basic cards

Shape:

    {
      "type": "basic",
      "front": "Question or cue",
      "back": "Answer or explanation",
      "reverseEnabled": false,
      "tags": ["optional-tag"]
    }

Required properties:

- type
- front
- back

Optional properties:

- reverseEnabled
- tags

front and back are restricted-Markdown strings with visible content.

## Cloze cards

Shape:

    {
      "type": "cloze",
      "text": "The {{c1::sinoatrial node}} is the normal cardiac pacemaker.",
      "mode": "one_at_a_time",
      "tags": ["optional-tag"]
    }

Required properties:

- type
- text

Optional properties:

- mode
- tags

mode may be one_at_a_time or all_together. The default is one_at_a_time.

Cloze markers use the form {{cN::answer}}. Group numbers 1 through 10000 are supported. A Cloze card must contain at least one valid marker. Do not create empty answers, malformed markers, or nested Cloze markers.

## Single-choice questions

Shape:

    {
      "type": "interactive",
      "question": {
        "kind": "single_choice",
        "prompt": "Which drug is an ACE inhibitor?",
        "options": [
          {"id": "lisinopril", "content": "Lisinopril"},
          {"id": "losartan", "content": "Losartan"}
        ],
        "correctOptionId": "lisinopril",
        "explanation": "Lisinopril is an ACE inhibitor."
      },
      "tags": ["optional-tag"]
    }

Use 2 through 32 options.

Each option contains:

- id
- content
- optional rationale

Option IDs must be unique within the question. Option content must also be unique within the question after Markdown is parsed. correctOptionId must reference exactly one existing option ID.

## Select-all questions

Shape:

    {
      "type": "interactive",
      "question": {
        "kind": "select_all",
        "prompt": "Which findings can occur with ACE inhibitors?",
        "options": [
          {"id": "cough", "content": "Cough"},
          {"id": "hyperkalemia", "content": "Hyperkalemia"},
          {"id": "hypokalemia", "content": "Hypokalemia"}
        ],
        "correctOptionIds": ["cough", "hyperkalemia"],
        "explanation": "ACE inhibitors can cause cough and hyperkalemia."
      }
    }

Use 2 through 32 options and at least one correct option.

Option IDs and visible option content must each be unique within the question. Every entry in correctOptionIds must reference an existing option ID, and correctOptionIds must not contain duplicates.

## True/false questions

Shape:

    {
      "type": "interactive",
      "question": {
        "kind": "true_false",
        "statement": "ACE inhibitors and ARBs should routinely be combined.",
        "correct": false,
        "correctedStatement": "ACE inhibitors and ARBs are generally not combined.",
        "explanation": "Combining them usually increases adverse effects without routine added benefit."
      }
    }

Required question properties:

- kind
- statement
- correct

correct must be a JSON boolean.

Optional properties:

- correctedStatement
- explanation

Omit unused optional fields rather than writing null.

## Matching questions

Shape:

    {
      "type": "interactive",
      "question": {
        "kind": "matching",
        "prompt": "Match each drug class with an example.",
        "pairs": [
          {"id": "ace", "left": "ACE inhibitor", "right": "Lisinopril"},
          {"id": "arb", "left": "ARB", "right": "Losartan"}
        ],
        "explanation": "Each pair tests a direct class-example relationship."
      }
    }

Use 2 through 16 pairs.

Each pair contains:

- id
- left
- right
- optional rationale

Pair IDs must be unique within the question.

Matching is bijective/one-to-one. After Markdown parsing:
- every left-side item must have unique visible content;
- every right-side item must have unique visible content.

Do not use matching when multiple left-side items share the same right-side answer. Use grouping for that many-to-one relation.

Invalid matching example:
- Outpatient mental health treatment → 13 or older
- Outpatient substance abuse treatment → 13 or older

Represent that as grouping with categories such as "13 or older", "14 or older", and "Any age", and put both outpatient services in the "13 or older" category.

## Grouping questions

Shape:

    {
      "type": "interactive",
      "question": {
        "kind": "grouping",
        "prompt": "Group each medication by class.",
        "categories": [
          {"id": "ace", "content": "ACE inhibitor"},
          {"id": "arb", "content": "ARB"}
        ],
        "items": [
          {"id": "lisinopril", "content": "Lisinopril", "categoryId": "ace"},
          {"id": "losartan", "content": "Losartan", "categoryId": "arb"}
        ]
      }
    }

Use 2 through 8 categories and 2 through 32 items.

Category IDs and category content must each be unique. Item IDs and item content must each be unique. Every categoryId must reference an existing category ID. Multiple different items may intentionally reference the same categoryId.

A grouped item may also contain an optional rationale.

## Ordering questions

Shape:

    {
      "type": "interactive",
      "question": {
        "kind": "ordering",
        "prompt": "Order the simplified RAAS sequence.",
        "items": [
          {"id": "renin", "content": "Renin release"},
          {"id": "angiotensin1", "content": "Angiotensin I formation"},
          {"id": "angiotensin2", "content": "Angiotensin II formation"}
        ],
        "explanation": "The authored item order is the correct sequence."
      }
    }

Use 2 through 32 items.

Item IDs and item content must each be unique. The authored array order is the correct answer order.

## Exact object-shape rules

Do not add properties outside these sets.

Basic card:

- type
- front
- back
- optional reverseEnabled
- optional tags

Cloze card:

- type
- text
- optional mode
- optional tags

Interactive card:

- type
- question
- optional tags

Option:

- id
- content
- optional rationale

Matching pair:

- id
- left
- right
- optional rationale

Grouping category and ordering item:

- id
- content

Grouping item:

- id
- content
- categoryId
- optional rationale

single_choice question:

- kind
- prompt
- options
- correctOptionId
- optional explanation

select_all question:

- kind
- prompt
- options
- correctOptionIds
- optional explanation

true_false question:

- kind
- statement
- correct
- optional explanation
- optional correctedStatement

matching question:

- kind
- prompt
- pairs
- optional explanation

grouping question:

- kind
- prompt
- categories
- items
- optional explanation

ordering question:

- kind
- prompt
- items
- optional explanation

## Restricted Markdown

Rich-text fields may use ordinary supported Markdown such as:

- paragraphs and headings;
- bold, italic, and strikethrough;
- inline code and fenced code blocks;
- block quotes;
- ordered and unordered lists;
- absolute http, https, mailto, and tel links;
- hard line breaks;
- horizontal rules.

Do not use these as Markdown/source syntax:

- raw HTML;
- embedded or remote Markdown images;
- local file paths;
- Markdown tables;
- executable content;
- unknown custom Markdown syntax.

Literal comparison characters such as 2 < 3 and syntax-looking text inside supported code spans or fenced code blocks are ordinary text, not HTML.

Plain text is always acceptable when formatting is unnecessary.

## Validation checklist

Before returning the file, re-open it and verify all of the following:

- It parses as UTF-8 JSON.
- format is exactly ostudy-flashcards.
- formatVersion is the JSON integer 2.
- deck contains only mode, name, and optional description.
- deck.mode is create or append.
- deck.name is non-empty and no longer than 200 characters.
- Append mode has no non-empty description.
- There is at least one card and no more than 10,000.
- The serialized file is no larger than 16 MiB.
- Every card has exactly the allowed properties for its type.
- Every required property is present.
- Every local component ID matches the allowed ASCII identifier syntax.
- IDs are unique within each option, pair, category, or item collection.
- Choice option content is unique within each choice question.
- Matching has 2–16 pairs, with unique visible content independently on the left side and on the right side; any repeated target value is represented as grouping instead.
- Grouping has 2–8 unique categories and 2–32 unique items; multiple items may reference the same category.
- Ordering has 2–32 items with unique visible content.
- Every correctOptionId, correctOptionIds entry, and categoryId resolves to an existing component.
- Select-all has at least one correct option.
- Tags satisfy their count and length limits, have no leading #, and do not contain duplicates.
- Every Cloze card has at least one valid non-nested marker.
- Restricted-Markdown fields contain no raw HTML, Markdown images, local paths, tables, executable syntax, or unsupported link schemes.
- Optional rationale, explanation, and correctedStatement fields are either valid non-empty strings or omitted; do not emit null.
- No image_occlusion question is generated. Image occlusion requires managed-image ownership and is outside this text-only JSON transport.

If validation fails, correct the file and run the checklist again. Never return a known-invalid artifact.

oStudy's native Rust importer remains the final authority for durable import semantics and will reject unsupported or malformed content atomically.

## Return the result

Return the completed .ostudy.json file itself rather than pasting the entire JSON unless the user asks to inspect it.

Also state briefly:

- deck name;
- source-card count;
- card-type mix;
- whether the destination mode is create or append;
- import path: Flashcards → Decks → Import deck from file.

Do not claim that the file has already been imported into oStudy.
