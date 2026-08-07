import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { dispatchPlatformBack } from "../../platform/backNavigation";
import { Dialog } from "./Dialog";
import { OverlayProvider } from "./OverlayProvider";
import { useConfirm } from "./overlayContext";

function FocusHarness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open dialog
      </button>
      {open ? (
        <Dialog title="Focus dialog" description="Keyboard focus stays inside." onClose={() => setOpen(false)}>
          <div>
            <button type="button" data-autofocus="true">
              First action
            </button>
            <button type="button">Last action</button>
          </div>
        </Dialog>
      ) : null}
    </>
  );
}

function NestedHarness() {
  const [outer, setOuter] = useState(true);
  const [inner, setInner] = useState(true);
  return (
    <>
      {outer ? (
        <Dialog title="Outer dialog" onClose={() => setOuter(false)}>
          <button type="button">Outer action</button>
          {inner ? (
            <Dialog title="Inner dialog" onClose={() => setInner(false)}>
              <button type="button">Inner action</button>
            </Dialog>
          ) : null}
        </Dialog>
      ) : null}
    </>
  );
}

function ConfirmHarness() {
  const confirm = useConfirm();
  const [result, setResult] = useState("none");
  return (
    <>
      <button
        type="button"
        onClick={() => {
          void confirm({
            title: "Delete the test item?",
            description: "This action is destructive.",
            confirmLabel: "Delete",
            destructive: true,
          }).then((accepted) => setResult(String(accepted)));
        }}
      >
        Ask
      </button>
      <output aria-label="Confirmation result">{result}</output>
    </>
  );
}

describe("OverlayProvider", () => {
  it("moves focus inside, contains Tab, and restores focus to the opener", async () => {
    render(
      <OverlayProvider>
        <FocusHarness />
      </OverlayProvider>,
    );

    const opener = screen.getByRole("button", { name: "Open dialog" });
    opener.focus();
    fireEvent.click(opener);

    const first = await screen.findByRole("button", { name: "First action" });
    const last = screen.getByRole("button", { name: "Last action" });
    await waitFor(() => expect(document.activeElement).toBe(first));

    last.focus();
    fireEvent.keyDown(screen.getByRole("dialog", { name: "Focus dialog" }), { key: "Tab" });
    expect(document.activeElement).toBe(first);

    first.focus();
    fireEvent.keyDown(screen.getByRole("dialog", { name: "Focus dialog" }), {
      key: "Tab",
      shiftKey: true,
    });
    expect(document.activeElement).toBe(last);

    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    await waitFor(() => expect(document.activeElement).toBe(opener));
  });

  it("assigns unique accessible label ids to nested dialogs", async () => {
    render(
      <OverlayProvider>
        <NestedHarness />
      </OverlayProvider>,
    );

    const outer = await screen.findByRole("dialog", { name: "Outer dialog" });
    const inner = screen.getByRole("dialog", { name: "Inner dialog" });
    expect(outer.getAttribute("aria-labelledby")).not.toBe(inner.getAttribute("aria-labelledby"));
  });

  it("lets only the top overlay consume Escape and platform Back", async () => {
    render(
      <OverlayProvider>
        <NestedHarness />
      </OverlayProvider>,
    );

    const outer = await screen.findByRole("dialog", { name: "Outer dialog" });
    const inner = screen.getByRole("dialog", { name: "Inner dialog" });
    fireEvent.keyDown(outer, { key: "Escape" });
    expect(screen.getByRole("dialog", { name: "Outer dialog" })).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "Inner dialog" })).toBeTruthy();

    fireEvent.keyDown(inner, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Inner dialog" })).toBeNull());
    expect(screen.getByRole("dialog", { name: "Outer dialog" })).toBeTruthy();

    expect(dispatchPlatformBack()).toBe(true);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Outer dialog" })).toBeNull());
  });

  it("makes the application root inert while a modal is active", async () => {
    const root = document.createElement("div");
    root.id = "root";
    document.body.append(root);
    const view = render(
      <OverlayProvider>
        <FocusHarness />
      </OverlayProvider>,
      { container: root },
    );

    fireEvent.click(screen.getByRole("button", { name: "Open dialog" }));
    await screen.findByRole("dialog", { name: "Focus dialog" });
    await waitFor(() => expect(root.inert).toBe(true));
    expect(root.getAttribute("aria-hidden")).toBe("true");

    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
    await waitFor(() => expect(root.inert).toBe(false));
    expect(root.hasAttribute("aria-hidden")).toBe(false);
    view.unmount();
    root.remove();
  });

  it("provides an asynchronous destructive confirmation service", async () => {
    render(
      <OverlayProvider>
        <ConfirmHarness />
      </OverlayProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Ask" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Delete the test item?" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Confirmation result").textContent).toBe("true"),
    );
  });

  it("does not dismiss a busy dialog", async () => {
    const onClose = vi.fn();
    render(
      <OverlayProvider>
        <Dialog title="Busy dialog" busy onClose={onClose}>
          <button type="button">Working</button>
        </Dialog>
      </OverlayProvider>,
    );

    const dialog = await screen.findByRole("dialog", { name: "Busy dialog" });
    fireEvent.keyDown(dialog, { key: "Escape" });
    fireEvent.mouseDown(dialog);
    expect(dispatchPlatformBack()).toBe(true);
    expect(onClose).not.toHaveBeenCalled();
  });
});
