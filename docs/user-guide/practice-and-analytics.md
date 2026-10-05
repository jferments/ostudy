# Practice, scheduling, and analytics

Practice is a contextual workflow: start from Review or Home, a deck, selected decks, Search results, or linked study-guide cards. It runs inside **Review**, which the navigation highlights during a session, rather than being a destination of its own. Your review history and current schedules are durable library data, while the visible progress and card-stack effects are presentation.

## Choose a scope

**Review** in the main navigation shows how many cards are due today and about how long they take, with **Start review** for all of them, a **Start** for each subject with due cards, and **Choose decks**, which opens Library's Decks view for a custom practice. Home's Today strip starts the same review. Both count what a scheduled session studies today under your daily limits.

**Scheduled practice** selects studyable due work within the chosen deck/tag scope and daily new/review limits. The workload display separates available due work from work omitted by those limits.

**Extra practice** is an explicit choice to study eligible material beyond the scheduled queue. A direct deck **Practice** action follows the deck-practice scope rather than merely opening an editor. One-click deck Practice always shuffles cards that have the same scheduling priority while preserving due-date and learning-priority ordering; a new session receives a fresh permutation and avoids immediately repeating the previous first card when an equivalent alternative exists. It does not make suspended, buried, archived, or otherwise unavailable cards studyable. Extra practice is not capped at an arbitrary 10,000-card prefix: the whole studyable selected scope can become the session.

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

**Show answer** and, once the answer shows, the four ratings sit together at the bottom of the practice screen. A long card scrolls under them, and on a phone they stay just above the bottom navigation, within reach of your thumb. Each rating shows its keyboard number. Space or Enter reveals the answer when the practice controls own the interaction. Rating shortcuts are not meant to intercept typing in an editor or an active overlay. The ratings wait for the native scheduler's preview of the next schedule; if that preview could not be calculated, **Retry interval preview** asks for it again.

By default, a Basic card shows the **question on the front and the answer on the revealed back**. Enable **Settings → Review/Scheduling → Display both question and answer when card is flipped** if you prefer to keep the Basic question visible beside the answer. Fill in the blank content keeps the blank/context as part of its revealed content so multi-blank questions remain understandable without requiring that global preference.

When motion is on, the card turns over to show the answer. **Show question** and **Show answer** at the card's top corner turn a revealed card back and forth without changing the review, and clicking the card face does the same. The card itself is ordinary content, so a screen reader reads the question and answer rather than a button label.

Single-best-answer, select-all, true/false, matching, grouping, and ordering use **answer → Check answer → feedback → difficulty/continue** instead of click-to-flip. **Check answer** sits where **Show answer** does. Correctness is evaluated locally from the captured immutable question instance. The first checked answer is frozen; later remediation does not rewrite it. Wrong/incomplete objective answers default to Wrong, while a correct answer permits Hard/Medium/Easy. A deliberate override retains the original objective result and its override reason.

Matching, grouping, and ordering do not auto-submit when the last item moves, and movement never reveals correctness before Check. Dragging is optional: keyboard/non-drag controls support the same logical answer. Each tile is a button named by what it says, so a screen reader announces the tile's text and its status, such as Connected. A compound board still produces one scheduling review rather than one review per tile.

Image occlusion uses a native masked derivative before reveal and is self-assessed; its **Reveal answer** also sits where **Show answer** does. Each occlusion question presentation has its own schedule. Revealing the answer records assistance/self-assessment state appropriately; oStudy does not pretend that an ungraded free-form thought was objectively correct.

Opening Tutor is not itself an assisted attempt. Assistance is recorded only when useful help is actually exposed. A failed/cancelled hint or a response that arrives after the first check cannot change the frozen first-attempt evidence.

The stored confidence choices map to the scheduler's Again, Hard, Good, and Easy ratings. Rating is a durable review, not a cosmetic button response. oStudy validates the previewed schedule revision, appends the review event, updates the current schedule, and advances the session transactionally. Supported retry identities prevent the same submission from counting twice.

A Basic note's forward and reverse presentations have separate schedules even though their editable content and tags are shared. A Fill in the blank note has one scheduled presentation; the hidden group selected for a session remains stable through reveal and resume. An interactive presentation likewise keeps stable identity even if visible item order is permuted for a captured attempt.

## Progress and completion

The practice top bar names the deck and shows the studied and remaining counts, with the session's progress as a thin line along its bottom edge. Wider screens also name the kind of practice, such as Extra practice; a phone's app bar already says Practice. Assistive technology receives a complete progress description: the card position, the counts and a rounded percentage. The current card remains the meaningful content; decorative stacked cards are not additional questions to navigate.

The completion view summarizes the finished work, confidence distribution, remembered proportion, and difficult answers. Optional celebration effects do not control whether reviews were saved. Turning them off or using reduced motion keeps the summary and its controls.

“Remembered” and confidence summaries describe recorded outcomes, not a guarantee of mastery. Historical results and current workload answer different questions: reviewing a card today changes its schedule, but a current due count does not rewrite a past session.

## Leave or resume a session

