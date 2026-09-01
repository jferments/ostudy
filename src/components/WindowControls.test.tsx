import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { WindowControlsController } from "../controllers/useWindowControlsController";
import { WindowControls } from "./WindowControls";

function makeController(
  overrides: Partial<WindowControlsController> = {},
): WindowControlsController {
  return {
    maximizeRestoreAvailable: true,
    fullScreenAvailable: true,
    ready: true,
    maximized: false,
    fullScreen: false,
    pendingAction: null,
    toggleMaximized: vi.fn().mockResolvedValue(undefined),
    toggleFullScreen: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("WindowControls", () => {
  it("renders keyboard-focusable native buttons with action-accurate labels", () => {
    const controller = makeController();
    render(<WindowControls controller={controller} placement="sidebar" />);

    const group = screen.getByRole("group", { name: "Window controls" });
    const maximize = screen.getByRole("button", { name: "Maximize window" });
    const fullScreen = screen.getByRole("button", { name: "Enter full screen" });
    expect(group.contains(maximize)).toBe(true);
    expect(maximize.tagName).toBe("BUTTON");
    expect(maximize.getAttribute("type")).toBe("button");
    expect(maximize.getAttribute("aria-pressed")).toBe("false");
    expect(fullScreen.getAttribute("aria-pressed")).toBe("false");

    act(() => maximize.focus());
    expect(document.activeElement).toBe(maximize);
    expect(screen.getByRole("tooltip").textContent).toBe("Maximize window");
    fireEvent.click(maximize);
    fireEvent.click(fullScreen);
    expect(controller.toggleMaximized).toHaveBeenCalledOnce();
    expect(controller.toggleFullScreen).toHaveBeenCalledOnce();
  });

  it("announces restore and exit actions for active native states", () => {
    render(
      <WindowControls
        controller={makeController({ maximized: true, fullScreen: true })}
        placement="sidebar"
      />,
    );

    expect(
      screen.getByRole("button", { name: "Restore window" }).getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      screen.getByRole("button", { name: "Exit full screen" }).getAttribute(
        "aria-pressed",
      ),
    ).toBe("true");
  });

  it("disables conflicting actions and identifies the pending native operation", () => {
    render(
      <WindowControls
        controller={makeController({ pendingAction: "maximize" })}
        placement="sidebar"
      />,
    );

    const maximize = screen.getByRole("button", { name: "Maximize window" });
    const fullScreen = screen.getByRole("button", { name: "Enter full screen" });
    expect((maximize as HTMLButtonElement).disabled).toBe(true);
    expect(maximize.getAttribute("aria-busy")).toBe("true");
    expect((fullScreen as HTMLButtonElement).disabled).toBe(true);
    expect(fullScreen.hasAttribute("aria-busy")).toBe(false);
  });

  it("renders only independently available operations", () => {
    render(
      <WindowControls
        controller={makeController({ maximizeRestoreAvailable: false })}
        placement="sidebar"
      />,
    );

    expect(screen.queryByRole("button", { name: "Maximize window" })).toBeNull();
    expect(screen.getByRole("button", { name: "Enter full screen" })).toBeTruthy();
  });

  it("renders no browser fallback when native capabilities are unavailable", () => {
    const { container } = render(
      <WindowControls
        controller={makeController({
          maximizeRestoreAvailable: false,
          fullScreenAvailable: false,
        })}
        placement="sidebar"
      />,
    );

    expect(container.textContent).toBe("");
    expect(screen.queryByRole("group", { name: "Window controls" })).toBeNull();
  });
});
