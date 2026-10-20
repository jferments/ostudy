import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getQuestionGenerationPolicy, saveQuestionGenerationPolicy } from "../../ipc/questionGeneration";
import { BuiltInTemplates } from "./BuiltInTemplates";
import { recommendedQuestionMix } from "./questionGenerationDraft";

import { requiredTestValue as required } from "../../test/factories";
vi.mock("../../ipc/questionGeneration", () => ({
  getQuestionGenerationPolicy: vi.fn(),
  saveQuestionGenerationPolicy: vi.fn(),
}));
const load = vi.mocked(getQuestionGenerationPolicy);
const save = vi.mocked(saveQuestionGenerationPolicy);

beforeEach(() => {
  load.mockReset();
  save.mockReset();
  load.mockResolvedValue({ revision: 4, policy: recommendedQuestionMix() });
  save.mockImplementation(async (input) => ({ ...input, revision: input.revision + 1 }));
});
afterEach(cleanup);

async function mount() {
  render(<BuiltInTemplates />);
  return screen.findByRole("switch", { name: "AI writes Matching" });
}

describe("built-in templates", () => {
  it("lists every built-in format, with image occlusion made by hand only", async () => {
    await mount();
    expect(screen.getAllByRole("switch")).toHaveLength(7);
    expect(screen.getByText("Image occlusion")).toBeTruthy();
    expect(screen.queryByRole("switch", { name: /Image occlusion/u })).toBeNull();
  });

  it("saves a switched format at once with the loaded revision", async () => {
    fireEvent.click(await mount());
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const input = required(save.mock.calls[0]?.[0], "save input");
    expect(input.revision).toBe(4);
    expect(input.policy.allowedTypes).not.toContain("matching");
    await waitFor(() => expect(screen.getByRole("switch", { name: "AI writes Matching" }).getAttribute("aria-checked")).toBe("false"));
  });

  it("keeps the saved state and shows the error when saving fails", async () => {
    save.mockRejectedValue(new Error("Changed in another window."));
    fireEvent.click(await mount());
    await screen.findByText("Changed in another window.");
    expect(screen.getByRole("switch", { name: "AI writes Matching" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Reload" }));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  });

  it("edits the mix in a sheet and saves only on Done", async () => {
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Question mix" }));
    const sheet = await screen.findByRole("dialog", { name: "Question mix" });
    fireEvent.click(within(sheet).getByRole("button", { name: "Recall only" }));
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(within(sheet).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(required(save.mock.calls[0]?.[0], "save input").policy.allowedTypes).toEqual(["recall"]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Question mix" })).toBeNull());
  });

  it("does not save an out-of-range part limit", async () => {
    await mount();
    fireEvent.click(screen.getByRole("button", { name: "Question mix" }));
    const sheet = await screen.findByRole("dialog", { name: "Question mix" });
    fireEvent.change(within(sheet).getByRole("spinbutton", { name: "Most parts in one question" }), { target: { value: "17" } });
    expect((within(sheet).getByRole("button", { name: "Done" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("offers a retry after a failed load without saving defaults", async () => {
    load.mockRejectedValueOnce(new Error("Library is busy."));
    render(<BuiltInTemplates />);
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await screen.findByRole("switch", { name: "AI writes Matching" });
    expect(save).not.toHaveBeenCalled();
  });
});
