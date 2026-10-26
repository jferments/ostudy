import { describe, expect, it } from "vitest";

import { sectionWithoutAi } from "./appDestinations";

describe("sectionWithoutAi", () => {
  it("shows Home for the Tutor and a subject's Flashcards for its AI-made pages", () => {
    expect(sectionWithoutAi("tutor")).toBe("home");
    expect(sectionWithoutAi("lessons")).toBe("decks");
    expect(sectionWithoutAi("study-guides")).toBe("decks");
    expect(sectionWithoutAi("podcasts")).toBe("decks");
  });

  it("keeps every page that works without AI", () => {
    for (const section of ["home", "overview", "decks", "documents", "notes", "analytics", "settings"] as const) {
      expect(sectionWithoutAi(section)).toBe(section);
    }
  });
});
