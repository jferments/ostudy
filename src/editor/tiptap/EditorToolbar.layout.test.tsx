import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TiptapRichTextEditor } from "./TiptapRichTextEditor";

async function openAdvanced() {
  render(<TiptapRichTextEditor label="Question" document={{ type: "doc", content: [{ type: "paragraph" }] }}
    assets={[]} onChange={vi.fn()} />);
  await screen.findByRole("textbox", { name: "Question" });
  fireEvent.click(screen.getByRole("button", { name: "Advanced editor" }));
}

describe("compact single-level advanced editor", () => {
  it("constrains the whole native picker, including its arrow, in a wrapping row", async () => {
    await openAdvanced();
    for (const [label, width] of [["Paragraph style", "tw:w-36"], ["Font family", "tw:w-40"], ["Text size", "tw:w-20"]]) {
      const select = screen.getByRole("combobox", { name: label });
      const nativeWrapper = select.closest('[data-slot="native-select-wrapper"]')!;
      const widthOwner = nativeWrapper.parentElement!;
      expect(widthOwner.getAttribute("data-compact-picker")).toBe("true");
      expect(widthOwner.className).toContain(width);
      expect(nativeWrapper.querySelector('[data-slot="native-select-icon"]')).not.toBeNull();
      expect(widthOwner.parentElement?.className).toContain("tw:flex-wrap");
    }
  });

  it("exposes all additional formatting without a second submenu", async () => {
    await openAdvanced();
    expect(screen.queryByRole("button", { name: "Advanced formatting" })).toBeNull();
    for (const label of ["Undo", "Redo", "Strikethrough", "Inline code", "Block quote", "Code block", "Clear formatting"]) {
      expect(screen.getByRole("button", { name: label }).closest('[role="toolbar"]')).not.toBeNull();
    }
  });
});
