import { beforeEach, describe, expect, it, vi } from "vitest";

import * as practiceIpc from "../ipc/practice";
import { makePracticeSession } from "../test/factories";
import { PracticeController } from "./practiceController";

vi.mock("../ipc/practice", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ipc/practice")>();
  return {
    ...actual,
    startCardPractice: vi.fn(),
    startSelectedPractice: vi.fn(),
  };
});

describe("PracticeController start boundary", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("dispatches selected deck and tag scopes through the selected-session gateway", async () => {
    const session = makePracticeSession();
    vi.mocked(practiceIpc.startSelectedPractice).mockResolvedValue(session);
    const controller = new PracticeController();

    await expect(
      controller.start({
        kind: "selected",
        operationId: "1".repeat(32),
        deckIds: ["deck-1"],
        tagIds: ["tag-1"],
        shuffle: true,
        mode: "scheduled",
      }),
    ).resolves.toBe(session);

    expect(practiceIpc.startSelectedPractice).toHaveBeenCalledWith({
      operationId: "1".repeat(32),
      deckIds: ["deck-1"],
      tagIds: ["tag-1"],
      shuffle: true,
      mode: "scheduled",
    });
    expect(practiceIpc.startCardPractice).not.toHaveBeenCalled();
  });

  it("dispatches exact card IDs through the custom-session gateway", async () => {
    const session = makePracticeSession({ mode: "custom" });
    vi.mocked(practiceIpc.startCardPractice).mockResolvedValue(session);
    const controller = new PracticeController();

    await expect(
      controller.start({
        kind: "cards",
        operationId: "2".repeat(32),
        cardIds: ["card-1", "card-2"],
        shuffle: false,
      }),
    ).resolves.toBe(session);

    expect(practiceIpc.startCardPractice).toHaveBeenCalledWith({
      operationId: "2".repeat(32),
      cardIds: ["card-1", "card-2"],
      shuffle: false,
    });
    expect(practiceIpc.startSelectedPractice).not.toHaveBeenCalled();
  });
});
