import { describe, expect, it, vi } from "vitest";

import {
  EDGE_SCROLL_MAX_PX,
  canStartPointerSession,
  edgeScrollVelocity,
  movementExceedsThreshold,
} from "./pointerDragSession";

describe("pointer drag session policy", () => {
  it("accepts primary pointer starts from non-interactive active row areas", () => {
    const row = document.createElement("article");
    const area = document.createElement("span");
    row.append(area);
    for (const pointerType of ["mouse", "pen", "touch"] as const) {
      expect(
        canStartPointerSession({
          button: 0,
          isPrimary: true,
          pointerType,
          target: area,
          currentTarget: row,
        }),
      ).toBe(true);
    }
  });

  it.each(["button", "a", "input", "label", "textarea", "select"])(
    "rejects starts from %s controls",
    (tag) => {
      const row = document.createElement("article");
      const control = document.createElement(tag);
      if (control instanceof HTMLAnchorElement) control.href = "#target";
      row.append(control);
      expect(
        canStartPointerSession({
          button: 0,
          isPrimary: true,
          pointerType: "mouse",
          target: control,
          currentTarget: row,
        }),
      ).toBe(false);
    },
  );

  it("rejects archived rows, non-primary buttons, editable content, and active selection", () => {
    const row = document.createElement("article");
    const area = document.createElement("span");
    row.append(area);
    row.dataset.archived = "true";
    expect(canStartPointerSession({ button: 0, isPrimary: true, pointerType: "mouse", target: area, currentTarget: row })).toBe(false);
    delete row.dataset.archived;
    expect(canStartPointerSession({ button: 1, isPrimary: true, pointerType: "mouse", target: area, currentTarget: row })).toBe(false);
    area.setAttribute("contenteditable", "true");
    expect(canStartPointerSession({ button: 0, isPrimary: true, pointerType: "mouse", target: area, currentTarget: row })).toBe(false);
    area.removeAttribute("contenteditable");
    vi.spyOn(document, "getSelection").mockReturnValue({
      isCollapsed: false,
      toString: () => "selected",
    } as Selection);
    expect(canStartPointerSession({ button: 0, isPrimary: true, pointerType: "mouse", target: area, currentTarget: row })).toBe(false);
    vi.restoreAllMocks();
  });

  it("rejects non-primary pointers and unsupported pointer types", () => {
    const row = document.createElement("article");
    const area = document.createElement("span");
    row.append(area);
    expect(
      canStartPointerSession({
        button: 0,
        isPrimary: false,
        pointerType: "mouse",
        target: area,
        currentTarget: row,
      }),
    ).toBe(false);
    expect(
      canStartPointerSession({
        button: 0,
        isPrimary: true,
        pointerType: "eraser" as "pen",
        target: area,
        currentTarget: row,
      }),
    ).toBe(false);
  });

  it("uses a tested activation threshold and bounded edge speed", () => {
    expect(movementExceedsThreshold(0, 0, 7, 0)).toBe(false);
    expect(movementExceedsThreshold(0, 0, 8, 0)).toBe(true);
    expect(edgeScrollVelocity(100, { top: 100, bottom: 500 })).toBe(-EDGE_SCROLL_MAX_PX);
    expect(edgeScrollVelocity(300, { top: 100, bottom: 500 })).toBe(0);
    expect(edgeScrollVelocity(500, { top: 100, bottom: 500 })).toBe(EDGE_SCROLL_MAX_PX);
  });
});
