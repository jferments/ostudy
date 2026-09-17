import { fireEvent, render, screen, within } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";

import { OcclusionImageView } from "./OcclusionImageView";

const mocks = vi.hoisted(() => ({ zoom: vi.fn() }));
vi.mock("./useOcclusionWorkspace", () => ({
  useOcclusionWorkspace: () => ({
    svgRef: createRef<SVGSVGElement>(),
    frame: { width: 800, height: 500 },
    viewport: { zoom: 2, center: { x: 0.5, y: 0.5 } },
    transform: { left: 20, top: 20, width: 760, height: 460 },
    zoom: mocks.zoom,
    pointerDown: vi.fn(), pointerMove: vi.fn(), pointerUp: vi.fn(), pointerCancel: vi.fn(), keyDown: vi.fn(),
  }),
}));

describe("minimal study image controls", () => {
  it("shows only tiny zoom-out, zoom-in and pan controls", () => {
    render(<OcclusionImageView imageUrl="blob:masked" imageKey="instance" imageSize={{ width: 800, height: 500 }}
      targets={[{ x: 0.1, y: 0.1, width: 0.2, height: 0.2 }]} label="Masked study image" />);
    const controls = screen.getByRole("group", { name: "Study image controls" });
    const buttons = within(controls).getAllByRole("button");
    expect(buttons).toHaveLength(3);
    expect(within(controls).getByRole("button", { name: "Zoom out" }).querySelector("svg")).not.toBeNull();
    expect(within(controls).getByRole("button", { name: "Zoom in" }).querySelector("svg")).not.toBeNull();
    expect(within(controls).getByRole("button", { name: "Drag to pan" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.queryByRole("button", { name: "Fit image" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Zoom to target" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Target to enlarge" })).toBeNull();
    fireEvent.click(within(controls).getByRole("button", { name: "Zoom in" }));
    expect(mocks.zoom).toHaveBeenCalledWith(1.5);
  });
});
