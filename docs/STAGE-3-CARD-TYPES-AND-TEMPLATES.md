# Card types and reusable templates

Stage 3 adds three authoring paths to the **Card template** selector in the card
editor:

1. **Basic** — one normal question-and-answer card, with an optional reverse
   presentation;
2. **Fill in the blank** — one source statement that generates one or more cloze
   cards;
3. a reusable custom template — one field form that generates several permanent
   Basic cards.

The selector defaults to **Basic** for every new card.

## Basic and reverse cards

Enter the Question and Answer exactly as for a normal card. Enable **Create
reverse card** only when both directions are meaningful.

For example:

```text
Question: What class of medication is lisinopril?
Answer: ACE inhibitor
```

With the reverse option disabled, the note creates one card. With the option
enabled, it creates two independently scheduled cards:

```text
What class of medication is lisinopril? → ACE inhibitor
ACE inhibitor → What class of medication is lisinopril?
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
adds a stable numbered marker internally and displays a generated-card preview.

Starting with:

```text
ACE inhibitors block the enzyme that creates angiotensin II, lowering blood pressure.
```

Marking two phrases produces source data equivalent to:

```text
{{c1::ACE inhibitors}} block the enzyme that creates {{c2::angiotensin II}}, lowering blood pressure.
```

The braces are data markers, not HTML or code. The normal workflow is to select
text and use the marking button rather than typing marker syntax manually.

### Hide one blank at a time

This mode generates one independently scheduled card for each numbered blank.
The example creates:

```text
[…] block the enzyme that creates angiotensin II, lowering blood pressure.

ACE inhibitors block the enzyme that creates […], lowering blood pressure.
```

Only the target blank is hidden on each card. Other marked phrases remain
visible as context.

### Hide all blanks together

This mode generates one card:

```text
[…] block the enzyme that creates […], lowering blood pressure.
```

Use this when recalling the complete set together is the intended learning task.

The combined presentation has a different schedule identity from every
individual blank. Switching modes cannot transfer one blank's history to the
combined question. Switching back restores the previous individual blank cards
and their schedules.

### Editing blank text

- Add another blank by selecting another phrase and marking it.
- Use **Remove all blank markers** to return to ordinary unmarked source text.
- A saved card cannot be converted between Basic and Fill in the blank. Create a
  new card when the underlying authoring model changes.
- Changing or removing a numbered blank retires presentations that no longer
  exist. Retaining the same marker number retains that presentation's identity.

## Creating a reusable template

Open **Templates** from the sidebar and select **New template**.

A template has:

- a name and optional description;
- one or more input fields;
- one or more card-generation rules.

A pharmacology template might define these fields:

```text
Name
Drug class
Side effects
Conditions
```

Optional field guidance can tell the user what belongs in each input, such as
“Include common and high-risk adverse effects.”

Then define rules such as:

```text
Rule: Drug class
Question: What is the drug class of {{Name}}?
Answer: {{Drug class}}

Rule: Side effects
Question: What are the important side effects of {{Name}}?
Answer: {{Side effects}}

Rule: Conditions
Question: What conditions is {{Name}} used to treat?
Answer: {{Conditions}}
```

Placeholder names are case-insensitive for matching but must refer to a field in
the same template. Use complete double-brace placeholders such as `{{Name}}`.

## Generating cards from a reusable template

Open a deck or select exactly one deck in Search/Filter, then choose the custom
template from the **Card template** selector. The editor replaces the normal
Question and Answer fields with one input for every template field.

After completing the form:

1. choose **Preview generated cards** to inspect every question and answer;
2. add any shared tags;
3. choose **Generate cards**.

Every accepted rule becomes an ordinary permanent Basic card in the selected
deck. Exact duplicates are skipped. The generated cards receive normal fresh
schedules and participate in Search/Filter, Practice, analytics, tags,
move/copy, and deck export like manually created Basic cards.

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

Templates perform literal data substitution only. They cannot execute:

- JavaScript or another scripting language;
- SQL;
- shell commands;
- raw HTML;
- filesystem operations;
- network requests;
- a general-purpose expression language.

Rust validates the template definition, field values, placeholder grammar, and
output sizes. Generated questions and answers then pass through the same content
validation and transactional write path as ordinary Basic cards.

## Search/Filter and note-level actions

Search/Filter lists every active generated presentation separately. A forward
card, reverse card, and each cloze blank can therefore have different due dates
and can be selected independently for temporary practice.

Tags, move/copy, and deletion apply to the shared source note. When several
presentations from the same note are selected, ostudy intentionally performs the
note-level action once:

- moving moves every presentation and preserves every schedule;
- copying creates a new note and fresh schedules for all active presentations;
- tags apply to all presentations through the note;
- deleting the note removes every generated presentation.

## Deck export and import

New `.studydeck` exports preserve Basic, reverse, one-at-a-time cloze, and
all-blanks cloze definitions. Imported cards receive fresh local schedules and
do not include the exporting learner's review history.

Reusable templates are not exported with a deck because they are local
-authoring tools rather than content owners. Older Basic packages without Stage
3 metadata remain importable.
