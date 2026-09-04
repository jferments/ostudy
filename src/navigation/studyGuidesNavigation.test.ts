import { describe, expect, it } from "vitest";

import { shouldResetStudyGuidesRoute } from "./studyGuidesNavigation";

describe("shouldResetStudyGuidesRoute", () => {
  it("resets an already-active Study Guides route so a viewer returns to the library", () => {
    expect(shouldResetStudyGuidesRoute("study-guides", "study-guides", null)).toBe(true);
  });

  it("does not reset on initial navigation, unrelated destinations, or active workflows", () => {
    expect(shouldResetStudyGuidesRoute("study-guides", "decks", null)).toBe(false);
    expect(shouldResetStudyGuidesRoute("documents", "study-guides", null)).toBe(false);
    expect(shouldResetStudyGuidesRoute("study-guides", "study-guides", "practice")).toBe(false);
  });
});
