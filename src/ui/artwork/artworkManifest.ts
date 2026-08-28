export type ArtworkSlot =
  | "brand.logo.sidebar"
  | "brand.logo.compact"
  | "empty.deck"
  | "empty.folder"
  | "empty.search"
  | "empty.documents"
  | "empty.templates"
  | "practice.complete"
  | "tutorial.welcome"
  | "document.processing";

export interface ArtworkDefinition {
  slot: ArtworkSlot;
  aspectRatio: string;
  decorative: boolean;
  fallbackLabel: string;
  category: "brand" | "empty" | "completion" | "tutorial" | "status";
}

export const artworkManifest: Readonly<Record<ArtworkSlot, ArtworkDefinition>> = {
  "brand.logo.sidebar": { slot: "brand.logo.sidebar", aspectRatio: "5 / 2", decorative: false, fallbackLabel: "ostudy", category: "brand" },
  "brand.logo.compact": { slot: "brand.logo.compact", aspectRatio: "1 / 1", decorative: false, fallbackLabel: "ostudy", category: "brand" },
  "empty.deck": { slot: "empty.deck", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Empty flashcard deck", category: "empty" },
  "empty.folder": { slot: "empty.folder", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Empty folder", category: "empty" },
  "empty.search": { slot: "empty.search", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "No matching flashcards", category: "empty" },
  "empty.documents": { slot: "empty.documents", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "No documents", category: "empty" },
  "empty.templates": { slot: "empty.templates", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "No templates", category: "empty" },
  "practice.complete": { slot: "practice.complete", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Practice complete", category: "completion" },
  "tutorial.welcome": { slot: "tutorial.welcome", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Welcome to ostudy", category: "tutorial" },
  "document.processing": { slot: "document.processing", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Processing document", category: "status" },
};
