import { beforeEach, describe, expect, it, vi } from "vitest";

import { previewCardDraft } from "./cards";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("./transport", () => ({ invoke }));

beforeEach(() => invoke.mockReset());

describe("card author preview gateway", () => {
  it("uses the pure native preview command for unsaved recall drafts", async () => {
    const input = {
      cardType: "cloze" as const,
      clozeSource: "The {{c1::heart}} pumps.",
      clozeMode: "one_at_a_time" as const,
    };
    const result = [{
      presentationKey: "cloze:1", presentationLabel: "Blank group 1", cardLabel: "Fill in the blank",
      frontDoc: { type: "doc", content: [] }, backDoc: { type: "doc", content: [] },
      frontText: "The […] pumps.", backText: "heart",
    }];
    invoke.mockResolvedValue(result);
    await expect(previewCardDraft(input)).resolves.toBe(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("preview_card_draft", { input });
  });
});
