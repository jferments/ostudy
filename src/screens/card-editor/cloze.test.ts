import { describe, expect, it } from "vitest";

import {
  clearClozeMarkers,
  nextClozeGroup,
  normalizeClozeMode,
  parseClozeMarkup,
  renderClozeQuestion,
} from "./cloze";

describe("fill-in-the-blank markup", () => {
  it("renders one blank group while leaving the other marked text visible", () => {
    const parsed = parseClozeMarkup(
      "{{c1::ACE inhibitors}} block formation of {{c2::angiotensin II}}.",
    );

    expect(parsed.valid).toBe(true);
    expect(parsed.groups).toEqual([1, 2]);
    expect(renderClozeQuestion(parsed, 1)).toBe(
      "[…] block formation of angiotensin II.",
    );
    expect(renderClozeQuestion(parsed, 2)).toBe(
      "ACE inhibitors block formation of […].",
    );
  });

  it("normalizes every marker into one group for all-together mode", () => {
    const value = "{{c4::First}} and {{c9::second}}";

    expect(normalizeClozeMode(value, "all_together")).toBe(
      "{{c1::First}} and {{c1::second}}",
    );
    expect(normalizeClozeMode(value, "one_at_a_time")).toBe(
      "{{c1::First}} and {{c2::second}}",
    );
  });

  it("rejects incomplete or nested markers and can return to plain text", () => {
    expect(parseClozeMarkup("Before {{c1::unfinished").valid).toBe(false);
    expect(parseClozeMarkup("{{c1::outer {{c2::inner}}}}").valid).toBe(false);
    expect(clearClozeMarkers("Know {{c1::this}} and {{c2::that}}.")).toBe(
      "Know this and that.",
    );
    expect(nextClozeGroup("{{c1::one}} {{c3::three}}", "one_at_a_time")).toBe(4);
    expect(nextClozeGroup("{{c1::one}}", "all_together")).toBe(1);
  });
});
