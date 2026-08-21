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
    version: 2,
    title: "Welcome to ostudy",
    steps: [
      {
        id: "flashcards-workspaces",
        title: "Everything for flashcards lives together",
        body: "Decks, templates, Search, and import or export are separate workspaces inside Flashcards. Open a deck to add one card or bulk add many cards to that deck.",
        anchor: "flashcards.workspaces",
        placement: "bottom",
      },
      {
        id: "navigation",
        title: "Move between study tools",
        body: "The main navigation stays consistent across desktop, tablet, and mobile. Android Back closes overlays and nested work before leaving the current area.",
        anchor: "app.navigation",
        placement: "end",
      },
    ],
  },
  search: {
    id: "search",
    version: 1,
    title: "Find and manage cards",
    steps: [
      {
        id: "filters",
        title: "Keep the ordinary view simple",
        body: "Search, deck scope, and sort stay visible. Open All filters for study state, folders, tags, and scheduling status; applied filters return as removable chips.",
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
