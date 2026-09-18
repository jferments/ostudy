import { describe, expect, it } from "vitest";
import { parseQuestionGenerationDraft, recommendedQuestionMix, toQuestionGenerationDraft } from "./questionGenerationDraft";

describe("editable question generation policy", () => {
  it("round-trips the recommended mix and does not include automatic occlusion", () => {
    const policy = recommendedQuestionMix();
    expect(parseQuestionGenerationDraft(toQuestionGenerationDraft(policy))).toEqual(policy);
    expect(policy.allowedTypes).toHaveLength(7);
    expect(policy.allowedTypes).not.toContain("image_occlusion");
  });

  it("preserves temporarily blank and invalid fields instead of silently rounding them", () => {
    for (const invalid of ["", "1", "17", "3.5", "NaN", "-4", "1e1"]) {
      const draft = toQuestionGenerationDraft(recommendedQuestionMix());
      draft.maxCompoundItems = invalid;
      expect(parseQuestionGenerationDraft(draft)).toBeNull();
      expect(draft.maxCompoundItems).toBe(invalid);
    }
  });

  it("allows soft zero weights but requires positive weight for an enabled type", () => {
    const draft = toQuestionGenerationDraft(recommendedQuestionMix());
    draft.allowedTypes = ["matching"];
    draft.weights.matching = "0";
    expect(parseQuestionGenerationDraft(draft)).toBeNull();
    draft.weights.matching = "1000";
    expect(parseQuestionGenerationDraft(draft)?.allowedTypes).toEqual(["matching"]);
    draft.weights.matching = "1001";
    expect(parseQuestionGenerationDraft(draft)).toBeNull();
    draft.allowedTypes = [];
    expect(parseQuestionGenerationDraft(draft)).toBeNull();
  });
});
