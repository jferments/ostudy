import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Button } from "../../ui/primitives/Button";
import { ImageOcclusionEditor } from "./ImageOcclusionEditor";
import { newOcclusionDraft, occlusionDefinition, occlusionDraftReady,
  type OcclusionEditorDraft } from "./occlusionDraft";

const mocks = vi.hoisted(() => ({ save: vi.fn(), confirm: vi.fn(), inspector: vi.fn() }));
vi.mock("../../ui/overlays/overlayContext", () => ({ useConfirm: () => mocks.confirm }));
vi.mock("./useOcclusionImage", () => ({
  useOcclusionSource: () => ({ url: "blob:retained-image", size: { width: 1000, height: 1000 }, error: null, retry: vi.fn() }),
  useOcclusionImport: () => ({ busy: false, error: null, start: vi.fn(), retry: vi.fn(), cancel: vi.fn() }),
}));
vi.mock("./OcclusionQuestionInspector", () => ({ OcclusionQuestionInspector: (props: unknown) => {
  mocks.inspector(props);
  return <section aria-label="Image question inspector">Custom question controls</section>;
} }));
function initial(): OcclusionEditorDraft {
  return { ...newOcclusionDraft(), imageAssetId: "a".repeat(32), imageSize: { width: 1000, height: 1000 } };
}
function Harness() {
  const [draft, setDraft] = useState(initial);
  return <>
    <ImageOcclusionEditor draft={draft} assets={[]} assetDraftId={"b".repeat(32)} disabled={false}
      onAssetImported={vi.fn()} onUpdate={setDraft} flushDraft={async () => {}} />
    <Button disabled={!occlusionDraftReady(draft)} onClick={() => mocks.save(occlusionDefinition(draft))}>Save test card</Button>
  </>;
}
function canvas(): SVGSVGElement {
  const element = screen.getByRole("group", { name: "Editable image regions" }) as unknown as SVGSVGElement;
  const captured = new Set<number>();
  Object.defineProperties(element, {
    setPointerCapture: { configurable: true, value: (id: number) => captured.add(id) },
    hasPointerCapture: { configurable: true, value: (id: number) => captured.has(id) },
    releasePointerCapture: { configurable: true, value: (id: number) => captured.delete(id) },
  });
  return element;
}
function draw(svg: SVGSVGElement, x: number, y: number): void {
  const point = { pointerId: 1, pointerType: "mouse", button: 0, clientX: 16 + x * 468, clientY: 16 + y * 468 };
  fireEvent.pointerDown(svg, point);
  fireEvent.pointerMove(svg, { ...point, clientX: point.clientX + 55, clientY: point.clientY + 35 });
  fireEvent.pointerUp(svg, { ...point, clientX: point.clientX + 55, clientY: point.clientY + 35 });
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.confirm.mockResolvedValue(true);
  vi.spyOn(SVGElement.prototype, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 500, 500));
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
});
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("rectangle-first image occlusion editor", () => {
  it("starts in continuous Draw mode and saves several cards without opening prose controls", () => {
    render(<Harness />);
    expect(screen.getByRole("button", { name: "Draw rectangle" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.queryByRole("region", { name: "Image question inspector" })).toBeNull();
    expect(mocks.inspector).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Save test card" })).toHaveProperty("disabled", true);
    const svg = canvas();
    draw(svg, 0.1, 0.1);
    draw(svg, 0.5, 0.5);
    expect(screen.getByText("2 regions · 2 study cards")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Draw rectangle" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Save test card" }));
    const question = mocks.save.mock.calls[0]?.[0].question;
    expect(question.masks).toHaveLength(2);
    expect(question.questions).toHaveLength(2);
    expect(question.questions.every((item: { mode: string }) => item.mode === "hide_target")).toBe(true);
    expect(JSON.stringify(question.questions)).toContain("Check your recall against the uncovered image.");
  });

  it("applies hide-all to existing and subsequent rectangles while keeping optional editing behind a toggle", () => {
    render(<Harness />);
    const svg = canvas();
    draw(svg, 0.1, 0.1);
    fireEvent.change(screen.getByRole("combobox", { name: "Question-side masks" }), { target: { value: "hide_all" } });
    draw(svg, 0.5, 0.5);
    fireEvent.click(screen.getByRole("button", { name: "Save test card" }));
    expect(mocks.save.mock.calls[0]?.[0].question.questions.every((item: { mode: string }) => item.mode === "hide_all")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Optional question details" }));
    expect(screen.getByText("Custom question controls")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Hide optional question details" }));
    expect(screen.queryByText("Custom question controls")).toBeNull();
  });

  it("groups rectangles into one card and restores a deleted quick region without a confirmation or prose repair", () => {
    render(<Harness />);
    const svg = canvas();
    draw(svg, 0.1, 0.1);
    fireEvent.click(screen.getByRole("checkbox", { name: "Group new rectangles with the selected card" }));
    draw(svg, 0.5, 0.5);
    expect(screen.getByText("2 regions · 1 study cards")).toBeTruthy();
    fireEvent.keyDown(svg, { key: "Delete" });
    expect(mocks.confirm).not.toHaveBeenCalled();
    expect(screen.getByText("1 regions · 1 study cards")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save test card" })).toHaveProperty("disabled", false);
    fireEvent.click(screen.getByRole("button", { name: "Undo region edit" }));
    fireEvent.click(screen.getByRole("button", { name: "Save test card" }));
    expect(mocks.save.mock.calls[0]?.[0].question.questions[0].maskIds).toHaveLength(2);
  });
});
