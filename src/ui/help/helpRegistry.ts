import type { TutorialModuleId } from "../tutorials/tutorialRegistry";

export type HelpTopicId =
  | "flashcards.folders"
  | "flashcards.deckDensity"
  | "flashcards.search.studyState"
  | "flashcards.search.selection"
  | "templates.layout"
  | "documents.provider"
  | "documents.processing"
  | "practice.confidence"
  | "practice.dailyLimits"
  | "settings.materialEffects"
  | "settings.motion";

export interface HelpTopic {
  id: HelpTopicId;
  title: string;
  summary: string;
  details: string;
  documentation: string;
  tutorialModuleId?: TutorialModuleId;
}

export const helpRegistry: Readonly<Record<HelpTopicId, HelpTopic>> = {
  "flashcards.folders": {
    id: "flashcards.folders",
    title: "Organizing decks into folders",
    summary: "Folders group related decks without changing their cards or review history.",
    details:
      "Move a deck with its drag handle, keyboard move commands, or the Move to folder menu. Deleting a folder returns its decks to Unfiled unless you explicitly delete the decks separately.",
    documentation: "getting-started#organize-flashcards",
    tutorialModuleId: "orientation",
  },
  "flashcards.deckDensity": {
    id: "flashcards.deckDensity",
    title: "Deck list density",
    summary: "Comfortable view emphasizes visual summaries; compact view fits more decks on screen.",
    details:
      "Density changes presentation only. It does not alter deck order, schedules, cards, or review history.",
    documentation: "getting-started#deck-library",
  },
  "flashcards.search.studyState": {
    id: "flashcards.search.studyState",
    title: "Study-state filters",
    summary: "Narrow cards by due status, review history, or scheduling state.",
    details:
      "Ready now follows the current local study calendar. Suspended and buried cards are excluded from ordinary study until resumed or unburied. Applied filters remain visible as removable chips.",
    documentation: "getting-started#search-and-filter",
    tutorialModuleId: "search",
  },
  "flashcards.search.selection": {
    id: "flashcards.search.selection",
    title: "Selecting search results",
    summary: "Select visible cards or every card matching the current filters.",
    details:
      "Select visible affects only loaded rows. Select all matching resolves the complete filtered set before performing a bulk action.",
    documentation: "getting-started#bulk-card-management",
  },
  "templates.layout": {
    id: "templates.layout",
    title: "Template layouts",
    summary: "A template defines reusable fields and how they appear on generated card sides.",
    details:
      "Fields store study content. Rules place those fields on question and answer sides without executing scripts or arbitrary HTML.",
    documentation: "getting-started#card-templates",
  },
  "documents.provider": {
    id: "documents.provider",
    title: "Flashcard-generation model",
    summary: "Documents use the provider profile selected for flashcard generation.",
    details:
      "Provider credentials remain in the native secret-storage boundary. The document screen only shows the selected profile and processing status.",
    documentation: "documents-and-ai-generation#select-a-model",
    tutorialModuleId: "documents",
  },
  "documents.processing": {
    id: "documents.processing",
    title: "Document processing",
    summary: "ostudy stores the source locally, extracts structure, and creates a normal deck.",
    details:
      "Large files can take time. You can stop generation, retry recoverable failures, and keep the source document for future generation passes.",
    documentation: "documents-and-ai-generation#processing-states",
    tutorialModuleId: "documents",
  },
  "practice.confidence": {
    id: "practice.confidence",
    title: "Confidence scores",
    summary: "Use 0 for Wrong, 1 for Hard, 2 for Medium, and 3 for Easy.",
    details:
      "Choose the score that matches actual recall. ostudy uses the response and review history to schedule the individual generated card's next presentation.",
    documentation: "getting-started#practice",
    tutorialModuleId: "practice",
  },
  "practice.dailyLimits": {
    id: "practice.dailyLimits",
    title: "Daily limits",
    summary: "Daily limits manage workload without deleting or permanently hiding due cards.",
    details:
      "Scheduled study respects the limits. Extra practice remains available when you intentionally choose a deck or custom scope.",
    documentation: "getting-started#daily-workload",
  },
  "settings.materialEffects": {
    id: "settings.materialEffects",
    title: "Material effects",
    summary: "Material effects control depth, translucency, blur, and texture independently from motion.",
    details:
      "Automatic uses restrained effects and follows reduced-transparency or forced-color preferences. Simple materials use opaque surfaces and no grain.",
    documentation: "getting-started#appearance",
  },
  "settings.motion": {
    id: "settings.motion",
    title: "Motion preference",
    summary:
      "Motion can follow the operating system, use full spatial feedback, reduce motion, or turn nonessential motion off.",
    details:
      "Reduced motion removes card flips, parallax, spatial route movement, reorder interpolation, and particle celebrations while preserving clear state changes.",
    documentation: "getting-started#appearance",
  },
};
