import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Dialog } from "../overlays/Dialog";
import { OverlayProvider } from "../overlays/OverlayProvider";
import { TOOLTIP_POINTER_DELAY_MS, Tooltip } from "./Tooltip";

function renderTooltip() {
  render(
    <>
      <span id="existing-description">Existing description</span>
      <Tooltip content="Helpful context">
        <button type="button" aria-describedby="existing-description">
          Action
        </button>
      </Tooltip>
    </>,
  );
  return screen.getByRole("button", { name: "Action" });
}

describe("Tooltip", () => {
  it.each(["mouse", "pen"])(
    "opens after the pointer delay for a %s pointer",
    (pointerType) => {
      vi.useFakeTimers();
      try {
        const trigger = renderTooltip();

        fireEvent.pointerEnter(trigger, { pointerType });
        act(() => vi.advanceTimersByTime(TOOLTIP_POINTER_DELAY_MS - 1));
        expect(screen.queryByRole("tooltip")).toBeNull();

        act(() => vi.advanceTimersByTime(1));
        expect(screen.getByRole("tooltip").textContent).toBe("Helpful context");
      } finally {
        vi.useRealTimers();
      }
    },
  );

  it("cancels delayed hover and ignores touch hover", () => {
    vi.useFakeTimers();
    try {
      const trigger = renderTooltip();

      fireEvent.pointerEnter(trigger, { pointerType: "mouse" });
      fireEvent.pointerLeave(trigger, { pointerType: "mouse" });
      act(() => vi.advanceTimersByTime(TOOLTIP_POINTER_DELAY_MS));
      expect(screen.queryByRole("tooltip")).toBeNull();

      fireEvent.pointerEnter(trigger, { pointerType: "touch" });
      act(() => vi.advanceTimersByTime(TOOLTIP_POINTER_DELAY_MS));
      expect(screen.queryByRole("tooltip")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens immediately for keyboard focus and preserves aria-describedby", () => {
    const trigger = renderTooltip();

    fireEvent.focus(trigger);

    const tooltip = screen.getByRole("tooltip");
    expect(trigger.getAttribute("aria-describedby")?.split(" ")).toEqual([
      "existing-description",
      tooltip.id,
    ]);
    fireEvent.blur(trigger);
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(trigger.getAttribute("aria-describedby")).toBe("existing-description");
  });

  it("dismisses on activation", () => {
    const trigger = renderTooltip();
    fireEvent.focus(trigger);
    expect(screen.getByRole("tooltip")).toBeTruthy();

    fireEvent.click(trigger);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("consumes the first Escape before an underlying dialog", () => {
    const onClose = vi.fn();
    render(
      <OverlayProvider>
        <Dialog title="Settings" onClose={onClose}>
          <Tooltip content="Edit this value">
            <button type="button">Edit</button>
          </Tooltip>
        </Dialog>
      </OverlayProvider>,
    );
    const trigger = screen.getByRole("button", { name: "Edit" });
    fireEvent.focus(trigger);

    fireEvent.keyDown(trigger, { key: "Escape" });
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.keyDown(trigger, { key: "Escape" });
    expect(onClose).toHaveBeenCalledWith("escape");
  });
});
