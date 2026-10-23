# Architecture decision index and reference policy

Architecture Decision Records preserve a decision's rationale and historical context. Current behavior is described by [the architecture overview](../architecture/overview.md) and its focused owners. A later decision can supersede part of an earlier record without making the earlier reasoning disposable.

## Unambiguous references

Use the **full filename and a link**, preferably with a short title. A numeric prefix is not a unique key in this repository: four historical numbers were allocated twice. Do not infer ordering, ownership or supersession solely from the number, and do not delete or rename a record just to make numbering look tidy.

| Historical prefix | Distinct records that must not be conflated |
| --- | --- |
| 0004 | [Safe card content and templates](0004-safe-card-content-and-templates.md); [Stage 5 scheduling controls](0004-stage-5-scheduling-controls.md) |
| 0005 | [Card organization and search](0005-card-organization-search.md); [Learning analytics](0005-learning-analytics.md) |
| 0026 | [Historical Android ML Kit OCR](0026-android-ml-kit-ocr.md); [Runtime platform ports and composition](0026-runtime-platform-ports-and-composition.md) |
| 0046 | [Modular AI infrastructure](0046-modular-ai-infrastructure.md); [Semantic retrieval backend](0046-semantic-retrieval-backend.md) |

Existing paths and inbound links are retained. A future renumbering requires a coordinated reference audit and compatibility pointers; none is planned. Before creating a new ADR, inspect the actual current directory and choose an unused prefix. Do not reserve numbers from an old conversation.

## Important supersessions and current entry points

- The top-level navigation in [learning-workflow UI simplification](0049-learning-workflow-ui-simplification.md) (Flashcards, Study Guides, Documents, Analytics, Settings) and the Flashcards workspace routes in [ADR 0034](0034-semantic-visual-system-and-flashcards-workspace.md) and [ADR 0039](0039-root-activated-flashcards-routes-and-responsive-layout.md) are replaced by the five destinations Home, Library, Tutor, Progress and Settings, with Review on Home and Library organized by subject under its Overview, Lessons, Study guides, Documents and Flashcards (the Workbench) tabs. No ADR records that change; [UI architecture](../architecture/ui.md) owns the current shell. Practice as an internal workflow and the checkpoint compatibility they established still hold, and Progress replaced the Analytics screen.
- [Subjects](0055-subjects.md) replaces deck folders and document collections with one subject per course, folds collection copies back into their original cards and makes studying a collection a live view of its decks. Earlier records that describe folders, several collections per document or copied collection decks explain history.
- [One subject list in Library](0059-one-subject-list-in-library.md) makes Library's left pane the only subject list, gives each subject an Overview home, removes the subject level from the Workbench tree and stops naming collections in the interface.
- [Library subject tiles](0060-library-subject-tiles.md) opens Library on a page of subject tiles, keeps the tabs inside a subject (adding Podcasts), and makes Notes a destination and Settings a header action; it supersedes 0059's Library layout.
- [Claude through the Messages API at the provider edge](0057-claude-messages-api-at-the-provider-edge.md) adds Claude as a dialect that lowers the shared OpenAI-compatible request to Anthropic's Messages API; it extends, rather than replaces, [one OpenAI-compatible provider family](0047-openai-compatible-provider-dialects.md).
- [One set of learning-metric definitions](0056-learning-metric-definitions.md) defines remembered, true retention, ready now and due today once for every surface, and replaces the topic classification in [Learning analytics](0005-learning-analytics.md) with one based on true retention.
- [Study days start at a configurable hour](0054-study-day-start.md) moves the day boundary for daily limits, Progress (then Analytics), card search date filters, burial and date-only rescheduling from local midnight to the **New day starts at** hour, 4 a.m. by default. "Local calendar day" in [Stage 5 scheduling controls](0004-stage-5-scheduling-controls.md) and [Learning analytics](0005-learning-analytics.md) now means that study day; [ADR 0010](0010-study-calendar-historical-attribution-and-projections.md) still defines what a review attribution stores.
- [Open-source Android imaging](0052-open-source-android-imaging.md) records the current Tesseract4Android/Tesseract and JourneyApps ZXing/ZXing choice. Earlier ML Kit and Google Code Scanner records explain history, not current runtime dependencies.
- [shadcn/Tailwind presentation architecture](0051-shadcn-tailwind-presentation-architecture.md) supersedes the CSS-Modules-first ordinary presentation rule in [frontend state and style ownership](0048-frontend-state-and-style-ownership.md). The older state-ownership reasoning remains relevant; [UI architecture](../architecture/ui.md) describes the current mixed specialized-CSS and utility ownership.
- [Versioned UI checkpoint and workflow restoration](0050-versioned-ui-checkpoint-workflow-restoration.md) preserves the distinction between a destination checkpoint and authoritative study-session state.
- [Xberg non-PDF adapter](0045-xberg-non-pdf-document-adapter.md) describes the current non-PDF adapter choice; old parser names in migrations are compatibility provenance, not authorization to restore an obsolete backend.
- [Library restore ownership](../architecture/library-restore.md) and the [PR #106 completion record](../development/lk-followups/consolidation-qualification.md) distinguish the current release-v1 baseline, supported historical inputs, same-process activation and exact-SHA acceptance.

The directory contains the complete records; this index highlights ambiguity and current supersessions rather than duplicating every decision. Proposed runtime changes in the [hardening register](../development/post-106-hardening.md) are not accepted historical ADRs and are not current implementation claims.
