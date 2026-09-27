# Practice, scheduling, and analytics

Practice is a contextual workflow: start from a deck, selected decks, Search results, or linked study-guide cards. It is not a permanent main-navigation destination. Your review history and current schedules are durable library data, while the visible progress and card-stack effects are presentation.

## Choose a scope

**Scheduled practice** selects studyable due work within the chosen deck/tag scope and daily new/review limits. The workload display separates available due work from work omitted by those limits.

**Extra practice** is an explicit choice to study eligible material beyond the scheduled queue. A direct deck Practice action follows the deck-practice scope rather than merely opening an editor. It does not make suspended, buried, archived, or otherwise unavailable cards studyable. Extra practice is not capped at an arbitrary 10,000-card prefix: the whole studyable selected scope can become the session.

**Selected-card practice** uses exact card identities resolved from a current Search selection or study-guide topic link. It is not a temporary deck or a guess based on a tag's display name. Explicit custom-card requests retain a defensive request-size bound because they carry every card identity across the command boundary. If you choose **Save as new deck**, oStudy creates that destination and copies the selected notes in one retry-safe native operation. The originals and their review histories stay where they are; copied presentations receive fresh schedules. If the screen cannot refresh after the native operation commits, the created deck is retained rather than deleted or created a second time.

Read the scope and workload before starting. A loading or failed workload request is not an empty library. Folder membership is a way to select decks; it does not own schedules or create its own review history.

### Large sessions stay windowed

A large selected session does not send the whole deck's rich card content to React. oStudy stores deterministic per-card session snapshots for resume/edit/history semantics, but creates those rich snapshots in bounded backend batches. The practice screen reads 50-card windows and discards consumed selected-session windows instead of accumulating every visited card in memory.

Starting a very large session is still linear work because durable snapshot rows must be created for the selected cards; it is not intended to pretend that 50,000 cards cost the same as 50. The important boundary is that rich card documents and managed-asset references are processed in bounded batches rather than materialized as one giant in-memory deck payload.

## Reveal, check, rate, and continue

Basic and Fill in the blank cards retain the ordinary recall/reveal flow. Try recalling the answer before revealing it, then choose the outcome that reflects what happened:

| Choice | Meaning in this session | Keyboard |
| --- | --- | --- |
| Wrong | You did not recall the answer | 0 |
| Hard | You recalled it with substantial difficulty | 1 |
| Medium | You recalled it with some effort | 2 |
| Easy | You recalled it readily | 3 |

Space or Enter reveals the answer when the practice controls own the interaction. Rating shortcuts are not meant to intercept typing in an editor or an active overlay. The displayed interval previews show the schedule consequences supplied by the native scheduler.

By default, a Basic card shows the **question on the front and the answer on the revealed back**. Enable **Settings → Review/Scheduling → Display both question and answer when card is flipped** if you prefer to keep the Basic question visible beside the answer. Fill in the blank content keeps the blank/context as part of its revealed content so multi-blank questions remain understandable without requiring that global preference.

Single-best-answer, select-all, true/false, matching, grouping, and ordering use **answer → Check answer → feedback → difficulty/continue** instead of click-to-flip. Correctness is evaluated locally from the captured immutable question instance. The first checked answer is frozen; later remediation does not rewrite it. Wrong/incomplete objective answers default to Wrong, while a correct answer permits Hard/Medium/Easy. A deliberate override retains the original objective result and its override reason.

Matching, grouping, and ordering do not auto-submit when the last item moves, and movement never reveals correctness before Check. Dragging is optional: keyboard/non-drag controls support the same logical answer. A compound board still produces one scheduling review rather than one review per tile.

Image occlusion uses a native masked derivative before reveal and is self-assessed. Each occlusion question presentation has its own schedule. Revealing the answer records assistance/self-assessment state appropriately; oStudy does not pretend that an ungraded free-form thought was objectively correct.

Opening Tutor is not itself an assisted attempt. Assistance is recorded only when useful help is actually exposed. A failed/cancelled hint or a response that arrives after the first check cannot change the frozen first-attempt evidence.

The stored confidence choices map to the scheduler's Again, Hard, Good, and Easy ratings. Rating is a durable review, not a cosmetic button response. oStudy validates the previewed schedule revision, appends the review event, updates the current schedule, and advances the session transactionally. Supported retry identities prevent the same submission from counting twice.

A Basic note's forward and reverse presentations have separate schedules even though their editable content and tags are shared. A Fill in the blank note has one scheduled presentation; the hidden group selected for a session remains stable through reveal and resume. An interactive presentation likewise keeps stable identity even if visible item order is permuted for a captured attempt.

