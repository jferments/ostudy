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
        body: "Library lists your subjects with their decks and documents. Choose All cards, a subject, a deck or a tag to see its cards, and open a card to edit it beside the list. Each subject and deck has a menu for actions such as renaming, moving and exporting.",
        anchor: "library.workbench",
        placement: "bottom",
      },
      {
        id: "navigation",
        title: "Move between study tools",
        body: "Home, Library, Review, Tutor and Progress are the same on every screen: in the side rail on wider screens and the bottom bar on a phone, with Search and Settings beside them. Android Back closes overlays and nested work first, then returns to Home.",
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
        body: "Search, Show and Sort work inside what you chose on the left: All cards, a subject, a deck or a tag. Show narrows the list to cards due today, new, often missed or suspended. Select cards to move, tag, suspend or reschedule them together.",
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
        body: "Read the question and recall the answer before you choose Show answer or press Space or Enter. The cards stacked behind it are decoration, not extra questions.",
        anchor: "practice.card",
        placement: "top",
      },
      {
        id: "confidence",
        title: "Rate the difficulty",
        body: "Choose Wrong, Hard, Medium or Easy, or press 0 to 3. Each card keeps its own schedule and review history, even the reverse card of the same note.",
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
        body: "Choose Upload, then Document… or Images…. ostudy keeps the source in your library, reads it on this device, and uses the AI model chosen in Settings to make an ordinary deck.",
        anchor: "documents.upload",
        placement: "bottom",
        helpTopicId: "documents.provider",
      },
    ],
  },
};
