import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import { Textarea } from "../../ui/primitives/FormControls";
import { OcclusionWorkspace, type OcclusionWorkspaceProps } from "./OcclusionWorkspace";

const mask = { id: "mask-a", rect: { x: 0.2, y: 0.2, width: 0.2, height: 0.2 } };
let box: DOMRect;
let resizeCallbacks: Set<() => void>;

function props(overrides: Partial<OcclusionWorkspaceProps> = {}): OcclusionWorkspaceProps {
  return {
    imageKey: "image-a", imageUrl: "blob:managed-image", imageSize: { width: 1000, height: 1000 },
    masks: [mask], selectedMaskId: mask.id, disabled: false, canUndo: true, canRedo: false,
    onSelect: vi.fn(), onMutation: vi.fn(), onDelete: vi.fn(), onUndo: vi.fn(), onRedo: vi.fn(),
    ...overrides,
  };
}
function svg(): SVGSVGElement {
  const element = screen.getByRole("group", { name: "Editable image regions" }) as unknown as SVGSVGElement;
  const captured = new Set<number>();
  Object.defineProperties(element, {
    setPointerCapture: { configurable: true, value: (id: number) => captured.add(id) },
    hasPointerCapture: { configurable: true, value: (id: number) => captured.has(id) },
    releasePointerCapture: { configurable: true, value: (id: number) => captured.delete(id) },
  });
  return element;
}
function point(x: number, y: number, pointerId = 1, pointerType = "mouse") {
  return { pointerId, pointerType, button: 0, clientX: 16 + x * 468, clientY: 16 + y * 468 };
}

