# Parallel artifact lifecycle regression

## Guide-first publication must not finish flashcards

Initial document generation shares understanding and a learning plan, then runs
boxed flashcard and study-guide futures concurrently. Future independence alone
is insufficient: each publication path must also respect durable state ownership.

`publish_document_study_guide` previously changed every non-archived document to
`ready`, cleared its error, and set its completion timestamp. When the guide
finished before flashcards, the Documents controller treated that row as a
terminal flashcard result. With no published deck it displayed the generic
`Document generation failed.` message and stopped tracking that background job.
The eventual initial-card transaction also requires `lifecycle_state =
'processing'`; the guide's premature transition made that guard fail and could
roll back an otherwise valid card import.

The guide publication transaction now preserves an already-processing document's
lifecycle, error fields, and completion timestamp. The guide remains independently
published and its artifact job can show Complete while flashcards continue.
Flashcard finalization or failure owns the later document transition. Existing
standalone guide recovery and archived-document behavior are retained.

A document can remain Ready after an actual flashcard failure because it has a
usable study guide. The flashcard progress row must still show Needs attention
(or Stopped for cancellation) when a terminal card error exists without a deck.
Its saved-window percentage is not proof of a published deck. An active retry
must not be mistaken for a terminal result because of a stale error field.

No prompt, output schema, checkpoint identity, provider model, or generation
budget changes are part of this correction. Do not invalidate already-paid
checkpoints to install it or remove the initial-card transaction's state guard.

## Regression coverage

`document_study_guides::parallel_publication_tests` exercises the real guide
publication transaction and checks guide-first card-commit eligibility, retained
card-owned metadata, and completed/archived lifecycle preservation. Existing
standalone guide recovery tests remain in place. These are database-boundary
regressions, not an end-to-end live-provider generation test.

`LearningArtifactProgress.test.tsx` covers a guide finishing with cards at 12/15
windows, a Ready guide-only document with failed cards, cancellation, and an
active retry. Both artifact rows remain independent. The normal Fast gate runs
these React tests and the native `document_` test filter.

## Separate retry-cost investigation

A lifecycle race does not explain dozens of contract repairs. The reported
70-plus contract retries and two-million-plus tokens require the failed run's
stage/window validation errors and physical-attempt accounting. Do not attribute
all usage to flashcard repair or claim a measured token reduction from this fix.

Capture the existing run without issuing new provider requests:

```bash
cd ~/code/ostudy
./scripts/export-diagnostics.sh
```

The maintained exporter discovers the library and operation for its usage report,
includes recent current/previous logs, and prints the archive path. It excludes
the SQLite library, cards, and managed images. Inspect retained diagnostics before
starting another generation, which could replace the latest-operation selection
or rotate useful failure evidence.