Ending early asks for confirmation and returns to the logical launch context, such as Review, Home, the Decks root, a particular deck, Search, or Study guides. Every rating that was already accepted remains a durable review event: it updates that card's schedule and is counted by Progress even though later cards in the session were never reached. Merely viewing or revealing an unrated card does not invent a review. The next route transition is fresh, rather than replaying stale in-memory navigation identities.

Restart recovery uses a saved hint to ask the native library for an actual resumable session. A saved practice-setup screen without a session is not restored as active practice. If a prior deck was removed, a valid session's return destination can fall back safely to the library.

Interactive sessions restore the captured question revision, stable presentation key, item permutation, acknowledged answer draft, first check/reveal state, and feedback boundary from native state. Editing the current source note does not retroactively replace the key for an already captured attempt. A crash after Check but before rating therefore resumes feedback rather than creating a blank question or a duplicate first result.

Unsaved authoring changes opened during a workflow still need to be saved or deliberately discarded. A display refresh error after a committed review is not evidence that the review should be submitted as a new operation.

## Scheduling controls

Scheduling controls are deliberate administrative actions, not review outcomes. During practice they live in the **Schedule** menu (the timer icon) beside **Edit card**. On a phone, **Edit card** and these actions share one **Card actions** menu, while **End session** stays in the top bar.

**Suspend** removes a card from ordinary practice until resumed. **Bury** removes it until the next study day starts. **Reset** returns current scheduling state to a fresh state while preserving retained review history. **Reschedule** changes the next due time without inventing a successful recall event; **Tomorrow** or a later date makes the card due when that study day starts.

Archived decks remain retained but unavailable for normal practice until restored. Deletion has a different lifecycle and eventually removes the history owned by the deleted material after its recovery window is finalized.

Configure defaults in **Settings → Review/Scheduling**. Daily limits and due calculations use the app's native study-calendar rules; they are not recomputed independently by each screen. Bulk schedule actions validate their requested scope and commit as a unit.

### When a new day starts

A study day starts at 4 a.m. by default, not at midnight, so a late-night session still counts toward the day you started it. Change the hour with **New day starts at** in **Settings → Review/Scheduling**; choose 12 a.m. for days that start at midnight. Reviews before that time count toward the previous day. The same boundary applies to the daily new and review limits, what Home and Review count as due today, the Today counts in Progress and its timeline days, weeks and months, Search/Filter's reviewed-today and recent filters, burial, and rescheduling to tomorrow or a later date.

Changing the hour re-groups all of your existing history by the new boundary; nothing stored is changed, so you can switch back at any time. Each review keeps the time-zone offset of the place where you made it, so travelling does not move your earlier reviews to different days.

## Read Progress

**Progress** presents stored learning activity, confidence/review outcomes, workload, time, and scoped summaries. Use the available date, deck, tag, and question-type controls to identify the question a chart answers. Dates are study days, which start at the **New day starts at** hour. An empty scope means no matching evidence, not necessarily no material in the whole library.

**Remembered** counts every review you got right: a checked question's verdict, or any rating above Wrong. **Of due reviews** narrows that to cards that were actually due for review, leaving out new cards, learning steps and early extra practice; that is the number to compare with your desired retention. A topic **Needs attention** when its due reviews fall at least 10 points below your desired retention, and shows **Building data** until it has 10 due reviews in the selected range. **Reviews due now** counts reviews waiting for you; new cards available to learn are listed beside it.

Basic/Fill in the blank recall ratings are **self-assessment**. Objective interactive kinds have a distinct **first-attempt correctness** signal (correct, incorrect, or incomplete). Progress does not collapse those into one pretend accuracy metric. It also keeps **assistance**, **rating overrides**, and **remediation attempts** separate: an assisted correct response, a learner override, and a post-feedback retry are different evidence.

Per-item choice/pair/group/order feedback is diagnostic detail for the compound question. It does not become multiple successful reviews or a separate mastery score. Image occlusion remains self-assessed and is not counted as objective correctness simply because the learner later rated the review Easy.

Recent reviews retain historical content and classification evidence while applicable records remain in the library. Renaming a deck does not rewrite what a prior review recorded. Moving a card preserves its identity and review evidence under the app's current ownership rules. Archived material remains available for historical analysis; finalized deletion removes the analytics owned by the deleted material.

A current tag on a recent review can open Search by stable tag identity. Historical-only tag text is not a current filter link. An unavailable tag must be cleared explicitly rather than allowing a supposedly scoped action to show unfiltered cards.

Progress is derived deterministically from stored data, not an LLM assessment. It does not require a cloud provider. Visual charts are presentations of that evidence; the database and native aggregation rules remain authoritative.

## Troubleshooting scope surprises

When fewer cards appear than expected, check whether the session is scheduled, extra, or exact-card; whether the daily limit is reached; and whether cards are archived, suspended, buried, deleted, or outside the selected scope. Check whether you selected visible rows or all matching results. An extra session over a large deck should report the full eligible scope rather than stopping at 10,000 cards.

Do not fix a missing-card surprise by resetting schedules or deleting data before understanding the filter and availability rules. For a defect report, include the scope, mode, displayed counts, exact application build, and a synthetic reproduction where possible. [Backups](backups-imports-and-sharing.md) and [local test isolation](../development/local-testing-and-handoff.md) protect valuable review history during investigation.
