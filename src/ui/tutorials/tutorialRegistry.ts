export type TutorialModuleId = "orientation" | "search" | "practice" | "documents";

export type TutorialPlacement = "auto" | "top" | "bottom" | "start" | "end";

export interface TutorialStep {
  id: string;
  title: string;
  body: string;
  anchor: string;
  placement?: TutorialPlacement;
  helpTopicId?: string;
}

export interface TutorialModule {
  id: TutorialModuleId;
  version: number;
  title: string;
  steps: readonly TutorialStep[];
}

export const tutorialModules: Readonly<Record<TutorialModuleId, TutorialModule>> = {
  orientation: {
    id: "orientation",
    version: 4,
    title: "Welcome to ostudy",
    steps: [
      {
        id: "library-workbench",
        title: "Everything for flashcards lives together",
        body: "Library lists your subjects with their decks and documents. Choose a subject, deck, view or tag to see its cards; open a card to edit it beside the list. Each subject and deck has a menu for renaming, moving, import and export.",
        anchor: "library.workbench",
        placement: "bottom",
      },
      {
        id: "navigation",
        title: "Move between study tools",
        body: "Home, Library, Review, Tutor and Progress are the same on desktop, tablet and phone, with Search and Settings in the header. Android Back closes overlays and nested work first, then returns to Home.",
        anchor: "app.navigation",
        placement: "auto",
      },
    ],
  },
  search: {
    id: "search",
    version: 2,
    title: "Find and manage cards",
    steps: [
      {
        id: "filters",
        title: "Search inside what you chose",
        body: "Search and sort work inside the subject, deck, view or tag chosen on the left, and the filters below them narrow it to due, new or suspended cards. Select cards to move, tag, suspend or reschedule them together.",
        anchor: "search.filters",
        placement: "bottom",
        helpTopicId: "flashcards.search.studyState",
      },
    ],
  },
  practice: {
    id: "practice",
    version: 1,
    title: "Practice a deck",
    steps: [
      {
        id: "card",
        title: "Recall before revealing",
        body: "Read the question and answer from memory before revealing. The backing layers show the remaining stack without adding extra card content to the accessibility tree.",
        anchor: "practice.card",
        placement: "top",
      },
      {
        id: "confidence",
        title: "Rate the difficulty",
        body: "After revealing, choose 0 through 3. Each generated card keeps its own schedule and review history, even when it came from a reversible note or template.",
        anchor: "practice.confidence",
        placement: "top",
        helpTopicId: "practice.confidence",
      },
    ],
  },
  documents: {
    id: "documents",
    version: 1,
    title: "Create cards from documents",
    steps: [
      {
        id: "upload",
        title: "Choose a local study source",
        body: "Upload a supported document through the system picker. ostudy keeps the managed source local and uses your configured AI profile to generate a normal deck.",
        anchor: "documents.upload",
        placement: "bottom",
        helpTopicId: "documents.provider",
      },
    ],
  },
};
