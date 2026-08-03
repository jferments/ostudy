import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import type { TagSummary } from "../ipc";
import { TagInput } from "./TagInput";

const suggestions: TagSummary[] = [
  { id: "a".repeat(32), name: "Cardiology", cardCount: 12 },
  { id: "b".repeat(32), name: "Medication safety", cardCount: 4 },
];

function Harness({ initial = [] }: { initial?: string[] }) {
  const [tags, setTags] = useState(initial);
  return (
    <>
      <TagInput tags={tags} suggestions={suggestions} onChange={setTags} />
      <output aria-label="Current tags">{tags.join("|")}</output>
    </>
  );
}

describe("TagInput", () => {
  it("normalizes whitespace and accepts Enter, comma, and blur", () => {
    render(<Harness />);
    const input = screen.getByRole("textbox", { name: "Tags" });

    fireEvent.change(input, { target: { value: "  Medication   safety  " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByLabelText("Current tags").textContent).toBe(
      "Medication safety",
    );

    fireEvent.change(input, { target: { value: "Cardiology" } });
    fireEvent.keyDown(input, { key: "," });
    expect(screen.getByLabelText("Current tags").textContent).toBe(
      "Medication safety|Cardiology",
    );

    fireEvent.change(input, { target: { value: "Respiratory" } });
    fireEvent.blur(input);
    expect(screen.getByLabelText("Current tags").textContent).toBe(
      "Medication safety|Cardiology|Respiratory",
    );
  });

  it("rejects case-insensitive duplicates and supports keyboard removal", () => {
    render(<Harness initial={["Cardiology", "Medication"]} />);
    const input = screen.getByRole("textbox", { name: "Tags" });

    fireEvent.change(input, { target: { value: "cardiology" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByLabelText("Current tags").textContent).toBe(
      "Cardiology|Medication",
    );

    fireEvent.keyDown(input, { key: "Backspace" });
    expect(screen.getByLabelText("Current tags").textContent).toBe("Cardiology");
  });

  it("removes a selected chip and omits it from suggestions", () => {
    render(<Harness initial={["Cardiology"]} />);

    expect(
      screen.queryByRole("option", { name: /Cardiology/u }),
    ).toBeNull();
    expect(
      screen.getByRole("option", { name: /Medication safety/u }),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Remove Cardiology tag" }));
    expect(screen.getByLabelText("Current tags").textContent).toBe("");
  });
});
