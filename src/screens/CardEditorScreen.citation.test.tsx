import { render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";

import { CardEditorScreen } from "./CardEditorScreen";

vi.mock("../components/SourceCitationPanel", () => ({
  SourceCitationPanel: ({ noteId }: { noteId?: string }) => (
    <div>Source citation for {noteId}</div>
  ),
}));

vi.mock("./CardEditorScreenBase", () => ({
  CardEditorScreen: () => <div>Card editor body</div>,
}));

function props(card: { noteId: string } | null): ComponentProps<typeof CardEditorScreen> {
  return {
    deck: { id: "deck-1" },
    card,
    onCancel: vi.fn(),
    onSaved: vi.fn(),
    onTemplateGenerated: vi.fn(),
    onDirtyChange: vi.fn(),
  } as unknown as ComponentProps<typeof CardEditorScreen>;
}

describe("CardEditorScreen source provenance shell", () => {
  it("loads a durable citation when an existing card is opened", () => {
    render(<CardEditorScreen {...props({ noteId: "note-1" })} />);

    expect(screen.getByText("Source citation for note-1")).toBeTruthy();
    expect(screen.getByText("Card editor body")).toBeTruthy();
  });

  it("does not request provenance for a new card", () => {
    render(<CardEditorScreen {...props(null)} />);

    expect(screen.queryByText(/Source citation for/u)).toBeNull();
    expect(screen.getByText("Card editor body")).toBeTruthy();
  });
});
