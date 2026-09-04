import { describe, expect, it } from "vitest";

import {
  EDGE_SCROLL_MAX_PX,
  POINTER_ACTIVATION_DISTANCE,
  TOUCH_POINTER_ACTIVATION_DISTANCE,
  activationDistance,
  canStartPointerSession,
  edgeScrollVelocity,
  movementExceedsThreshold,
} from "./pointerDragSession";

describe("pointer drag session policy", () => {
  it.each(["mouse", "pen", "touch"] as const)(
    "accepts the accessible broad drag surface itself for a primary %s pointer",
    (pointerType) => {
      const row = document.createElement("article");
      const surface = document.createElement("div");
      const copy = document.createElement("span");
      surface.setAttribute("role", "button");
      surface.tabIndex = 0;
      surface.dataset.libraryPointerDragSource = "true";
      surface.append(copy);
      row.append(surface);

      expect(
        canStartPointerSession({
          button: 0,
          isPrimary: true,
          pointerType,
          target: copy,
          currentTarget: surface,
        }),
      ).toBe(true);
    },
  );

  it.each(["button", "a", "input", "label", "textarea", "select"])(
    "rejects a nested %s control without rejecting its parent drag surface",
    (tag) => {
      const row = document.createElement("article");
      row.setAttribute("role", "button");
      row.tabIndex = 0;
      row.dataset.libraryPointerDragSource = "true";
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

  it("rejects an explicit nested drag exclusion and archived ancestors", () => {
    const archived = document.createElement("section");
    archived.dataset.archived = "true";
    const surface = document.createElement("div");
    surface.dataset.libraryPointerDragSource = "true";
    const excluded = document.createElement("span");
    excluded.dataset.libraryNoPointerDrag = "true";
    surface.append(excluded);
    archived.append(surface);

    expect(
      canStartPointerSession({
        button: 0,
        isPrimary: true,
        pointerType: "mouse",
        target: excluded,
        currentTarget: surface,
      }),
    ).toBe(false);

    excluded.removeAttribute("data-library-no-pointer-drag");
    expect(
      canStartPointerSession({
        button: 0,
        isPrimary: true,
        pointerType: "mouse",
        target: excluded,
        currentTarget: surface,
      }),
    ).toBe(false);
  });

  it("rejects non-primary buttons, secondary pointers, unsupported types, and foreign targets", () => {
    const row = document.createElement("article");
    const area = document.createElement("span");
    const outside = document.createElement("span");
    row.append(area);

    expect(
      canStartPointerSession({
        button: 1,
        isPrimary: true,
        pointerType: "mouse",
        target: area,
        currentTarget: row,
      }),
    ).toBe(false);
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
    expect(
      canStartPointerSession({
        button: 0,
        isPrimary: true,
        pointerType: "mouse",
        target: outside,
        currentTarget: row,
      }),
    ).toBe(false);
  });

  it("uses a responsive mouse threshold, a safer touch threshold, and bounded edge speed", () => {
    expect(POINTER_ACTIVATION_DISTANCE).toBe(4);
    expect(TOUCH_POINTER_ACTIVATION_DISTANCE).toBe(8);
    expect(activationDistance("mouse")).toBe(4);
    expect(activationDistance("pen")).toBe(4);
    expect(activationDistance("touch")).toBe(8);
    expect(movementExceedsThreshold(0, 0, 3, 0, activationDistance("mouse"))).toBe(false);
    expect(movementExceedsThreshold(0, 0, 4, 0, activationDistance("mouse"))).toBe(true);
    expect(movementExceedsThreshold(0, 0, 7, 0, activationDistance("touch"))).toBe(false);
    expect(movementExceedsThreshold(0, 0, 8, 0, activationDistance("touch"))).toBe(true);
    expect(edgeScrollVelocity(100, { top: 100, bottom: 500 })).toBe(-EDGE_SCROLL_MAX_PX);
    expect(edgeScrollVelocity(300, { top: 100, bottom: 500 })).toBe(0);
    expect(edgeScrollVelocity(500, { top: 100, bottom: 500 })).toBe(EDGE_SCROLL_MAX_PX);
  });
});
