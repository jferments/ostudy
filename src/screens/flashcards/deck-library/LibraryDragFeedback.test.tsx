import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LibraryDragFeedback } from "./LibraryDragFeedback";

describe("LibraryDragFeedback", () => {
  it.each(["valid", "noop", "invalid"] as const)(
    "announces a %s destination without rendering a detached partial-tile preview",
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

      expect(
        container.querySelector(
          `[data-drag-destination-feedback][data-status="${status}"]`,
        ),
      ).not.toBeNull();
      expect(screen.getByText("Nursing · position 2")).not.toBeNull();
      expect(screen.getByText(`Nursing, position 2. ${status}.`)).not.toBeNull();
      expect(container.querySelector("[data-drag-panel-preview]")).toBeNull();
      expect(container.querySelectorAll("[aria-live='polite']")).toHaveLength(1);
    },
  );

  it("does not invent a small status tile before a destination is resolved", () => {
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

    expect(container.querySelector("[data-drag-destination-feedback]")).toBeNull();
    expect(container.querySelector("[data-drag-panel-preview]")).toBeNull();
  });

  it("keeps completion announced after the visible DOM-clone preview is removed", () => {
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

    expect(container.querySelector("[data-drag-destination-feedback]")).toBeNull();
    expect(screen.getByText("Moved Biology to Nursing, position 2.")).not.toBeNull();
  });
});
