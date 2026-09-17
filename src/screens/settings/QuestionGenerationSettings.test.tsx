import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getQuestionGenerationPolicy, saveQuestionGenerationPolicy } from "../../ipc/questionGeneration";
import { QuestionGenerationSettings } from "./QuestionGenerationSettings";
import { recommendedQuestionMix } from "./questionGenerationDraft";

vi.mock("../../ipc/questionGeneration", () => ({
  getQuestionGenerationPolicy: vi.fn(),
  saveQuestionGenerationPolicy: vi.fn(),
}));
const load = vi.mocked(getQuestionGenerationPolicy);
const save = vi.mocked(saveQuestionGenerationPolicy);
const showToast = vi.fn();
const dirty = vi.fn();

beforeEach(() => {
  load.mockReset();
  save.mockReset();
  showToast.mockReset();
  dirty.mockReset();
  load.mockResolvedValue({ revision: 4, policy: recommendedQuestionMix() });
});
afterEach(cleanup);

async function mount() {
  render(<QuestionGenerationSettings showToast={showToast} onDirtyChange={dirty} />);
  await screen.findByRole("checkbox", { name: "Matching" });
}

describe("question generation controls", () => {
  it("saves recall-only preferences with optimistic revision and no generation side effect", async () => {
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Recall only" }));
    expect(dirty).toHaveBeenLastCalledWith(true);
    save.mockImplementation(async (input) => ({ ...input, revision: input.revision + 1 }));
    fireEvent.click(screen.getByRole("button", { name: "Save question preferences" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "Question generation preferences saved."));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0]).toMatchObject({ revision: 4, policy: { allowedTypes: ["recall"] } });
    expect(dirty).toHaveBeenLastCalledWith(false);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("retains a failed or conflicted draft until the user explicitly reloads it", async () => {
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Recall only" }));
    save.mockRejectedValue(new Error("Changed in another window."));
    fireEvent.click(screen.getByRole("button", { name: "Save question preferences" }));
    await screen.findByText("Changed in another window.");
    expect(screen.getByRole("checkbox", { name: "Matching" }).getAttribute("aria-checked")).toBe("false");
    expect(dirty).toHaveBeenLastCalledWith(true);
    expect(load).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Reload saved question preferences" }));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(dirty).toHaveBeenLastCalledWith(false));
  });

  it("does not save an invalid compound limit and preserves the raw input", async () => {
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Customize question mix" }));
    const maximum = screen.getByRole("spinbutton", { name: "Maximum components per compound question" });
    fireEvent.change(maximum, { target: { value: "17" } });
    expect((maximum as HTMLInputElement).value).toBe("17");
    expect((screen.getByRole("button", { name: "Save question preferences" }) as HTMLButtonElement).disabled).toBe(true);
    expect(save).not.toHaveBeenCalled();
    fireEvent.change(maximum, { target: { value: "4" } });
    expect((screen.getByRole("button", { name: "Save question preferences" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("allows retry after a failed load without publishing defaults automatically", async () => {
    load.mockRejectedValueOnce(new Error("Library is busy."));
    render(<QuestionGenerationSettings showToast={showToast} onDirtyChange={dirty} />);
    fireEvent.click(await screen.findByRole("button", { name: "Retry loading preferences" }));
    await screen.findByRole("checkbox", { name: "Matching" });
    expect(load).toHaveBeenCalledTimes(2);
    expect(save).not.toHaveBeenCalled();
  });
});
