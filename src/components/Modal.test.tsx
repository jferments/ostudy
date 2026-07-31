import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { Modal } from "./Modal";

describe("Modal accessibility", () => {
  beforeEach(() => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 1;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
  });

  it("labels the dialog, closes with Escape, and traps Tab focus", () => {
    const onClose = vi.fn();
    render(
      <Modal title="Move or copy" description="Choose a destination deck." onClose={onClose}>
        <button type="button">First action</button>
        <button type="button">Last action</button>
      </Modal>,
    );

    const dialog = screen.getByRole("dialog", { name: "Move or copy" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.getAttribute("aria-describedby")).not.toBeNull();

    const close = screen.getByRole("button", { name: "Close dialog" });
    const last = screen.getByRole("button", { name: "Last action" });
    close.focus();
    fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(last);

    last.focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(document.activeElement).toBe(close);

    last.focus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("restores focus to the control that opened it", () => {
    const opener = document.createElement("button");
    opener.textContent = "Open";
    document.body.append(opener);
    opener.focus();

    const { unmount } = render(
      <Modal title="Editor" onClose={() => undefined}>
        <input aria-label="Card field" />
      </Modal>,
    );
    expect(document.activeElement).not.toBe(opener);

    unmount();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});
