import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../layout/LayoutModeProvider";
import { Button } from "../primitives/Button";
import { TextField } from "../primitives/TextField";
import { resolveModalLayout } from "../vendor/shadcn/modal";
import { Dialog, Sheet } from "./Dialog";
import { OverlayProvider } from "./OverlayProvider";
import { useConfirm } from "./overlayContext";
import { layoutWidths, matchesLayoutQuery } from "../../test/viewport";

const originalMatchMedia = window.matchMedia;
afterEach(() => { window.matchMedia = originalMatchMedia; });

function useViewport(mode: "wide" | "compact") {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, layoutWidths[mode]),
    media: query,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn(),
  }));
}

function slots(dialog: HTMLElement) {
  return {
    header: dialog.querySelector<HTMLElement>("[data-slot='dialog-header']"),
    body: dialog.querySelector<HTMLElement>("[data-slot='dialog-body']"),
    actions: dialog.querySelector<HTMLElement>("[data-slot='dialog-actions']"),
  };
}

describe("shared dialog structure", () => {
  it("puts the title in the header, content in the scrolling body and actions in a pinned footer", () => {
    useViewport("wide");
    render(<LayoutModeProvider><OverlayProvider>
      <Dialog title="Move cards" description="3 cards are selected." onClose={vi.fn()}
        actions={<><Button>Cancel</Button><Button variant="primary">Move 3 cards</Button></>}>
        <p>Destination deck</p>
      </Dialog>
    </OverlayProvider></LayoutModeProvider>);
    const dialog = screen.getByRole("dialog", { name: "Move cards" });
    const { header, body, actions } = slots(dialog);
    expect(within(header!).getByRole("heading", { level: 2, name: "Move cards" })).toBeTruthy();
    expect(within(header!).getByRole("button", { name: "Close dialog" })).toBeTruthy();
    expect(within(body!).getByText("Destination deck")).toBeTruthy();
    expect(within(actions!).getByRole("button", { name: "Move 3 cards" })).toBeTruthy();
    // The footer is a sibling of the scrolling body, so it stays visible while the body scrolls.
    expect(body!.contains(actions)).toBe(false);
    expect(body!.compareDocumentPosition(actions!) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  });

  it("submits body and actions as one form without navigating", () => {
    useViewport("wide");
    const submit = vi.fn();
    render(<LayoutModeProvider><OverlayProvider>
      <Dialog title="Rename folder" onClose={vi.fn()} onSubmit={submit}
        actions={<Button type="submit" variant="primary">Save name</Button>}>
        <TextField aria-label="Folder name" defaultValue="Physiology" />
      </Dialog>
    </OverlayProvider></LayoutModeProvider>);
    const dialog = screen.getByRole("dialog", { name: "Rename folder" });
    const form = dialog.querySelector("form")!;
    expect(form.contains(within(dialog).getByRole("textbox", { name: "Folder name" }))).toBe(true);
    expect(form.contains(within(dialog).getByRole("button", { name: "Save name" }))).toBe(true);
    fireEvent.click(within(dialog).getByRole("button", { name: "Save name" }));
    expect(submit).toHaveBeenCalledOnce();
    expect((submit.mock.calls[0]?.[0] as Event | undefined)?.defaultPrevented).toBe(true);
  });

  it("omits the footer when a dialog has no actions", () => {
    useViewport("wide");
    render(<LayoutModeProvider><OverlayProvider>
      <Dialog title="Source" onClose={vi.fn()}><p>Excerpt</p></Dialog>
    </OverlayProvider></LayoutModeProvider>);
    expect(slots(screen.getByRole("dialog", { name: "Source" })).actions).toBeNull();
  });

  it("fills a compact screen with a dialog and raises sheets and confirmations from the bottom edge", async () => {
    useViewport("compact");
    const dialogView = render(<LayoutModeProvider><OverlayProvider>
      <Dialog title="Edit formula" onClose={vi.fn()}><p>Formula</p></Dialog>
    </OverlayProvider></LayoutModeProvider>);
    expect(screen.getByRole("dialog", { name: "Edit formula" }).getAttribute("data-presentation")).toBe("fullscreen");
    dialogView.unmount();

    const sheetView = render(<LayoutModeProvider><OverlayProvider>
      <Sheet title="Rename folder" onClose={vi.fn()}><p>Name</p></Sheet>
    </OverlayProvider></LayoutModeProvider>);
    expect(screen.getByRole("dialog", { name: "Rename folder" }).getAttribute("data-presentation")).toBe("sheet");
    sheetView.unmount();

    function Confirming() {
      const confirm = useConfirm();
      return <Button onClick={() => void confirm({ title: "Delete deck?", description: "Its cards move to the trash.", destructive: true })}>Delete</Button>;
    }
    render(<LayoutModeProvider><OverlayProvider><Confirming /></OverlayProvider></LayoutModeProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    const alert = await screen.findByRole("alertdialog", { name: "Delete deck?" });
    expect(alert.getAttribute("data-presentation")).toBe("sheet");
    expect(within(slots(alert).actions!).getByRole("button", { name: "Continue" })).toBeTruthy();
  });

  it("centers every presentation on wider screens unless full screen is requested", () => {
    expect(resolveModalLayout("dialog", "dialog", false)).toBe("dialog");
    expect(resolveModalLayout("sheet", "dialog", false)).toBe("dialog");
    expect(resolveModalLayout("dialog", "alertdialog", false)).toBe("dialog");
    expect(resolveModalLayout("fullscreen", "dialog", false)).toBe("fullscreen");
    expect(resolveModalLayout("dialog", "dialog", true)).toBe("fullscreen");
    expect(resolveModalLayout("sheet", "dialog", true)).toBe("sheet");
    expect(resolveModalLayout("dialog", "alertdialog", true)).toBe("sheet");
  });
});
