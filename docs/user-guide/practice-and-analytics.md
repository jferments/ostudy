# Practice, scheduling, and Progress

Practice is a contextual workflow: start it from Home, from a deck or subject in the Library Workbench, from selected cards in the Workbench, from a topic in Progress, or from linked study-guide cards. It runs inside **Home**, which the navigation highlights during a session, rather than being a destination of its own. Your review history and current schedules are durable library data; the progress line and stacked-card effects are only presentation.

## Choose a scope

Home's Today strip shows how many cards are due today and about how long they take, with **Start review** for all of them; its arrow lets you choose which subjects to review. **Home** in the main navigation shows a badge with today's count. For a custom practice, open a deck in Library and choose **Practice with options…**. In Library, a subject's header offers **Review N due** for that subject. These review buttons study what today's schedule holds under your daily limits. Today's limits are shared, so reviewing one subject can lower what is left in the others. It also offers **Practice a mix**, one session mixing cards from all of its decks.

In the Workbench, a deck's **Practice with options…** or a subject's **Practice…** opens **Practice selected decks**. It lists the chosen decks, offers the decks' tags to narrow the session, and under **Cards** offers **Due** or **Extra**, each with how many cards it would study. **Shuffle** mixes cards with the same priority.

**Due** takes the cards due in the chosen decks and tags, within your daily new-card and review limits.

**Extra** is an explicit choice to study cards beyond today's schedule. A deck's one-click **Practice** studies the whole deck this way. It shuffles cards with the same priority while keeping due and learning cards first, and a new session does not start with the same card as the last one when another is available. Extra never includes suspended, buried, or archived cards, and it is not cut short at an arbitrary number of cards: every studyable card in the scope can be part of the session.

