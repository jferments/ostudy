import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { TutorSources } from "./TutorSources";

describe("TutorSources", () => {
  it("uses the product collection terminology for collection sources", () => {
    render(
      <OverlayProvider>
        <TutorSources
          scope={{ kind: "general", id: null }}
          choices={[{
            scope: { kind: "collection", id: "11111111111111111111111111111111" },
            title: "GI physiology",
          }]}
          onApply={vi.fn()}
          onClose={vi.fn()}
        />
      </OverlayProvider>,
    );

    expect(screen.getByText("Document collection")).toBeTruthy();
    expect(screen.queryByText("Document folder")).toBeNull();
  });
});
