import { describe, expect, it } from "vitest";

import {
  createFlashcardsRootRoute,
  normalizeFlashcardsWorkspace,
} from "./flashcardsRoutes";

describe("Flashcards route compatibility boundary", () => {
  it.each([
    ["templates", "decks"],
    ["transfer", "decks"],
    ["cards", "decks"],
    [null, "decks"],
    ["search", "search"],
  ] as const)("normalizes legacy or unknown workspace %s to the current %s route", (input, expected) => {
    expect(normalizeFlashcardsWorkspace(input)).toBe(expected);
  });

  it.each(["templates", "transfer"] as const)(
    "accepts legacy %s only at the route constructor and emits a current workspace",
    (workspace) => {
      const route = createFlashcardsRootRoute(workspace, 7);
      expect(route).toEqual({ requestId: 7, workspace: "decks", view: "root" });
    },
  );
});
