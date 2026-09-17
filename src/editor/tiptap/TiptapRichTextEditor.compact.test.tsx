import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TiptapRichTextEditor } from "./TiptapRichTextEditor";

const empty = { type: "doc", content: [{ type: "paragraph" }] };

async function openEditor(disabled = false) {
  const changed = vi.fn();
  render(<TiptapRichTextEditor label="Question" document={empty} assets={[]} assetDraftId={"a".repeat(32)}
    minHeight={180} disabled={disabled} onChange={changed} />);
  const textbox = await screen.findByRole("textbox", { name: "Question" });
  return { textbox, changed };
}

describe("compact rich editor and full-surface focus", () => {
  it("keeps image/link/formula/media one click away while advanced formatting starts closed", async () => {
    await openEditor();
    for (const label of ["Insert image", "Add or edit link", "Insert or edit formula", "Insert audio or video"]) {
      const button = screen.getByRole("button", { name: label });
      expect(button.querySelector("svg")).not.toBeNull();
      expect(button.getAttribute("title")).toBe(label);
    }
    expect(screen.queryByRole("combobox", { name: "Font family" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Bold" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Advanced editor" }));
    expect(screen.getByRole("combobox", { name: "Font family" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Bold" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Bullet list" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Lists" }));
    const menu = await screen.findByRole("menu");
    for (const label of ["Bullet list", "Numbered list", "Checkbox list"]) {
      expect(within(menu).getByRole("menuitem", { name: label })).toBeTruthy();
    }
  });

  it("the editable element fills the text surface and its blank padding focuses the editor without a content write", async () => {
    const { textbox, changed } = await openEditor();
    expect(textbox.style.minHeight).toBe("148px");
    expect(textbox.style.outline).toBe("none");
    const surface = textbox.parentElement?.parentElement;
    expect(surface?.style.minHeight).toBe("180px");
    fireEvent.click(surface!);
    await waitFor(() => expect(document.activeElement).toBe(textbox));
    expect(changed).not.toHaveBeenCalled();
  });

  it("switching toolbar modes preserves the same editable node and document", async () => {
    const { textbox, changed } = await openEditor();
    const toggle = screen.getByRole("button", { name: "Advanced editor" });
    fireEvent.click(toggle);
    fireEvent.click(toggle);
    expect(screen.getByRole("textbox", { name: "Question" })).toBe(textbox);
    expect(changed).not.toHaveBeenCalled();
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
  });

  it("disabled input does not focus from the padding or enable quick insert", async () => {
    const { textbox, changed } = await openEditor(true);
    fireEvent.click(textbox.parentElement!.parentElement!);
    expect(document.activeElement).not.toBe(textbox);
    expect(screen.getByRole("button", { name: "Advanced editor" })).toHaveProperty("disabled", true);
    expect(changed).not.toHaveBeenCalled();
  });
});
