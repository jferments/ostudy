import type { TutorialModuleId } from "../tutorials/tutorialRegistry";

export type HelpTopicId =
  | "flashcards.subjects"
  | "flashcards.search.studyState"
  | "flashcards.search.selection"
  | "templates.layout"
  | "documents.provider"
  | "documents.processing"
  | "practice.confidence"
  | "practice.dailyLimits"
  | "settings.motion";

export interface HelpTopic {
  id: HelpTopicId;
  title: string;
  summary: string;
  details: string;
  /** The user guide and section that covers the topic, as "Guide title: Section heading". */
  documentation: string;
  tutorialModuleId?: TutorialModuleId;
}

export const helpRegistry: Readonly<Record<HelpTopicId, HelpTopic>> = {
  "flashcards.subjects": {
    id: "flashcards.subjects",
    title: "Subjects",
    summary: "Subjects group a course's decks and documents without changing their cards, schedules, or review history.",
    details:
      "Choose + beside Subjects to create one. A deck's menu has Move to subject…, Move up, and Move down; a subject's menu has New deck here, Rename, Move up, Move down, and Delete subject. Collapsing a subject only changes what the tree shows. Deleting a subject moves its decks and documents to No subject and removes its overview guide; it never deletes cards or review history.",
    documentation: "Subjects and deck organization: Subjects hold decks and documents",
    tutorialModuleId: "orientation",
  },
  "flashcards.search.studyState": {
    id: "flashcards.search.studyState",
    title: "Show filters",
    summary: "Show narrows the card list inside the subject, deck, or tag you chose.",
    details:
      "Due today lists cards that are ready to study now. New lists cards you have not studied yet. Often missed lists cards you missed on your last two reviews, or on three of your last five. Suspended lists cards kept out of study until you resume them. Each choice shows its count.",
    documentation: "Library Workbench, authoring, and search: The card list",
    tutorialModuleId: "search",
  },
  "flashcards.search.selection": {
    id: "flashcards.search.selection",
    title: "Selecting cards",
    summary: "Select the cards shown, or every card that matches the list.",
    details:
      "Select shown selects only the rows loaded so far. Select all selects every card that matches the current list, including rows not loaded yet, before you act on them.",
    documentation: "Library Workbench, authoring, and search: Select cards and act on them",
  },
  "templates.layout": {
    id: "templates.layout",
    title: "Template layouts",
    summary: "A template defines reusable fields and how they appear on the question and answer sides of its cards.",
    details:
      "Fields hold study content. Rules place those fields on the question and answer sides without running scripts or arbitrary HTML.",
    documentation: "Library Workbench, authoring, and search: Reusable templates",
  },
  "documents.provider": {
    id: "documents.provider",
    title: "AI model for documents",
    summary: "Documents make cards and study guides with the model chosen for Flashcard and study-guide generation in Settings.",
    details:
      "In Settings → AI/LLM, add a provider under AI providers, then choose it and a model for Flashcard and study-guide generation under Default AI models. A saved API key stays in this device's secure storage. The source stays in your library, and the cards it makes become an ordinary deck you can edit.",
    documentation: "AI providers and model profiles: Choose a model for each task",
    tutorialModuleId: "documents",
  },
  "documents.processing": {
    id: "documents.processing",
    title: "Document processing",
    summary: "ostudy keeps the source in your library, reads it on this device, and makes an ordinary deck.",
    details:
      "Large files can take a while. Use Stop processing in the document's menu to stop, Retry when a document failed, and Make more cards… to add cards from it later.",
    documentation: "Documents and AI-generated learning material: Progress and stopping",
    tutorialModuleId: "documents",
  },
  "practice.confidence": {
    id: "practice.confidence",
    title: "Rating your answer",
    summary: "Use 0 for Wrong, 1 for Hard, 2 for Medium, and 3 for Easy.",
    details:
      "Choose the rating that matches how well you actually recalled the answer. It saves a review and sets when this card comes back. Each card keeps its own schedule, including the reverse card of the same note.",
    documentation: "Practice, scheduling, and Progress: Reveal, check, rate, and continue",
    tutorialModuleId: "practice",
  },
  "practice.dailyLimits": {
    id: "practice.dailyLimits",
    title: "Daily limits",
    summary: "Daily limits manage your workload without deleting or permanently hiding due cards.",
    details:
      "Scheduled study keeps to New cards per day and Reviews per day from Settings → Review/Scheduling, and Cards available to study shows what the limits leave out. Choose Extra study when you want to study beyond today's schedule.",
    documentation: "Practice, scheduling, and Progress: Choose a scope",
  },
  "settings.motion": {
    id: "settings.motion",
    title: "Motion preference",
    summary:
      "Motion can follow the operating system, use full spatial feedback, reduce motion, or turn nonessential motion off.",
    details:
      "Reduced motion removes card flips, parallax, spatial route movement, reorder interpolation, and particle celebrations while preserving clear state changes.",
    documentation: "Appearance, accessibility, help, and tutorials: Motion",
  },
};
