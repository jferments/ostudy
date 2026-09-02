import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LibraryDragFeedback } from "./LibraryDragFeedback";

describe("LibraryDragFeedback", () => {
  it.each(["valid", "noop", "invalid"] as const)(
    "renders a panel-like %s drag ghost with destination copy",
    (status) => {
      const bindPreview = vi.fn();
      const { container } = render(
        <LibraryDragFeedback
          feedback={{
            active: true,
            sourceName: "Biology",
            candidate: {
              key: `${status}:nursing`,
              status,
              intent: null,
              targetKey: "folder:nursing",
              destinationFolderId: "nursing",
              destinationName: "Nursing",
              insertionLabel: "position 2",
              insertionTargetKey: "deck:biology",
              insertionEdge: "before",
              announcement: `Nursing, position 2. ${status}.`,
            },
          }}
          bindPreview={bindPreview}
        />,
      );
      const preview = container.querySelector<HTMLElement>(
        `[data-drag-panel-preview="true"][data-status="${status}"]`,
      );
      expect(preview).not.toBeNull();
      expect(preview?.style.transform).toContain("translate3d(");
      expect(bindPreview).toHaveBeenCalledWith(preview);
      expect(screen.getByText("Moving")).not.toBeNull();
      expect(screen.getByText("Biology")).not.toBeNull();
      expect(screen.getByText("Nursing · position 2")).not.toBeNull();
      expect(container.querySelectorAll("[aria-live='polite']")).toHaveLength(1);
    },
  );

  it("shows the full drag ghost immediately before a candidate is resolved", () => {
    const { container } = render(
      <LibraryDragFeedback
        feedback={{
          active: true,
          sourceName: "Biology",
          candidate: null,
        }}
        bindPreview={vi.fn()}
      />,
    );

    const preview = container.querySelector<HTMLElement>(
      '[data-drag-panel-preview="true"][data-status="pending"]',
    );
    expect(preview).not.toBeNull();
    expect(screen.getByText("Biology")).not.toBeNull();
    expect(screen.getByText("Choose a destination")).not.toBeNull();
  });

  it("keeps completion announced after the visual preview is removed", () => {
    const { container } = render(
      <LibraryDragFeedback
        feedback={{
          active: false,
          sourceName: "Biology",
          candidate: {
            key: "valid:nursing",
            status: "valid",
            intent: null,
            targetKey: "folder:nursing",
            destinationFolderId: "nursing",
            destinationName: "Nursing",
            insertionLabel: "position 2",
            insertionTargetKey: "deck:biology",
            insertionEdge: "before",
            announcement: "Moved Biology to Nursing, position 2.",
          },
        }}
        bindPreview={vi.fn()}
      />,
    );
    expect(container.querySelector("[data-drag-preview]")).toBeNull();
    expect(screen.getByText("Moved Biology to Nursing, position 2.")).not.toBeNull();
  });
});
