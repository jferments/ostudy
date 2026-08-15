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
    title: "Deck folders",
    summary: "Folders organize decks without changing their cards, schedules, or review history.",
    details:
      "A deck belongs to at most one folder. Dragging is optional: the Move action and up/down controls provide the same operation. Deleting a folder moves its decks to Unfiled and does not delete cards or historical learning evidence.",
    documentation: "Flashcards and deck organization",
    tutorialModuleId: "orientation",
  },
  "flashcards.deckDensity": {
    id: "flashcards.deckDensity",
    title: "Deck list density",
    summary: "Comfortable view emphasizes visual summaries; Compact view fits more decks on screen.",
    details:
      "Density changes presentation only. It does not alter deck order, schedules, cards, or review history, and it can be chosen independently on each device.",
    documentation: "Customizing the Flashcards library",
  },
  "flashcards.search.studyState": {
    id: "flashcards.search.studyState",
    title: "Study-state filters",
    summary: "Availability, status, and last-studied filters describe different parts of a card's schedule.",
    details:
      "Ready now follows the current local study calendar. Suspended and buried cards are excluded from ordinary study until resumed or unburied. Applied filters remain visible as removable chips.",
    documentation: "Searching, filtering, and bulk card management",
    tutorialModuleId: "search",
  },
  "flashcards.search.selection": {
    id: "flashcards.search.selection",
    title: "Selecting search results",
    summary: "Select visible cards or every card matching the current filters.",
    details:
      "Select visible affects only loaded rows. Select all matching resolves the complete filtered set before performing a bulk action.",
    documentation: "Searching, filtering, and bulk card management",
  },
  "templates.layout": {
    id: "templates.layout",
    title: "Template layouts",
    summary: "A template defines reusable fields and how they appear on generated card sides.",
    details:
      "Fields store study content. Rules place those fields on question and answer sides without executing scripts or arbitrary HTML.",
    documentation: "Card templates and reusable layouts",
  },
  "documents.provider": {
    id: "documents.provider",
    title: "Document AI profile",
    summary: "Documents use the profile selected for the Flashcard Generation task in Settings.",
    details:
      "Provider credentials remain in the native secret-storage boundary. The source is adopted into managed local storage before processing, and generated cards become an ordinary editable deck.",
    documentation: "Documents and AI-generated flashcards",
    tutorialModuleId: "documents",
  },
  "documents.processing": {
    id: "documents.processing",
    title: "Document processing",
    summary: "ostudy stores the source locally, extracts structure, and creates a normal deck.",
    details:
      "Large files can take time. You can stop generation, retry recoverable failures, and keep the source document for future generation passes.",
    documentation: "Documents and AI-generated flashcards",
    tutorialModuleId: "documents",
  },
  "practice.confidence": {
    id: "practice.confidence",
    title: "Confidence ratings",
    summary: "Use 0 for Wrong, 1 for Hard, 2 for Medium, and 3 for Easy.",
    details:
      "Choose the score that matches actual recall. The rating updates the individual generated card's schedule and appends an immutable review event.",
    documentation: "Practice, confidence, and scheduling",
    tutorialModuleId: "practice",
  },
  "practice.dailyLimits": {
    id: "practice.dailyLimits",
    title: "Daily limits",
    summary: "Daily limits manage workload without deleting or permanently hiding due cards.",
    details:
      "Scheduled study respects the limits. Extra practice remains available when you intentionally choose a deck or custom scope.",
    documentation: "Practice, confidence, and scheduling",
  },
  "settings.materialEffects": {
    id: "settings.materialEffects",
    title: "Material effects",
    summary: "Material effects control depth, translucency, blur, and texture independently from motion.",
    details:
      "Automatic uses restrained effects and follows reduced-transparency or forced-color preferences. Simple materials use opaque surfaces and no grain.",
    documentation: "Themes, motion, typography, and visual accessibility",
  },
  "settings.motion": {
    id: "settings.motion",
    title: "Motion preference",
    summary:
      "Motion can follow the operating system, use full spatial feedback, reduce motion, or turn nonessential motion off.",
    details:
      "Reduced motion removes card flips, parallax, spatial route movement, reorder interpolation, and particle celebrations while preserving clear state changes.",
    documentation: "Themes, motion, typography, and visual accessibility",
  },
};