beforeEach(() => {
  box = new DOMRect(0, 0, 500, 500);
  resizeCallbacks = new Set();
  vi.spyOn(SVGElement.prototype, "getBoundingClientRect").mockImplementation(() => box);
  vi.stubGlobal("ResizeObserver", class {
    private notify: () => void;
    constructor(callback: ResizeObserverCallback) {
      this.notify = () => callback([], this);
      resizeCallbacks.add(this.notify);
    }
    observe() {}
    unobserve() {}
    disconnect() { resizeCallbacks.delete(this.notify); }
  });
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("image occlusion workspace", () => {
  it.each(["mouse", "touch", "pen"])("draws with %s and publishes one edit only when released", (pointerType) => {
    const options = props();
    render(<OcclusionWorkspace {...options} />);
    fireEvent.click(screen.getByRole("radio", { name: "Draw rectangle" }));
    const canvas = svg();
    fireEvent.pointerDown(canvas, point(0.1, 0.1, 1, pointerType));
    fireEvent.pointerMove(canvas, point(0.5, 0.5, 1, pointerType));
    expect(options.onMutation).not.toHaveBeenCalled();
    fireEvent.pointerUp(canvas, point(0.5, 0.5, 1, pointerType));
    expect(options.onMutation).toHaveBeenCalledTimes(1);
    expect(required(vi.mocked(options.onMutation).mock.calls[0]?.[0], "occlusion mutation")).toMatchObject({ kind: "create" });
  });

  it("supports two taps and Add rectangle without a drag gesture", () => {
    const options = props({ masks: [], selectedMaskId: null });
    render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    fireEvent.click(screen.getByRole("radio", { name: "Tap two corners" }));
    fireEvent.pointerDown(canvas, point(0.2, 0.2));
    fireEvent.pointerUp(canvas, point(0.2, 0.2));
    expect(screen.getByText(/Tap the opposite corner/)).toBeTruthy();
    expect(options.onMutation).not.toHaveBeenCalled();
    fireEvent.pointerDown(canvas, point(0.6, 0.6));
    fireEvent.pointerUp(canvas, point(0.6, 0.6));
    expect(options.onMutation).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Add rectangle" }));
    expect(options.onMutation).toHaveBeenCalledTimes(2);
  });

  it("cancels a partial drawing on pointer cancellation or Escape", () => {
    const options = props();
    render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    fireEvent.click(screen.getByRole("radio", { name: "Draw rectangle" }));
    fireEvent.pointerDown(canvas, point(0.1, 0.1));
    fireEvent.pointerMove(canvas, point(0.6, 0.6));
    fireEvent.pointerCancel(canvas, point(0.6, 0.6));
    fireEvent.pointerUp(canvas, point(0.6, 0.6));
    expect(options.onMutation).not.toHaveBeenCalled();
    fireEvent.pointerDown(canvas, point(0.1, 0.1));
    fireEvent.pointerMove(canvas, point(0.6, 0.6));
    fireEvent.keyDown(canvas, { key: "Escape" });
    fireEvent.pointerUp(canvas, point(0.6, 0.6));
    expect(options.onMutation).not.toHaveBeenCalled();
  });

  it("pinches without drawing and does not resume drawing when one finger lifts", () => {
    const options = props();
    render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    fireEvent.click(screen.getByRole("radio", { name: "Draw rectangle" }));
    fireEvent.pointerDown(canvas, point(0.2, 0.2, 1, "touch"));
    fireEvent.pointerMove(canvas, point(0.3, 0.3, 1, "touch"));
    fireEvent.pointerDown(canvas, point(0.6, 0.6, 2, "touch"));
    fireEvent.pointerMove(canvas, point(0.8, 0.8, 2, "touch"));
    expect(screen.queryByText("100% of Fit")).toBeNull();
    fireEvent.pointerUp(canvas, point(0.8, 0.8, 2, "touch"));
    fireEvent.pointerMove(canvas, point(0.9, 0.9, 1, "touch"));
    fireEvent.pointerUp(canvas, point(0.9, 0.9, 1, "touch"));
    expect(options.onMutation).not.toHaveBeenCalled();
  });

  it("provides keyboard and button alternatives for moving, resizing and removing", () => {
    const options = props();
    render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    fireEvent.keyDown(canvas, { key: "ArrowRight" });
    expect(required(vi.mocked(options.onMutation).mock.calls[0]?.[0], "occlusion mutation")).toMatchObject({
      kind: "change", maskId: mask.id, after: { x: 0.201 },
    });
    fireEvent.click(screen.getByRole("button", { name: "Move down" }));
    fireEvent.click(screen.getByRole("button", { name: "Wider" }));
    expect(options.onMutation).toHaveBeenCalledTimes(3);
    fireEvent.keyDown(canvas, { key: "Delete" });
    expect(options.onDelete).toHaveBeenCalledExactlyOnceWith(mask.id);
    fireEvent.keyDown(canvas, { key: "z", ctrlKey: true });
    expect(options.onUndo).toHaveBeenCalledTimes(1);
  });

  it("does not take text-editing keyboard shortcuts from a sibling rich-field control", () => {
    const options = props();
    render(<><Textarea aria-label="Question answer" /><OcclusionWorkspace {...options} /></>);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Question answer" }), { key: "z", ctrlKey: true });
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Question answer" }), { key: "Delete" });
    expect(options.onUndo).not.toHaveBeenCalled();
    expect(options.onDelete).not.toHaveBeenCalled();
  });

  it("a resize or lost focus cancels a pending edit without changing the saved masks", () => {
    const options = props();
    render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    fireEvent.click(screen.getByRole("radio", { name: "Draw rectangle" }));
    fireEvent.pointerDown(canvas, point(0.1, 0.1));
    fireEvent.pointerMove(canvas, point(0.6, 0.6));
    act(() => {
      box = new DOMRect(0, 0, 320, 250);
      for (const callback of resizeCallbacks) callback();
    });
    fireEvent.pointerUp(canvas, point(0.6, 0.6));
    expect(options.onMutation).not.toHaveBeenCalled();
    expect(canvas.getAttribute("viewBox")).toBe("0 0 320 250");
    fireEvent.pointerDown(canvas, { ...point(0.1, 0.1), clientX: 100, clientY: 100 });
    fireEvent.pointerMove(canvas, { ...point(0.6, 0.6), clientX: 200, clientY: 200 });
    fireEvent.blur(window);
    fireEvent.pointerUp(canvas, { ...point(0.6, 0.6), clientX: 200, clientY: 200 });
    expect(options.onMutation).not.toHaveBeenCalled();
  });

  it("rejects a move if the parent changed that region during the gesture", () => {
    const options = props();
    const result = render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    const region = canvas.querySelector('[data-mask-id="mask-a"] rect');
    expect(region).not.toBeNull();
    fireEvent.pointerDown(region!, point(0.3, 0.3));
    fireEvent.pointerMove(canvas, point(0.5, 0.5));
    result.rerender(<OcclusionWorkspace {...options} masks={[{ ...mask, rect: { ...mask.rect, x: 0.6 } }]} />);
    fireEvent.pointerUp(canvas, point(0.5, 0.5));
    expect(options.onMutation).not.toHaveBeenCalled();
    expect(screen.getByRole("status").textContent).toMatch(/No geometry was overwritten/);
  });

  it("disabling editing cancels any in-flight geometry mutation", () => {
    const options = props();
    const result = render(<OcclusionWorkspace {...options} />);
    const canvas = svg();
    fireEvent.click(screen.getByRole("radio", { name: "Draw rectangle" }));
    fireEvent.pointerDown(canvas, point(0.1, 0.1));
    fireEvent.pointerMove(canvas, point(0.6, 0.6));
    result.rerender(<OcclusionWorkspace {...options} disabled />);
    fireEvent.pointerUp(canvas, point(0.6, 0.6));
    expect(options.onMutation).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Add rectangle" })).toHaveProperty("disabled", true);
  });
});
