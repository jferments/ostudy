import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Dialog } from "./Dialog";

function Harness({ onClose }: { onClose: () => void }) {
  const [open, setOpen] = useState(true);
  return open ? <Dialog title="Confirm action" description="Review the change." onClose={() => { onClose(); setOpen(false); }}><button type="button">Inside action</button></Dialog> : null;
}

// These are the former compatibility-Modal assertions, retained at the actual
// dialog owner after the last feature caller migrated to it directly.
describe("Dialog feature dismissal", () => {
  it("exposes a labelled modal dialog", () => {
    render(<Harness onClose={vi.fn()} />);
    expect(screen.getByRole("dialog", { name: "Confirm action" }).getAttribute("aria-modal")).toBe("true");
    expect(screen.getByText("Review the change.")).toBeTruthy();
  });
  it.each(["Escape", "close", "backdrop"])("closes once from %s", async (action) => {
    const onClose = vi.fn(); render(<Harness onClose={onClose} />);
    const dialog = await screen.findByRole("dialog", { name: "Confirm action" });
    if (action === "Escape") fireEvent.keyDown(dialog, { key: "Escape" });
    else if (action === "close") fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    else { const backdrop = document.querySelector('[data-slot="dialog-overlay"]') as HTMLElement; fireEvent.mouseDown(backdrop); fireEvent.click(backdrop); }
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it("does not close when dialog content is clicked", () => {
    const onClose = vi.fn(); render(<Harness onClose={onClose} />);
    fireEvent.mouseDown(screen.getByRole("button", { name: "Inside action" })); expect(onClose).not.toHaveBeenCalled();
  });
});
