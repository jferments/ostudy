import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { mountFormulaInput } from "../math/mathlive";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { FormulaDialog } from "./FormulaDialog";
import type { FormulaEditSession } from "./formula";

vi.mock("../math/mathlive", () => ({ mountFormulaInput: vi.fn() }));

it("keeps mount failures and responsive actions inside the padded dialog body", async () => {
  vi.mocked(mountFormulaInput).mockRejectedValue(new Error("Formula editor unavailable"));
  const session: FormulaEditSession = {
    initialValue: { latex: "x", placement: "inline" },
    save: vi.fn(() => ({ ok: true as const })), cancel: vi.fn(),
  };
  const onClose = vi.fn();
  render(<OverlayProvider><FormulaDialog session={session} onClose={onClose} /></OverlayProvider>);
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
  expect(session.cancel).toHaveBeenCalledOnce();
  expect(session.save).not.toHaveBeenCalled();
});
