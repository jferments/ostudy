import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { mountFormulaInput } from "../math/mathlive";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { FormulaDialog } from "./FormulaDialog";
import type { FormulaEditSession } from "./formula";

vi.mock("../math/mathlive", () => ({ mountFormulaInput: vi.fn() }));

afterEach(() => vi.resetAllMocks());

function session(): FormulaEditSession {
  return {
    initialValue: { latex: "x", placement: "inline" },
    save: vi.fn(() => ({ ok: true as const })),
    cancel: vi.fn(),
  };
}

it("keeps mount failures and responsive actions inside the padded dialog body", async () => {
  vi.mocked(mountFormulaInput).mockRejectedValue(new Error("Formula editor unavailable"));
  const editSession = session();
  const onClose = vi.fn();
  render(<OverlayProvider><FormulaDialog session={editSession} onClose={onClose} /></OverlayProvider>);
  const alert = await screen.findByRole("alert");
  const save = screen.getByRole("button", { name: "Save formula" }) as HTMLButtonElement;
  const actions = save.parentElement!;
  const body = actions.parentElement!;
  expect(body.classList.contains("tw:p-4")).toBe(true);
  expect(body.classList.contains("tw:box-border")).toBe(true);
  expect(body.classList.contains("tw:min-w-0")).toBe(true);
  expect(body.contains(alert)).toBe(true);
  expect(actions.classList.contains("tw:max-[760px]:flex-col-reverse")).toBe(true);
  expect(actions.classList.contains("tw:max-[760px]:[&>button]:w-full")).toBe(true);
  expect(save.disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  expect(editSession.cancel).toHaveBeenCalledOnce();
  expect(editSession.save).not.toHaveBeenCalled();
});

it("puts placement above the graphical tools and the full keyboard below them", async () => {
  const toggleKeyboard = vi.fn();
  vi.mocked(mountFormulaInput).mockResolvedValue({
    focus: vi.fn(),
    toggleKeyboard,
    hideKeyboard: vi.fn(() => false),
    applyTemplate: vi.fn(() => true),
    dispose: vi.fn(),
  });
  render(<OverlayProvider><FormulaDialog session={session()} onClose={vi.fn()} /></OverlayProvider>);
  const keyboard = await screen.findByRole("button", { name: "Full math keyboard" });
  await waitFor(() => expect((keyboard as HTMLButtonElement).disabled).toBe(false));
  const placement = screen.getByLabelText("Formula placement") as HTMLSelectElement;
  expect(Array.from(placement.options, (option) => option.text)).toEqual([
    "Inline with text",
    "On its own line",
  ]);
  const toolbox = screen.getByRole("region", { name: "Visual formula controls" });
  expect(placement.compareDocumentPosition(toolbox) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  expect(toolbox.compareDocumentPosition(keyboard) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  fireEvent.click(keyboard);
  expect(toggleKeyboard).toHaveBeenCalledOnce();
});
