import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LibraryDragFeedback } from "./LibraryDragFeedback";

describe("LibraryDragFeedback", () => {
  it.each(["valid", "noop", "invalid"] as const)(
    "renders distinct %s feedback with destination and insertion copy",
    (status) => {
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
          bindPreview={vi.fn()}
        />,
      );
      expect(container.querySelector(`[data-status="${status}"]`)).not.toBeNull();
      expect(screen.getByText("Biology")).not.toBeNull();
      expect(screen.getByText("Nursing")).not.toBeNull();
      expect(screen.getByText("position 2")).not.toBeNull();
      expect(container.querySelectorAll("[aria-live='polite']")).toHaveLength(1);
    },
  );

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