## Progress and completion

The active progress track includes studied/remaining counts and a rounded percentage. Assistive technology receives a complete progress description. The current card remains the meaningful content; decorative stacked cards are not additional questions to navigate.

The completion view summarizes the finished work, confidence distribution, remembered proportion, and difficult answers. Optional celebration effects do not control whether reviews were saved. Turning them off or using reduced motion keeps the summary and its controls.

“Remembered” and confidence summaries describe recorded outcomes, not a guarantee of mastery. Historical results and current workload answer different questions: reviewing a card today changes its schedule, but a current due count does not rewrite a past session.

## Leave or resume a session

Ending early asks for confirmation and returns to the logical launch context, such as the Decks root, a particular deck, Search, or Study Guides. Previously committed reviews remain durable. The next route transition is fresh, rather than replaying stale in-memory navigation identities.

Restart recovery uses a saved hint to ask the native library for an actual resumable session. A saved practice-setup screen without a session is not restored as active practice. If a prior deck was removed, a valid session's return destination can fall back safely to the library.

Interactive sessions restore the captured question revision, stable presentation key, item permutation, acknowledged answer draft, first check/reveal state, and feedback boundary from native state. Editing the current source note does not retroactively replace the key for an already captured attempt. A crash after Check but before rating therefore resumes feedback rather than creating a blank question or a duplicate first result.

Unsaved authoring changes opened during a workflow still need to be saved or deliberately discarded. A display refresh error after a committed review is not evidence that the review should be submitted as a new operation.

## Scheduling controls

Scheduling controls are deliberate administrative actions, not review outcomes:

**Suspend** removes a card from ordinary practice until resumed. **Bury** temporarily removes it according to the app's study-day policy. **Reset** returns current scheduling state to a fresh state while preserving retained review history. **Reschedule** changes the next due time without inventing a successful recall event.

Archived decks remain retained but unavailable for normal practice until restored. Deletion has a different lifecycle and eventually removes the history owned by the deleted material after its recovery window is finalized.

Configure defaults in **Settings → Review/Scheduling**. Daily limits and due calculations use the app's native study-calendar rules; they are not recomputed independently by each screen. Bulk schedule actions validate their requested scope and commit as a unit.

## Read Analytics

Analytics presents stored learning activity, confidence/review outcomes, workload, time, and scoped summaries. Use the available date, deck, tag, and question-type controls to identify the question a chart answers. An empty scope means no matching evidence, not necessarily no material in the whole library.

Basic/Fill in the blank recall ratings are **self-assessment**. Objective interactive kinds have a distinct **first-attempt correctness** signal (correct, incorrect, or incomplete). Analytics does not collapse those into one pretend accuracy metric. It also keeps **assistance**, **rating overrides**, and **remediation attempts** separate: an assisted correct response, a learner override, and a post-feedback retry are different evidence.

Per-item choice/pair/group/order feedback is diagnostic detail for the compound question. It does not become multiple successful reviews or a separate mastery score. Image occlusion remains self-assessed and is not counted as objective correctness simply because the learner later rated the review Easy.

Recent reviews retain historical content and classification evidence while applicable records remain in the library. Renaming a deck does not rewrite what a prior review recorded. Moving a card preserves its identity and review evidence under the app's current ownership rules. Archived material remains available for historical analysis; finalized deletion removes the analytics owned by the deleted material.

A current tag on a recent review can open Search by stable tag identity. Historical-only tag text is not a current filter link. An unavailable tag must be cleared explicitly rather than allowing a supposedly scoped action to show unfiltered cards.

Analytics is derived deterministically from stored data, not an LLM assessment. It does not require a cloud provider. Visual charts are presentations of that evidence; the database and native aggregation rules remain authoritative.

## Troubleshooting scope surprises

When fewer cards appear than expected, check whether the session is scheduled, extra, or exact-card; whether the daily limit is reached; and whether cards are archived, suspended, buried, deleted, or outside the selected scope. Check whether you selected visible rows or all matching results. An extra session over a large deck should report the full eligible scope rather than stopping at 10,000 cards.

Do not fix a missing-card surprise by resetting schedules or deleting data before understanding the filter and availability rules. For a defect report, include the scope, mode, displayed counts, exact application build, and a synthetic reproduction where possible. [Backups](backups-imports-and-sharing.md) and [local test isolation](../development/local-testing-and-handoff.md) protect valuable review history during investigation.
