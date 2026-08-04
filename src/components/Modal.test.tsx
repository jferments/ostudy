import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Modal } from "./Modal";

function renderModal(onClose = vi.fn()) {
  render(
    <Modal title="Confirm action" description="Review the change." onClose={onClose}>
      <button type="button">Inside action</button>
    </Modal>,
  );
  return onClose;
}

describe("Modal", () => {
  it("exposes a labelled modal dialog", () => {
    renderModal();

    const dialog = screen.getByRole("dialog", { name: "Confirm action" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(screen.getByText("Review the change.")).toBeTruthy();
  });

  it("closes from Escape, the close button, and the backdrop", () => {
    const onClose = renderModal();

    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    fireEvent.mouseDown(document.querySelector(".modal-backdrop") as HTMLElement);

    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it("does not close when content inside the dialog is clicked", () => {
    const onClose = renderModal();

    fireEvent.mouseDown(screen.getByRole("button", { name: "Inside action" }));

    expect(onClose).not.toHaveBeenCalled();
  });
});
