import type { TutorialModuleId } from "../tutorials/tutorialRegistry";

export type HelpTopicId =
  | "flashcards.subjects"
  | "flashcards.search.studyState"
  | "flashcards.search.selection"
  | "templates.layout"
  | "templates.builtIn"
  | "templates.questionMix"
  | "documents.provider"
  | "documents.processing"
  | "practice.confidence"
  | "practice.dailyLimits"
  | "settings.motion"
  | "settings.retention"
  | "settings.reveal"
  | "settings.backup"
  | "settings.aiServices"
  | "settings.deviceSync";

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
  "templates.builtIn": {
    id: "templates.builtIn",
    title: "Built-in templates",
    summary: "The card formats ostudy ships with. A switch on means AI may write that format when it makes cards from your documents.",
    details:
      "Turning a format off never changes cards you already have or tasks that are running. Image occlusion is always made by hand. You can still write any format yourself in the Workbench.",
    documentation: "Documents and AI-generated learning material: Generated card formats",
  },
  "templates.questionMix": {
    id: "templates.questionMix",
    title: "Question mix",
    summary: "Weights say which formats AI should prefer. They are preferences, not quotas.",
    details:
      "Covering the source and writing clear, answerable questions come first; if a source does not suit a format, ostudy uses another one or makes fewer cards rather than filler. A weight of zero keeps a format allowed but never preferred; switch it off to rule it out. Most parts in one question limits the options, pairs, items or steps in matching, grouping, ordering and select-all questions to 2 to 16.",
    documentation: "Documents and AI-generated learning material: Generated card formats",
  },
  "documents.provider": {
    id: "documents.provider",
    title: "AI model for documents",
    summary: "Documents make cards and study guides with the model chosen for Flashcard and study-guide generation in Settings.",
    details:
      "In Settings → AI, add a provider under AI providers, then choose it and a model for Flashcard and study-guide generation under Default AI models. A saved API key stays in this device's secure storage. The source stays in your library, and the cards it makes become an ordinary deck you can edit.",
    documentation: "AI providers and model profiles: Choose a model for each task",
    tutorialModuleId: "documents",
  },
  "documents.processing": {
    id: "documents.processing",
    title: "Document processing",
    summary: "ostudy keeps the source in your library, reads it on this device, and makes an ordinary deck.",
    details:
      "Large files can take a while. Use Stop in the document's menu to stop, Retry when a document failed, and Make more cards… to add cards from it later.",
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
      "Scheduled study keeps to New cards per day and Reviews per day from Settings → Review, and Cards available to study shows what the limits leave out. A limit of 0 pauses that kind of card in scheduled sessions. Reviews before New day starts at count toward the previous day. Choose Extra study when you want to study beyond today's schedule; shuffling is chosen when you start a session.",
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
  "settings.retention": {
    id: "settings.retention",
    title: "Desired retention",
    summary: "How likely you want to be to remember a card when it comes back. Higher retention means shorter intervals and more reviews.",
    details:
      "The FSRS scheduler picks each card's next date so that you recall it with about this probability. Changing it affects future scheduling only; your review history and current schedules stay as they are.",
    documentation: "Practice, scheduling, and Progress: Scheduling controls",
  },
  "settings.reveal": {
    id: "settings.reveal",
    title: "Answer side",
    summary: "Basic cards show only the answer when flipped unless you keep the question above it.",
    details:
      "Fill in the blank cards always keep their surrounding text after reveal, so questions with several blanks stay understandable.",
    documentation: "Practice, scheduling, and Progress: Reveal, check, rate, and continue",
  },
  "settings.backup": {
    id: "settings.backup",
    title: "Library backups",
    summary: "A backup holds your whole library. API keys and session-only credentials are never included.",
    details:
      "Backups include documents, source files, study guides, subjects, Tutor history, decks, templates, images, drafts, schedules, reviews, provider settings and app settings. Restoring replaces the current library after it checks the backup.",
    documentation: "Backups, imports, and sharing: Create a whole-library backup",
  },
  "settings.aiServices": {
    id: "settings.aiServices",
    title: "AI services",
    summary: "A service is a connection to an AI provider. Each task then uses one of your services and a model.",
    details:
      "Cloud services need an API key, kept in this device's secure store unless you choose this session only. A llama.cpp server on this computer or your network needs no key unless you set one. Test model checks that a task's model works with ostudy before you rely on it.",
    documentation: "AI providers and model profiles: Add a provider",
  },
  "settings.deviceSync": {
    id: "settings.deviceSync",
    title: "Device sync",
    summary: "Send chosen study data straight to another of your devices over your local network or a USB cable. There is no cloud account.",
    details:
      "The receiving device merges what it gets and never sends its own library back. Appearance and other app settings stay on each device. When both devices changed the same thing, it waits here for you to choose which version to keep.",
    documentation: "Synchronize trusted devices: Choose what to synchronize",
  },
};
