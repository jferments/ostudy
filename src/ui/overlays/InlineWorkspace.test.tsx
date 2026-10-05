import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Button } from "../primitives/Button";
import { InlineWorkspace } from "./Dialog";

describe("InlineWorkspace", () => {
  it("frames a card workspace with a heading and a close button", () => {
    const onClose = vi.fn();
    render(<InlineWorkspace title="Upload images" description="Add photos of your notes." closeLabel="Close upload" onClose={onClose}>
      <p>Body</p>
    </InlineWorkspace>);

    const workspace = screen.getByRole("region", { name: "Upload images" });
    expect(within(workspace).getByRole("heading", { level: 2, name: "Upload images" })).toBeTruthy();
    fireEvent.click(within(workspace).getByRole("button", { name: "Close upload" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("presents a page with a back button, the title over its description, and actions", async () => {
    const onClose = vi.fn();
    const onAction = vi.fn();
    render(<InlineWorkspace presentation="page" title="Renal physiology" description="Study guide"
      closeLabel="Back to Study Guides" onClose={onClose}
      actions={<Button variant="ghost" onClick={onAction}>Contents</Button>}>
      <h1>Overview</h1>
    </InlineWorkspace>);

    const page = screen.getByRole("region", { name: "Renal physiology" });
    expect(page.getAttribute("data-presentation")).toBe("page");
    expect(page.getAttribute("aria-modal")).toBeNull();
    // The page's own heading leads the outline; the bar title is not a second heading.
    expect(within(page).getAllByRole("heading").map((heading) => heading.textContent)).toEqual(["Overview"]);
    expect(document.getElementById(page.getAttribute("aria-describedby") ?? "")?.textContent).toBe("Study guide");
    const back = within(page).getByRole("button", { name: "Back to Study Guides" });
    await waitFor(() => expect(document.activeElement).toBe(back));

    fireEvent.click(within(page).getByRole("button", { name: "Contents" }));
    expect(onAction).toHaveBeenCalledOnce();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
    fireEvent.click(back);
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("keeps a busy page open", () => {
    const onClose = vi.fn();
    render(<InlineWorkspace presentation="page" title="Renal physiology" busy onClose={onClose}>
      <p>Saving</p>
    </InlineWorkspace>);

    const page = screen.getByRole("region", { name: "Renal physiology" });
    expect(page.getAttribute("aria-busy")).toBe("true");
    expect((within(page).getByRole("button", { name: "Close workspace" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });
});
