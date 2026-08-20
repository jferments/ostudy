import { describe, expect, it } from "vitest";

import {
  flashcardsWorkspaceNames,
  normalizeFlashcardsWorkspaceName,
} from "./flashcardsRoutes";

describe("Flashcards workspace routes", () => {
  it("exposes only the four canonical workspaces", () => {
    expect(flashcardsWorkspaceNames).toEqual(["decks", "templates", "search", "transfer"]);
  });

  it("repairs the removed create workspace to the deck-library root", () => {
    expect(normalizeFlashcardsWorkspaceName("create")).toBe("decks");
    expect(normalizeFlashcardsWorkspaceName("unknown")).toBe("decks");
  });
});
