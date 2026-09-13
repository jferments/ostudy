import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { TagInput } from "./TagInput";

function Harness() {
  const [tags, setTags] = useState<string[]>([]);
  return <><TagInput tags={tags} suggestions={[]} onChange={setTags} /><output aria-label="Tag values">{tags.join("|")}</output></>;
}

describe("migrated tag input", () => {
  it("does not turn IME composition confirmation into an added tag", () => {
    render(<Harness />);
    const input = screen.getByRole("combobox", { name: "Tags" });
    fireEvent.change(input, { target: { value: "心臓" } });
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(screen.getByLabelText("Tag values").textContent).toBe("");
    fireEvent.keyDown(input, { key: "Enter", isComposing: false });
    expect(screen.getByLabelText("Tag values").textContent).toBe("心臓");
  });

  it("associates help with the input without nesting tag actions inside its label", () => {
    const onChange = vi.fn();
    render(<TagInput tags={["Anatomy"]} suggestions={[]} disabled onChange={onChange} />);
    const input = screen.getByRole("combobox", { name: "Tags" });
    const remove = screen.getByRole("button", { name: "Remove Anatomy tag" });
    expect(remove.closest("label")).toBeNull();
    expect(document.getElementById(input.getAttribute("aria-describedby")!)?.textContent).toContain("Press Enter");
    fireEvent.click(remove);
    fireEvent.keyDown(input, { key: "Backspace" });
    fireEvent.blur(input);
    expect(onChange).not.toHaveBeenCalled();
  });
});