**Selected-card practice** studies exactly the cards you selected in the Workbench (**Practice…** in the selection toolbar's menu), the cards behind a study-guide topic, or the missed cards offered on **Session complete**. In **Practice these cards**, **Start practice** studies them without changing their decks. **Save as new deck** copies the selected notes into a new deck in one step; the originals and their review histories stay where they are, and the copies start with fresh schedules. If the screen cannot refresh after the deck is created, the deck is kept rather than deleted or created a second time.

Read the scope and the counts before starting. A count that is still loading or failed to load does not mean the library is empty. A subject is a way to choose decks; it does not hold schedules or a review history of its own.

Very large sessions work too: ostudy loads the session's cards a few at a time as you go, so starting one on a big deck takes longer but does not hold every card in memory at once.

## Reveal, check, rate, and continue

Basic and Fill in the blank cards use recall and reveal. Try recalling the answer before revealing it, then choose the rating that reflects what happened:

| Choice | Meaning in this session | Keyboard |
| --- | --- | --- |
| Wrong | You did not recall the answer | 0 |
| Hard | You recalled it with substantial difficulty | 1 |
| Medium | You recalled it with some effort | 2 |
| Easy | You recalled it readily | 3 |

On a wide screen the card and **Show answer** sit together in the middle of the practice screen, and once the answer shows the four ratings take the button's place. A long card scrolls under them, and on a phone they stay just above the bottom navigation, within reach of your thumb. Each rating shows its keyboard number and the interval it would schedule, such as **1 day** or **4 days**; on a phone the number is left out. Space or Enter reveals the answer when no button, field, or overlay has focus, and rating shortcuts do not intercept typing in an editor. The ratings wait until ostudy has worked out each one's next interval; if that could not be calculated, **Retry interval preview** asks again.

By default, a revealed Basic card keeps the **question above the answer**. Turn off **Display both question and answer when card is flipped** in **Settings → Review** to show only the answer on the revealed back. Fill in the blank cards always keep the surrounding text with the revealed answer, so multi-blank questions stay understandable.

When motion is on, the card turns over to show the answer. **Show question** and **Show answer** at the card's top corner turn a revealed card back and forth without changing the review, and clicking the card face does the same. The card itself is ordinary content, so a screen reader reads the question and answer rather than a button label.

Single best answer, select all that apply, true/false, matching, grouping, and ordering questions use **answer → Check answer → feedback → rating** instead of flipping. **Check answer** sits where **Show answer** does. ostudy checks the answer on your device against the question as it was when you started it. The first checked answer is final; trying again afterwards is practice and does not change it. After an incorrect or incomplete answer only **Wrong** is available (the ratings are headed **First answer recorded as wrong**); after a correct one choose **Hard**, **Medium**, or **Easy**.

Matching, grouping, and ordering do not submit on their own when the last item moves, and moving items never shows whether they are right before you choose Check answer. Dragging is optional: buttons and the keyboard can build the same answer. Each tile is a button named by what it says, so a screen reader announces the tile's text and its status, such as Connected. A whole board still counts as one review rather than one per tile.

Image occlusion hides regions of an image until you choose **Reveal answer**, which also sits where **Show answer** does, and you rate yourself afterwards. Each occlusion card has its own schedule. ostudy does not pretend that a free recall can be graded automatically.

Opening Tutor from practice (**Ask Tutor**) is not itself counted as help. A review is marked as assisted only when the tutor actually showed you useful help, and a hint that fails, is cancelled, or arrives after you checked your answer cannot change your first result.

Each rating is saved as a review, not just a button response: ostudy records the review, updates the card's schedule, and moves to the next card in one step. If a rating's save has to be retried, it still counts only once. The four ratings correspond to the scheduler's Again, Hard, Good, and Easy.

A Basic note's forward and reverse cards have separate schedules even though their wording and tags are shared. A Fill in the blank note has one scheduled card; the blank chosen for a session stays the same through reveal and resume. An interactive question keeps the same identity even when its items are shown in a different order.

## Progress and completion

The practice top bar names the deck and shows the studied and remaining counts, with the session's progress as a thin line along its bottom edge. Wider screens also name the kind of practice, such as Extra practice; a phone's app bar already says Practice. Assistive technology hears the card position, the counts, and a rounded percentage. The cards stacked behind the current one are decoration, not extra questions to navigate.

When the last card is rated, **Session complete** shows how many cards you studied and how long it took, how many more are due today, and your study streak. Three tiles show the share you **remembered**, the cards **now solid**, and your **focused time**. If you missed cards, it names the topic most of them share and offers **Practice N** to study just those; otherwise it says you remembered every card. **Done** returns to where you started, **See your progress** opens Progress with this session highlighted, and **Keep going** starts the next cards due today when there are more. If the summary cannot load, your answers are still saved and **Try again** reloads it.

Optional celebration effects do not decide whether reviews were saved. Turning them off or using reduced motion keeps the summary and its buttons.

"Remembered" describes recorded outcomes, not a guarantee of mastery. History and current workload answer different questions: reviewing a card today changes its schedule, but a current due count does not rewrite a past session.

## Leave or resume a session

**End session** in the top bar stops at once and returns to where you started the session, such as Home, Library (on the deck that was open in the Workbench), or Study guides. On Android, Back does the same. Every rating you already gave stays saved: it updated that card's schedule and counts in Progress even though later cards were never reached. Viewing or revealing a card without rating it does not create a review. If a rating is still being saved, ostudy asks you to finish or retry it first.

If ostudy closes during a session, reopening it can resume that session. A practice setup screen that never started a session is not restored as active practice. If the deck that launched the session has since been removed, ending the session returns safely to Library.

Interactive questions resume exactly where they were: the same version of the question, item order, answer so far, and, after Check answer, the feedback. Editing the card in the meantime does not change the answer key of an attempt already in progress, so a crash after Check but before rating resumes the feedback rather than a blank question or a second first result.

Changes made in **Edit card** during practice still need to be saved or deliberately discarded. A display refresh error after a review was saved does not mean the review should be submitted again.

## Scheduling controls

Scheduling controls are deliberate actions, not review outcomes. During practice they live in the **Schedule** menu (the timer icon) beside **Edit card**: **Bury until tomorrow**, **Suspend card**, **Reschedule for tomorrow**, and **Reset schedule**. On a phone, **Edit card** and these actions share one **Card actions** menu, while **End session** stays in the top bar. In the Workbench, a selection's menu and an open card's **…** menu offer similar actions.

**Suspend** removes a card from ordinary practice until you resume it. **Bury** removes it until the next study day starts. **Reset** returns the card to New while keeping its past reviews. **Reschedule** changes the next due date without pretending you recalled the card; **Tomorrow** or a later date makes the card due when that study day starts.

Archived decks are kept but cannot be practiced until restored. Deletion is different: once its **Undo** period ends, the deleted material's review history is removed too.

Set defaults in **Settings → Review**: **Desired retention**, **New cards per day**, **Reviews per day**, and **New day starts at**. Every screen counts due cards and daily limits with the same study-day rules. Schedule actions on several cards apply to all of them together or not at all.

### When a new day starts

A study day starts at 4 a.m. by default, not at midnight, so a late-night session still counts toward the day you started it. Change the hour with **New day starts at** in **Settings → Review**; choose 12 a.m. for days that start at midnight. Reviews before that time count toward the previous day. The same boundary applies to the daily new and review limits, what Home and Library count as due today, Progress's days, weeks, months, and streaks, the Workbench's **Due today** filter and due dates, burying, and rescheduling to tomorrow or a later date.

Changing the hour re-groups all of your existing history by the new boundary; nothing stored is changed, so you can switch back at any time. Each review keeps the time-zone offset of the place where you made it, so travelling does not move your earlier reviews to different days.

## Read Progress

**Progress** shows what you have studied and how well it is sticking. Choose a **Subject** (or **All subjects**), a time range (**7 days**, **30 days**, **90 days**, **Year**, **All time**, or **Custom dates**), and, under **Decks, tags and dates**, particular decks and tags. Chosen filters appear as removable chips with **Clear all**. Dates are study days, which start at the **New day starts at** hour. An empty view means no reviews match, not necessarily that the library is empty. Before anything has been studied, Progress explains what it will show, with a preview of the page after a few weeks, how spaced repetition and FSRS schedule each card, and **Start review**.

From top to bottom:

- **Review N due** beside the filters when cards are due (below the charts on a phone).
- Four tiles: **Remembered**, **Study time**, **Days studied** with your streak, and **Solid cards**, whose (i) says what solid means.
- **Reviews and recall**, a chart of reviews per day, week, or month (remembered and forgotten), which can switch to **Minutes** or **New cards** and to a table; and **Coming up**, the cards due each day for the next two weeks.
- **Needs attention**, your tags ordered with the weakest first, each with **Practice N** for its due cards; and **Card strength**, which sorts cards into **Solid**, **Getting there**, **Learning**, and **New**.
- **By subject**, a table of each subject's card strength, recall, and cards due today, when you are looking at all subjects.
- **Activity** for the last six months, with your current and longest streak and busiest day of the week.
- **Question types**, **Recent sessions**, and **Every answer**, whose **Open review log** lists each answer with its rating, time taken, and date.

**Remembered** counts every review you got right: a checked question's verdict, or any rating above Wrong. A card is **Solid** when it is expected to stay remembered for about three weeks or more. A tag needs attention when its due reviews fall at least 10 points below your desired retention, once it has at least 10 due reviews in the chosen range; a subject shows **Building data** until it has due reviews to measure.

Basic and Fill in the blank ratings are **self-ratings**. Interactive questions also have a **first-check accuracy** (correct, incorrect, or incomplete), and **Question types** keeps the two apart rather than blending them into one accuracy figure. It also counts **Recorded hints**, **Overrides**, and **Retries** separately: a correct answer after a hint, a changed rating, and a retry after feedback are different evidence. Image occlusion is self-rated and is never counted as objectively correct.

Reviews keep what they recorded while the card remains in the library. Renaming a deck does not rewrite what an earlier review recorded, and moving a card keeps its history. Archived material still counts in Progress; deleting material removes its history once the deletion is final.

A tag shown on an answer in the review log opens the Library Workbench on that tag's cards. A tag that has since been deleted shows its old name but is no longer a link.

Progress is calculated from your stored reviews, not judged by an AI model, and needs no cloud provider.

## Troubleshooting scope surprises

When fewer cards appear than expected, check whether the session is scheduled, extra, or selected cards; whether today's limit is reached; and whether cards are archived, suspended, buried, deleted, or outside the chosen decks and tags. In the Workbench, check whether you selected the shown rows (**Select shown**) or every matching card (**Select all N**). Extra over a large deck should include every eligible card.

Do not fix a missing-card surprise by resetting schedules or deleting data before understanding the filters and availability rules. For a defect report, include the scope, mode, displayed counts, exact application build, and a synthetic reproduction where possible. [Backups](backups-imports-and-sharing.md) and [local test isolation](../development/local-testing-and-handoff.md) protect valuable review history during investigation.
