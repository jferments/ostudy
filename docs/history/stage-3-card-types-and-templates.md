# Card types and reusable templates

Stage 3 adds three authoring paths to the **Card template** selector in the card
editor:

1. **Basic** — one normal question-and-answer card, with an optional reverse
   presentation;
2. **Fill in the blank** — one source statement with one scheduled card that can
   rotate through several marked blanks;
3. a reusable custom template — one rich field form that generates several
   permanent Basic cards.

The selector defaults to **Basic** for every new card.

## Basic and reverse cards

Enter the Question and Answer exactly as for a normal card. Enable **Create
reverse card** only when both directions are meaningful.

For example:

```text
Question: What is the capital of Japan?
Answer: Tokyo
```

With the reverse option disabled, the note creates one card. With the option
enabled, it creates two independently scheduled cards:

```text
What is the capital of Japan? → Tokyo
Tokyo → What is the capital of Japan?
```

The reverse card has its own due date and review history. Disabling the reverse
option hides that presentation without deleting its learning state. Enabling it
again restores the same card identity, schedule, and prior reviews.

Use reverse cards selectively. A long explanation usually makes sense in one
direction only, whereas vocabulary pairs, abbreviations, anatomy labels, and
simple associations often work well in both directions.

## Fill in the blank

Choose **Fill in the blank**, then write the complete statement in the source
field. Select a word or phrase and choose **Mark selection as blank**. The editor
adds a stable numbered marker internally and displays a preview.

Starting with:

```text
The sinoatrial node normally initiates the cardiac electrical impulse.
```

Marking two phrases produces source data equivalent to:

```text
The {{c1::sinoatrial node}} normally initiates the {{c2::cardiac electrical impulse}}.
```

The braces are data markers, not HTML or code. The normal workflow is to select
text and use the marking button rather than typing marker syntax manually.

### Hide one blank per review

This mode still creates exactly one scheduled card. On each accepted review,
the next practice session rotates to another marked blank:

```text
The […] normally initiates the cardiac electrical impulse.

The sinoatrial node normally initiates the […].
```

Only the target blank is hidden. Other marked phrases remain visible as context.
The selected blank is stored in the active practice session, so revealing,
leaving, or resuming that session cannot switch the question halfway through.

### Hide all blanks together

This mode uses the same card ID and schedule but hides every marked phrase:

```text
The […] normally initiates the […].
```

Switching between cloze modes does not create another schedule or reset review
history.

### Editing blank text

- Add another blank by selecting another phrase and marking it.
- Use **Remove all blank markers** to return to ordinary unmarked source text.
- A saved card cannot be converted between Basic and Fill in the blank. Create a
  new card when the underlying authoring model changes.
- Adding, removing, or renumbering blanks changes the rotating presentations but
  does not create sibling card schedules.

## Creating a reusable template

Open **Templates** from the sidebar and select **New template**.

A template has:

- a name and optional description;
- one or more rich input fields;
- one or more card-generation rules.

A general concept template might define:

```text
Subject
Definition
Important details
Diagram
```

The optional **Field description** can explain what belongs in each field.

Design each Question and Answer layout with the normal rich editor. Insert a
field by typing its name in complete double braces:

```text
Question: What is {{Subject}}?
Answer: {{Definition}}

Question: What are the important details about {{Subject}}?
Answer: {{Important details}}
```

Field-name matching ignores capitalization and repeated surrounding whitespace,
but the name must refer to one unique field in the same template. Incomplete or
unknown placeholders fail visibly when the template is saved.

There is no separate inline or block insertion control. Placement is determined
from the rich value entered when the template is used.

## Automatic inline and block placement

When previewing or generating cards, ostudy examines the validated structure of
each submitted field value.

A value remains **inline** when it contains exactly one paragraph made only of
text and hard breaks, with no image. This includes ordinary words, numbers,
short phrases, and formatted text.

A value becomes a **block** when it contains supported structured content such
as:

- ordered or bullet lists;
- images;
- headings;
- block quotes or code blocks;
- horizontal rules;
- multiple paragraphs.

For example, this layout:

```text
Important findings: {{Important details}} Continue with the interpretation.
```

renders as one sentence when `Important details` is a short phrase. When the
field contains a numbered list, the renderer safely produces:

```text
Important findings:

1. First finding
2. Second finding

Continue with the interpretation.
```

The list remains a real list. The renderer splits the surrounding paragraph into
valid before/value/after blocks and removes empty wrapper paragraphs. Images and
other supported block nodes are likewise preserved rather than flattened into
plain text.

## Generating cards from a reusable template

Open a deck or select exactly one deck in Search/Filter, then choose the custom
template from the **Card template** selector. The editor replaces the normal
Question and Answer fields with one full rich editor for every template field.

After completing the form:

1. choose **Preview generated cards** to inspect every question and answer;
2. add any shared tags;
3. choose **Generate cards**.

Every accepted rule becomes an ordinary permanent Basic card in the selected
deck. Exact duplicates are skipped. The generated cards receive fresh schedules
and participate in Search/Filter, Practice, analytics, tags, move/copy, and deck
export like manually created Basic cards.

## Template deletion and generated cards

A reusable template is only an authoring shortcut. Generated cards do not remain
linked to it.

Deleting or editing a template:

- does not edit cards already generated from it;
- does not delete generated cards;
- does not reset their schedules;
- does not remove their review history.

Delete generated cards from their deck or Search/Filter, not from the Templates
screen.

## Template safety

Templates perform bounded, literal data substitution only. They cannot execute:

- JavaScript or another scripting language;
- SQL;
- shell commands;
- raw HTML;
- filesystem operations;
- network requests;
- a general-purpose expression language.

Rust resolves typed placeholders to stable field identifiers, validates every
layout and value, chooses placement from the document structure, and validates
the complete generated output. The final card is then written through the same
transactional path as an ordinary Basic card.

## Search/Filter and note-level actions

Search/Filter lists each active scheduled card separately. Basic forward and
reverse cards therefore appear as distinct rows, while a multi-blank cloze note
appears once because its alternate blanks share one card and schedule.

Tags, move/copy, and deletion apply to the shared source note. When several
presentations from the same note are selected, ostudy intentionally performs the
note-level action once:

- moving moves every active presentation and preserves every schedule;
- copying creates a new note and fresh schedules for its active presentations;
- tags apply to all presentations through the note;
- deleting the note removes every generated presentation.

## Deck export and import

New `.studydeck` exports preserve Basic, reverse, one-at-a-time cloze, and
all-blanks cloze definitions. Imported cards receive fresh local schedules and
do not include the exporting learner's review history.

Reusable templates are not exported with a deck because they are local
authoring tools rather than content owners. Older Basic and Stage 3 packages
remain importable. Older multi-row cloze packages are normalized to one active
card during import.
