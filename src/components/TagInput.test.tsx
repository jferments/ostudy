import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import {
  type TagSummary,
} from "../ipc/cards";
import { TagInput } from "./TagInput";

const suggestions: TagSummary[] = [
  {
    id: "a".repeat(32),
    name: "Cardiology",
    sourceNoteCount: 10,
    generatedCardCount: 12,
    studyableDueCount: 4,
  },
  {
    id: "b".repeat(32),
    name: "Medication safety",
    sourceNoteCount: 4,
    generatedCardCount: 4,
    studyableDueCount: 1,
  },
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

function datalistValues(): string[] {
  return [...document.querySelectorAll<HTMLOptionElement>("datalist option")].map(
    (option) => option.value,
  );
}

describe("TagInput", () => {
  it("normalizes whitespace and accepts Enter, comma, and blur", () => {
    render(<Harness />);
    const input = screen.getByRole("combobox", { name: "Tags" });

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
    const input = screen.getByRole("combobox", { name: "Tags" });

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

    expect(datalistValues()).toEqual(["Medication safety"]);

    fireEvent.click(screen.getByRole("button", { name: "Remove Cardiology tag" }));
    expect(screen.getByLabelText("Current tags").textContent).toBe("");
    expect(datalistValues()).toEqual(["Cardiology", "Medication safety"]);
  });
});