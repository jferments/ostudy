import raccoonHead128 from "./images/raccoon-head-128.webp";
import raccoonHead64 from "./images/raccoon-head-64.webp";
import raccoonStudy240 from "./images/raccoon-study-240.webp";
import raccoonStudy480 from "./images/raccoon-study-480.webp";

export type ArtworkSlot =
  | "brand.logo.sidebar"
  | "brand.logo.compact"
  | "empty.deck"
  | "empty.folder"
  | "empty.search"
  | "empty.documents"
  | "empty.templates"
  | "practice.complete"
  | "progress.coach"
  | "tutorial.welcome"
  | "document.processing";

/** Raster art for a slot: the base file for 1x screens and a srcSet adding the 2x file. */
export interface ArtworkImage {
  src: string;
  srcSet: string;
}

export interface ArtworkDefinition {
  slot: ArtworkSlot;
  aspectRatio: string;
  decorative: boolean;
  fallbackLabel: string;
  category: "brand" | "empty" | "completion" | "tutorial" | "status";
  image?: ArtworkImage;
}

export const artworkManifest: Readonly<Record<ArtworkSlot, ArtworkDefinition>> = {
  "brand.logo.sidebar": { slot: "brand.logo.sidebar", aspectRatio: "5 / 2", decorative: false, fallbackLabel: "ostudy", category: "brand" },
  "brand.logo.compact": {
    slot: "brand.logo.compact", aspectRatio: "1 / 1", decorative: false, fallbackLabel: "ostudy", category: "brand",
    // The raccoon's head and shoulders, cropped square from the project mascot (see ARTWORK-LICENSES.md).
    image: { src: raccoonHead64, srcSet: `${raccoonHead64} 1x, ${raccoonHead128} 2x` },
  },
  "empty.deck": { slot: "empty.deck", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Empty flashcard deck", category: "empty" },
  "empty.folder": { slot: "empty.folder", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Empty folder", category: "empty" },
  "empty.search": { slot: "empty.search", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "No matching flashcards", category: "empty" },
  "empty.documents": { slot: "empty.documents", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "No documents", category: "empty" },
  "empty.templates": { slot: "empty.templates", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "No templates", category: "empty" },
  "practice.complete": {
    slot: "practice.complete", aspectRatio: "1 / 1", decorative: true, fallbackLabel: "Practice complete", category: "completion",
    // The whole mascot at its desk, from the same source file as the brand mark.
    image: { src: raccoonStudy240, srcSet: `${raccoonStudy240} 1x, ${raccoonStudy480} 2x` },
  },
  "progress.coach": {
    slot: "progress.coach", aspectRatio: "1 / 1", decorative: true, fallbackLabel: "The ostudy raccoon", category: "status",
    // The brand mark's raccoon, here as decoration the mascot-art setting can hide.
    image: { src: raccoonHead64, srcSet: `${raccoonHead64} 1x, ${raccoonHead128} 2x` },
  },
  "tutorial.welcome": { slot: "tutorial.welcome", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Welcome to ostudy", category: "tutorial" },
  "document.processing": { slot: "document.processing", aspectRatio: "4 / 3", decorative: true, fallbackLabel: "Processing document", category: "status" },
};
