import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ResizablePanes, type ResizablePane } from "./ResizablePanes";

const panes: ResizablePane[] = [
  { id: "tree", label: "subjects", defaultSize: 260, minSize: 200, maxSize: 400, content: <p>Tree</p> },
  { id: "cards", label: "cards", minSize: 320, content: <p>Cards</p> },
  { id: "editor", label: "card editor", defaultSize: 380, minSize: 320, maxSize: 600, content: <p>Editor</p> },
];

function paneWidth(id: string) {
  return (document.querySelector(`[data-pane="${id}"]`) as HTMLElement).style.width;
}

describe("ResizablePanes", () => {
  it("draws a handle between each pair of panes for the fixed pane beside it", () => {
    render(<ResizablePanes panes={panes} />);
    const handles = screen.getAllByRole("separator");
    expect(handles.map((handle) => handle.getAttribute("aria-label"))).toEqual([
      "Resize subjects",
      "Resize card editor",
    ]);
    expect(handles[0]!.getAttribute("aria-valuenow")).toBe("260");
    expect(handles[0]!.getAttribute("aria-valuemin")).toBe("200");
    expect(handles[0]!.getAttribute("aria-controls")).toBe("pane-tree");
    expect(paneWidth("tree")).toBe("260px");
    expect(paneWidth("editor")).toBe("380px");
  });

  it("resizes with the keyboard within the pane's limits and reports the result", () => {
    const onSizesChange = vi.fn();
    render(<ResizablePanes panes={panes} onSizesChange={onSizesChange} />);
    const [tree, editor] = screen.getAllByRole("separator");
    fireEvent.keyDown(tree!, { key: "ArrowRight" });
    expect(onSizesChange).toHaveBeenLastCalledWith({ tree: 276, editor: 380 });
    // The editor sits right of its handle, so moving the handle left widens it.
    fireEvent.keyDown(editor!, { key: "ArrowLeft", shiftKey: true });
    expect(onSizesChange).toHaveBeenLastCalledWith({ tree: 260, editor: 444 });
    fireEvent.keyDown(tree!, { key: "Home" });
    expect(onSizesChange).toHaveBeenLastCalledWith({ tree: 200, editor: 380 });
    fireEvent.keyDown(tree!, { key: "End" });
    expect(onSizesChange).toHaveBeenLastCalledWith({ tree: 400, editor: 380 });
  });

  it("follows a pointer drag and commits once on release", () => {
    const onSizesChange = vi.fn();
    render(<ResizablePanes panes={panes} sizes={{ tree: 300 }} onSizesChange={onSizesChange} />);
    const [tree] = screen.getAllByRole("separator");
    fireEvent.pointerDown(tree!, { pointerId: 1, button: 0, clientX: 300 });
    fireEvent.pointerMove(tree!, { pointerId: 1, clientX: 340 });
    expect(paneWidth("tree")).toBe("340px");
    expect(onSizesChange).not.toHaveBeenCalled();
    fireEvent.pointerMove(tree!, { pointerId: 1, clientX: 900 });
    expect(paneWidth("tree")).toBe("400px");
    fireEvent.pointerUp(tree!, { pointerId: 1, clientX: 900 });
    expect(onSizesChange).toHaveBeenCalledTimes(1);
    expect(onSizesChange).toHaveBeenLastCalledWith({ tree: 400, editor: 380 });
  });

  it("restores the default width on double click", () => {
    const onSizesChange = vi.fn();
    render(<ResizablePanes panes={panes} sizes={{ tree: 350 }} onSizesChange={onSizesChange} />);
    fireEvent.doubleClick(screen.getAllByRole("separator")[0]!);
    expect(onSizesChange).toHaveBeenLastCalledWith({ tree: 260, editor: 380 });
  });
});
