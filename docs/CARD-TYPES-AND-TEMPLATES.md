# Card types and safe templates

Phase 2 Stage 3 adds reverse cards, Fill in the blank notes, and reusable
**data-only template forms**. The three features deliberately do not use the
same ownership model.

## Basic and reverse cards

A Basic note contains the existing rich-text **Question** and **Answer** fields.
It always owns a forward card. Selecting **Also create a reverse card** adds a
second independently scheduled presentation with the answer as its prompt and
the question as its answer.

Turning reverse off temporarily retires only that presentation. Turning it on
again revives the same card ID and schedule. The source note remains one editable
record, so correcting either side updates both directions without duplicating
content.

## Fill in the blank

Fill-in-the-blank notes store plain source text with explicit data markers:

```text
{{c1::ACE inhibitors}} block formation of {{c2::angiotensin II}}.
```

Authors normally create markers by selecting text and clicking **Mark selection
as blank**.

- **One blank per card** gives each marked region a group and creates one
  independently scheduled card per group. The active group is hidden while the
  other marked regions remain visible.
- **Hide all blanks together** creates one card and hides every marked region.

Marker groups are used as durable card ordinals. Reordering text does not reset a
blank's schedule as long as its group number remains the same. Markers are
bounded data, not HTML or executable expressions.

## Reusable templates are authoring shortcuts

A saved template defines an input form and one or more literal question/answer
rules. A Pharmacology template can define:

```text
name
drug class
side effects
conditions
```

and rules such as:

```text
What is the drug class of {{name}}?  ->  {{drug class}}
What are the side effects of {{name}}?  ->  {{side effects}}
What conditions is {{name}} used to treat?  ->  {{conditions}}
```

Submitting the form expands those rules in one transaction and creates three
**ordinary Basic cards** in the selected deck. From that point forward they are
indistinguishable from cards created with **Add card**:

- each card is a normal note plus one forward card;
- each has its own ordinary FSRS schedule and review history;
- search, tags, analytics, move/copy, export, and deletion use the normal paths;
- no template ID or field-value payload is stored on the generated cards; and
- editing or deleting the template cannot alter or delete existing cards.

Templates therefore remain safe to reorganize. They are reusable bulk-entry
forms, not durable card types and not parents of generated cards.

## Data-only language

Template patterns support only literal Unicode text and exact
`{{field name}}` substitution for declared fields. The backend rejects malformed
braces and unknown fields. It never evaluates JavaScript, SQL, shell commands,
raw HTML, filesystem operations, network requests, loops, conditionals, property
access, or a general expression language.

Rendered questions and answers become ordinary validated text documents before
being stored. Duplicate handling matches Bulk Add: exact normalized duplicates
already in the destination deck, or repeated within the same generation request,
are skipped and reported.

## Persistence and compatibility

Schema 6 adds:

- the independent `card_templates` definition table;
- `reverse_enabled` on source notes; and
- `cloze_text` plus `cloze_mode` on source notes.

Every new note column has a safe default, so existing schema-5 Basic cards remain
Basic forward cards with unchanged IDs, schedules, and review events. This is
**data compatibility**, not a compatibility shim: removed IPC fields and obsolete
code paths continue to fail visibly rather than being silently accepted.

`.studydeck` version 2 preserves reverse/cloze source data and every generated
card direction/ordinal. The importer continues to accept version-1 Basic-only
packages. Templates themselves are application authoring tools and are not
required to travel with a deck; cards generated from them are already ordinary
cards and export normally.
